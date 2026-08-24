// Command gofmt_golden generates the golden reference file that the Rust
// implementation of Go's fmt verbs is verified against.
//
// It exhaustively evaluates a matrix of format strings against a set of
// representative argument lists using the *real* Go fmt package and writes one
// TSV record per case:
//
//	<kind>\t<format-hex>\t<argspec>\t<result-hex>
//
// The argument list is encoded in a self-describing "argspec" so that the Rust
// side can reconstruct the exact same arguments without the two case lists ever
// having to be kept in sync by hand.
//
// Usage:
//
//	go run ./tools/gofmt_golden > tests/testdata/gofmt_golden.tsv
package main

import (
	"bufio"
	"encoding/hex"
	"errors"
	"fmt"
	"math"
	"os"
	"strconv"
	"strings"
)

// ---------------------------------------------------------------------------
// argspec encoding
// ---------------------------------------------------------------------------
//
//	n            nil
//	b:0|1        bool
//	i:<dec>      int
//	u:<dec>      uint
//	f:<bits>     float64, as the hex of its IEEE-754 bit pattern (exact)
//	s:<hex>      string
//	r:<dec>      rune (int32)
//	y:<hex>      []byte
//	e:<hex>      error (errors.New)
//	l:<hex>      []interface{}, hex of a nested argspec
//
// Multiple arguments are separated by ';'. An empty argspec means no arguments.

type arg struct {
	spec  string
	value interface{}
}

func str(s string) arg {
	return arg{"s:" + hex.EncodeToString([]byte(s)), s}
}
func i(v int) arg {
	return arg{"i:" + strconv.Itoa(v), v}
}
func u(v uint) arg {
	return arg{"u:" + strconv.FormatUint(uint64(v), 10), v}
}
func f(v float64) arg {
	bits := fmt.Sprintf("%016x", math.Float64bits(v))
	return arg{"f:" + bits, v}
}
func b(v bool) arg {
	if v {
		return arg{"b:1", true}
	}
	return arg{"b:0", false}
}
func r(v rune) arg {
	return arg{"r:" + strconv.Itoa(int(v)), v}
}
func by(v []byte) arg {
	return arg{"y:" + hex.EncodeToString(v), v}
}
func er(s string) arg {
	return arg{"e:" + hex.EncodeToString([]byte(s)), errors.New(s)}
}
func nilArg() arg { return arg{"n", nil} }

func slice(items ...arg) arg {
	specs := make([]string, len(items))
	vals := make([]interface{}, len(items))
	for k, it := range items {
		specs[k] = it.spec
		vals[k] = it.value
	}
	inner := strings.Join(specs, ";")
	return arg{"l:" + hex.EncodeToString([]byte(inner)), vals}
}

func encodeArgs(args []arg) (string, []interface{}) {
	specs := make([]string, len(args))
	vals := make([]interface{}, len(args))
	for k, a := range args {
		specs[k] = a.spec
		vals[k] = a.value
	}
	return strings.Join(specs, ";"), vals
}

