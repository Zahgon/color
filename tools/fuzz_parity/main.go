// Command fuzz_parity generates a *randomized* differential corpus.
//
// Unlike the fixed matrices in tools/gofmt_golden and tools/color_golden, this
// tool synthesizes random format strings, random argument lists and random
// attribute sets, evaluates them with the real Go implementation, and writes
// corpora in the same TSV formats. Pointing the Rust replay tests at the
// output (via GOFMT_GOLDEN / COLOR_GOLDEN) checks behavior on inputs nobody
// hand-picked.
//
// Usage:
//
//	go run ./tools/fuzz_parity -seed 1 -n 20000 -fmt /tmp/f.tsv -color /tmp/c.tsv
package main

import (
	"bufio"
	"bytes"
	"encoding/hex"
	"flag"
	"fmt"
	"math"
	"math/rand"
	"os"
	"strconv"
	"strings"

	"github.com/fatih/color"
)

var (
	seed     = flag.Int64("seed", 1, "random seed")
	n        = flag.Int("n", 20000, "number of cases per corpus")
	fmtOut   = flag.String("fmt", "", "path for the gofmt corpus")
	colorOut = flag.String("color", "", "path for the color corpus")
)

// ---------------------------------------------------------------------------
// random argument generation (argspec encoding shared with the other tools)
// ---------------------------------------------------------------------------

type arg struct {
	spec  string
	value interface{}
}

var interestingStrings = []string{
	"", "a", "hello", "héllo wörld", "世界", "tab\there", "nl\nhere",
	"\x1b[31mred\x1b[0m", "%s", "%d%%", "a\"b\\c", "`tick`", "\x00\x01\x7f",
	"😀", "\u00a0", "\u200b", "\ufeff", strings.Repeat("x", 40),
}

var interestingInts = []int64{
	0, 1, -1, 7, 42, -42, 65, 255, -255, 1000000, 1000001, -1000001,
	1 << 20, math.MaxInt32, math.MinInt32, math.MaxInt64, math.MinInt64,
	0x10FFFF, 0x110000, 0x100000041, 0xD800,
}

var interestingUints = []uint64{
	0, 1, 255, 4294967295, 1 << 62, 1 << 63, math.MaxUint64,
}

var interestingFloats = []float64{
	0, math.Copysign(0, -1), 1, -1, 0.5, 1.5, 0.1, 1e6, 1e20, 1e21, 1e-5,
	-1.5e-7, 123456, 1234567, math.Pi, math.MaxFloat64, math.SmallestNonzeroFloat64,
	math.NaN(), math.Inf(1), math.Inf(-1),
}

func randArg(r *rand.Rand) arg {
	switch r.Intn(10) {
	case 0:
		return arg{"n", nil}
	case 1:
		if r.Intn(2) == 0 {
			return arg{"b:1", true}
		}
		return arg{"b:0", false}
	case 2, 3:
		v := interestingInts[r.Intn(len(interestingInts))]
		return arg{"i:" + strconv.FormatInt(v, 10), int(v)}
	case 4:
		v := interestingUints[r.Intn(len(interestingUints))]
		return arg{"u:" + strconv.FormatUint(v, 10), uint(v)}
	case 5:
		v := interestingFloats[r.Intn(len(interestingFloats))]
		return arg{"f:" + fmt.Sprintf("%016x", math.Float64bits(v)), v}
	case 6:
		b := make([]byte, r.Intn(5))
		for i := range b {
			b[i] = byte(r.Intn(128)) // keep it valid UTF-8
		}
		return arg{"y:" + hex.EncodeToString(b), b}
	case 7:
		s := interestingStrings[r.Intn(len(interestingStrings))]
		return arg{"e:" + hex.EncodeToString([]byte(s)), fmt.Errorf("%s", s)}
	case 8:
		k := r.Intn(3)
		items := make([]arg, k)
		specs := make([]string, k)
		vals := make([]interface{}, k)
		for i := 0; i < k; i++ {
			items[i] = randScalar(r)
			specs[i] = items[i].spec
			vals[i] = items[i].value
		}
		return arg{"l:" + hex.EncodeToString([]byte(strings.Join(specs, ";"))), vals}
	default:
		s := interestingStrings[r.Intn(len(interestingStrings))]
		return arg{"s:" + hex.EncodeToString([]byte(s)), s}
	}
}

// ---------------------------------------------------------------------------
// determinism guard
// ---------------------------------------------------------------------------