func main() {
	out := bufio.NewWriter(os.Stdout)
	defer out.Flush()

	emit := func(kind, format string, args []arg) {
		spec, vals := encodeArgs(args)
		var result string
		switch kind {
		case "sprintf":
			result = fmt.Sprintf(format, vals...)
		case "sprint":
			result = fmt.Sprint(vals...)
		case "sprintln":
			result = fmt.Sprintln(vals...)
		}
		fmt.Fprintf(out, "%s\t%s\t%s\t%s\n",
			kind,
			hex.EncodeToString([]byte(format)),
			spec,
			hex.EncodeToString([]byte(result)),
		)
	}

	// -- the argument matrix ------------------------------------------------
	argSets := [][]arg{
		{},
		{str("hello")},
		{str("")},
		{str("héllo wörld")},
		{str("tab\there")},
		{str("nl\nhere")},
		{str("\x1b[31mred\x1b[0m")},
		{str("%s")},
		{i(0)},
		{i(1)},
		{i(-1)},
		{i(42)},
		{i(-42)},
		{i(255)},
		{i(123456789)},
		{i(65)},
		{u(0)},
		{u(255)},
		{u(4294967295)},
		// Boundary values for fmt.intFromArg's `int64(n) >= 0 &&
		// uint64(int(n)) == n` round-trip check and for tooLarge().
		{u(uint(math.MaxUint64))},
		{u(1 << 63)},
		{i(math.MaxInt64)},
		{i(math.MinInt64)},
		{i(1000001)},
		{i(-1000001)},
		{i(1000000)},
		{f(0)},
		{f(1)},
		{f(-1)},
		{f(1.5)},
		{f(0.1)},
		{f(3.14159265358979)},
		{f(1e6)},
		{f(1e20)},
		{f(1e21)},
		{f(1e-5)},
		{f(-1.5e-7)},
		{f(123456.0)},
		{f(1234567.0)},
		{b(true)},
		{b(false)},
		{r('A')},
		{r('世')},
		{by([]byte("abc"))},
		{by([]byte{})},
		// NOTE: deliberately valid UTF-8. A Go string is an arbitrary byte
		// sequence whereas a Rust String is guaranteed UTF-8, so a []byte
		// holding invalid UTF-8 cannot round-trip through `%s`/`%q`. Every
		// string reaching the color API from Rust is UTF-8 by construction, so
		// the distinction is unobservable in practice.
		{by([]byte{0, 1, 127})},
		{er("boom")},
		{nilArg()},
		{slice(i(1), i(2), i(3))},
		{slice(str("a"), str("b"))},
		{slice()},
		// multi-argument sets exercise MISSING / EXTRA / spacing rules
		{str("a"), str("b")},
		{i(1), i(2)},
		{str("a"), i(1)},
		{i(1), str("a"), i(2)},
		{str("hello"), str("world"), i(123)},
		{nilArg(), str("x")},
		{i(1), i(2), i(3)},
		{f(1), f(2)},
		{b(true), str("x")},
	}

	// -- the format matrix --------------------------------------------------
	verbs := []string{"v", "s", "q", "d", "x", "X", "o", "O", "b", "c", "U", "e", "E", "f", "F", "g", "G", "t", "T"}
	flags := []string{"", "-", "+", "#", " ", "0", "+#", "- "}
	widths := []string{"", "1", "5", "12"}
	precs := []string{"", ".0", ".2", ".7"}

	var formats []string
	for _, v := range verbs {
		for _, fl := range flags {
			for _, w := range widths {
				for _, p := range precs {
					formats = append(formats, "%"+fl+w+p+v)
				}
			}
		}
	}

	// Hand-written edge cases covering the scanner itself.
	formats = append(formats,
		"",
		"plain text",
		"%%",
		"100%%",
		"%",
		"abc%",
		"%!",
		"%z",
		"%v %v",
		"%s-%s",
		"%[1]d",
		"%[2]d %[1]d",
		"%[1]d %[1]d",
		"%[3]d",
		"%[0]d",
		"%[d",
		"%*d",
		"%-*d",
		"%.*f",
		"%*.*f",
		"%*s",
		"%.*s",
		"%[1]5d",
		"%[1]*d",
		"%[1]d.%[1]d",
		"%[2]*d",
		"%v%v%v",
		"a%sb%dc",
		"%10.3s",
		"%-10.3s|",
		"%+.3d",
		"% .3d",
		"%08.3f",
		"%#v",
		"%+v",
		"%1000000000000d",
		"%.d",
		"%5.d",
		"%s %!",
		"%\t",
		"%世",
	)

	for _, format := range formats {
		for _, args := range argSets {
			emit("sprintf", format, args)
		}
	}
	for _, args := range argSets {
		emit("sprint", "", args)
		emit("sprintln", "", args)
	}
}