// refreshArgs returns an equal-valued argument list built from fresh
// allocations.
//
// Only the pointer-shaped operands differ between the two: `errors.New` hands
// back a `*errors.errorString`, and several code paths in `fmt` render such a
// value as its *address* (`%p`, `%#v`, and the integer verbs by way of
// `fmtPointer`). Those outputs change from run to run, so a corpus entry
// recording one is worthless as a reference — no implementation, in Go or in
// Rust, can reproduce it.
func refreshArgs(vals []interface{}) []interface{} {
	out := make([]interface{}, len(vals))
	for i, v := range vals {
		switch t := v.(type) {
		case error:
			out[i] = fmt.Errorf("%s", t.Error())
		case []interface{}:
			out[i] = refreshArgs(t)
		default:
			out[i] = v
		}
	}
	return out
}

// isDeterministic reports whether `produce` yields the same bytes for two
// equal-valued but separately allocated argument lists. Cases that fail this
// check are dropped from the corpus.
func isDeterministic(vals []interface{}, produce func([]interface{}) string) bool {
	return produce(vals) == produce(refreshArgs(vals))
}

// randScalar avoids nesting slices inside slices.
func randScalar(r *rand.Rand) arg {
	for {
		a := randArg(r)
		if !strings.HasPrefix(a.spec, "l:") {
			return a
		}
	}
}

func randArgs(r *rand.Rand) (string, []interface{}) {
	k := r.Intn(4)
	specs := make([]string, k)
	vals := make([]interface{}, k)
	for i := 0; i < k; i++ {
		a := randArg(r)
		specs[i] = a.spec
		vals[i] = a.value
	}
	return strings.Join(specs, ";"), vals
}

// ---------------------------------------------------------------------------
// random format strings
// ---------------------------------------------------------------------------

const verbs = "vsqdxXoObcUeEfFgGtTn%z"
const flagChars = "-+# 0"

func randFormat(r *rand.Rand) string {
	var b strings.Builder
	parts := 1 + r.Intn(3)
	for i := 0; i < parts; i++ {
		// literal chunk
		switch r.Intn(4) {
		case 0:
			b.WriteString("lit ")
		case 1:
			b.WriteString("世")
		}
		if r.Intn(12) == 0 {
			// a deliberately malformed tail
			switch r.Intn(4) {
			case 0:
				b.WriteString("%")
			case 1:
				b.WriteString("%!")
			case 2:
				b.WriteString("%[")
			case 3:
				b.WriteString("%.")
			}
			continue
		}
		b.WriteByte('%')
		// explicit argument index
		if r.Intn(10) == 0 {
			b.WriteString("[" + strconv.Itoa(1+r.Intn(4)) + "]")
		}
		// flags
		nf := r.Intn(3)
		for j := 0; j < nf; j++ {
			b.WriteByte(flagChars[r.Intn(len(flagChars))])
		}
		// width
		switch r.Intn(5) {
		case 0:
			b.WriteString(strconv.Itoa(r.Intn(14)))
		case 1:
			b.WriteByte('*')
		}
		// precision
		switch r.Intn(5) {
		case 0:
			b.WriteString("." + strconv.Itoa(r.Intn(9)))
		case 1:
			b.WriteString(".*")
		case 2:
			b.WriteString(".")
		}
		b.WriteByte(verbs[r.Intn(len(verbs))])
	}
	return b.String()
}

// ---------------------------------------------------------------------------

func writeFmtCorpus(path string, r *rand.Rand, count int) error {
	f, err := os.Create(path)
	if err != nil {
		return err
	}
	defer f.Close()
	w := bufio.NewWriter(f)
	defer w.Flush()

	for i := 0; i < count; i++ {
		spec, vals := randArgs(r)
		kind := "sprintf"
		format := randFormat(r)
		var produce func([]interface{}) string
		switch r.Intn(8) {
		case 0:
			kind, format = "sprint", ""
			produce = func(a []interface{}) string { return fmt.Sprint(a...) }
		case 1:
			kind, format = "sprintln", ""
			produce = func(a []interface{}) string { return fmt.Sprintln(a...) }
		default:
			f := format
			produce = func(a []interface{}) string { return fmt.Sprintf(f, a...) }
		}
		// Drop any case whose Go output embeds a pointer address.
		if !isDeterministic(vals, produce) {
			continue
		}
		res := produce(vals)
		fmt.Fprintf(w, "%s\t%s\t%s\t%s\n", kind,
			hex.EncodeToString([]byte(format)), spec, hex.EncodeToString([]byte(res)))
	}
	return nil
}

var allAttrs = []color.Attribute{
	color.Reset, color.Bold, color.Faint, color.Italic, color.Underline,
	color.BlinkSlow, color.BlinkRapid, color.ReverseVideo, color.Concealed,
	color.CrossedOut, color.ResetBold, color.ResetItalic, color.ResetUnderline,
	color.ResetBlinking, color.ResetReversed, color.ResetConcealed, color.ResetCrossedOut,
	color.FgBlack, color.FgRed, color.FgGreen, color.FgYellow, color.FgBlue,
	color.FgMagenta, color.FgCyan, color.FgWhite,
	color.FgHiBlack, color.FgHiRed, color.FgHiGreen, color.FgHiYellow, color.FgHiBlue,
	color.FgHiMagenta, color.FgHiCyan, color.FgHiWhite,
	color.BgBlack, color.BgRed, color.BgGreen, color.BgYellow, color.BgBlue,
	color.BgMagenta, color.BgCyan, color.BgWhite,
	color.BgHiBlack, color.BgHiRed, color.BgHiGreen, color.BgHiYellow, color.BgHiBlue,
	color.BgHiMagenta, color.BgHiCyan, color.BgHiWhite,
	38, 48, 2, 0, 255, 128,
}

func writeColorCorpus(path string, r *rand.Rand, count int) error {
	f, err := os.Create(path)
	if err != nil {
		return err
	}
	defer f.Close()
	w := bufio.NewWriter(f)
	defer w.Flush()

	ops := []string{"sprint", "sprintln", "sprintfunc", "sprintlnfunc",
		"fprint", "fprintln", "print", "println", "sprintf", "fprintf",
		"printf", "setwriter", "setunset"}

	for i := 0; i < count; i++ {
		k := r.Intn(6)
		attrs := make([]color.Attribute, k)
		for j := range attrs {
			attrs[j] = allAttrs[r.Intn(len(allAttrs))]
		}
		spec, vals := randArgs(r)
		format := randFormat(r)
		global := r.Intn(2) == 0
		override := r.Intn(3)
		op := ops[r.Intn(len(ops))]

		color.NoColor = global

		run := func(vals []interface{}) ([]byte, int) {
			c := color.New(attrs...)
			switch override {
			case 1:
				c.DisableColor()
			case 2:
				c.EnableColor()
			}

			var result []byte
			nn := -1
			var buf bytes.Buffer
			switch op {
			case "sprint":
				result = []byte(c.Sprint(vals...))
			case "sprintln":
				result = []byte(c.Sprintln(vals...))
			case "sprintfunc":
				result = []byte(c.SprintFunc()(vals...))
			case "sprintlnfunc":
				result = []byte(c.SprintlnFunc()(vals...))
			case "sprintf":
				result = []byte(c.Sprintf(format, vals...))
			case "fprint":
				nn, _ = c.Fprint(&buf, vals...)
				result = buf.Bytes()
			case "fprintln":
				nn, _ = c.Fprintln(&buf, vals...)
				result = buf.Bytes()
			case "fprintf":
				nn, _ = c.Fprintf(&buf, format, vals...)
				result = buf.Bytes()
			case "print":
				color.Output = &buf
				nn, _ = c.Print(vals...)
				result = buf.Bytes()
			case "println":
				color.Output = &buf
				nn, _ = c.Println(vals...)
				result = buf.Bytes()
			case "printf":
				color.Output = &buf
				nn, _ = c.Printf(format, vals...)
				result = buf.Bytes()
			case "setwriter":
				c.SetWriter(&buf)
				buf.WriteString("X")
				c.UnsetWriter(&buf)
				result = buf.Bytes()
			case "setunset":
				color.Output = &buf
				c.Set()
				buf.WriteString("X")
				color.Unset()
				result = buf.Bytes()
			}
			return result, nn
		}

		// Drop any case whose Go output embeds a pointer address.
		result, nn := run(vals)
		again, nnAgain := run(refreshArgs(vals))
		if !bytes.Equal(result, again) || nn != nnAgain {
			continue
		}

		attrSpec := make([]string, len(attrs))
		for j, a := range attrs {
			attrSpec[j] = strconv.Itoa(int(a))
		}
		fmt.Fprintf(w, "%s\t%s\t%s\t%s\t%v\t%d\t%s\t%d\n",
			strings.Join(attrSpec, ","), op,
			hex.EncodeToString([]byte(format)), spec,
			global, override, hex.EncodeToString(result), nn)
	}
	return nil
}

func main() {
	flag.Parse()
	r := rand.New(rand.NewSource(*seed))

	if *fmtOut != "" {
		if err := writeFmtCorpus(*fmtOut, r, *n); err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
	}
	if *colorOut != "" {
		if err := writeColorCorpus(*colorOut, r, *n); err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
	}
}
