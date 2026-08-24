// Command color_golden generates the golden reference file used to verify that
// the Rust port of fatih/color produces byte-identical output to the original
// Go package.
//
// It sweeps a matrix of
//
//	attribute sets × operations × argument lists × global NoColor × local override
//
// and records, for every combination, the exact bytes produced and — for the
// operations that return `(n int, err error)` — the reported byte count.
//
// Usage:
//
//	go run ./tools/color_golden > tests/testdata/color_golden.tsv
package main

import (
	"bufio"
	"bytes"
	"encoding/hex"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/fatih/color"
)

// ---------------------------------------------------------------------------
// argspec encoding (identical to tools/gofmt_golden)
// ---------------------------------------------------------------------------

type arg struct {
	spec  string
	value interface{}
}

func str(s string) arg { return arg{"s:" + hex.EncodeToString([]byte(s)), s} }
func i(v int) arg      { return arg{"i:" + strconv.Itoa(v), v} }
func b(v bool) arg {
	if v {
		return arg{"b:1", true}
	}
	return arg{"b:0", false}
}
func nilArg() arg { return arg{"n", nil} }

func encodeArgs(args []arg) (string, []interface{}) {
	specs := make([]string, len(args))
	vals := make([]interface{}, len(args))
	for k, a := range args {
		specs[k] = a.spec
		vals[k] = a.value
	}
	return strings.Join(specs, ";"), vals
}

func encodeAttrs(attrs []color.Attribute) string {
	parts := make([]string, len(attrs))
	for k, a := range attrs {
		parts[k] = strconv.Itoa(int(a))
	}
	return strings.Join(parts, ",")
}

func main() {
	out := bufio.NewWriter(os.Stdout)
	defer out.Flush()

	// -- attribute matrix ---------------------------------------------------
	attrSets := [][]color.Attribute{
		{},
		{color.FgBlack}, {color.FgRed}, {color.FgGreen}, {color.FgYellow},
		{color.FgBlue}, {color.FgMagenta}, {color.FgCyan}, {color.FgWhite},
		{color.FgHiBlack}, {color.FgHiRed}, {color.FgHiGreen}, {color.FgHiYellow},
		{color.FgHiBlue}, {color.FgHiMagenta}, {color.FgHiCyan}, {color.FgHiWhite},
		{color.BgBlack}, {color.BgRed}, {color.BgGreen}, {color.BgYellow},
		{color.BgBlue}, {color.BgMagenta}, {color.BgCyan}, {color.BgWhite},
		{color.BgHiBlack}, {color.BgHiRed}, {color.BgHiGreen}, {color.BgHiYellow},
		{color.BgHiBlue}, {color.BgHiMagenta}, {color.BgHiCyan}, {color.BgHiWhite},
		{color.Reset}, {color.Bold}, {color.Faint}, {color.Italic}, {color.Underline},
		{color.BlinkSlow}, {color.BlinkRapid}, {color.ReverseVideo}, {color.Concealed},
		{color.CrossedOut},
		{color.ResetBold}, {color.ResetItalic}, {color.ResetUnderline},
		{color.ResetBlinking}, {color.ResetReversed}, {color.ResetConcealed},
		{color.ResetCrossedOut},
		// combinations exercising the per-attribute reset table
		{color.FgRed, color.Bold},
		{color.FgRed, color.Underline},
		{color.FgWhite, color.BgGreen},
		{color.FgCyan, color.Underline, color.Bold},
		{color.Bold, color.Faint, color.Italic, color.Underline},
		{color.BlinkSlow, color.BlinkRapid, color.ReverseVideo, color.Concealed, color.CrossedOut},
		{color.FgRed, color.FgRed},
		{color.FgRed, color.Bold, color.FgRed},
		// 24-bit RGB
		{38, 2, 255, 128, 0},
		{48, 2, 230, 42, 42},
		{38, 2, 255, 128, 0, 48, 2, 0, 0, 0},
		{48, 2, 230, 42, 42, 38, 2, 255, 255, 255},
	}

	// -- argument matrix ----------------------------------------------------
	argSets := [][]arg{
		{},
		{str("")},
		{str("foo")},
		{str("foo"), str("bar")},
		{str("foo"), i(1)},
		{i(1), i(2)},
		{str("word1"), str("word2"), str("word3"), str("word4\n")},
		{str("multi\nline")},
		{str("héllo")},
		{str("%s")},
		{nilArg()},
		{b(true)},
		{str("hello"), str("world"), i(123)},
	}

	formats := []string{"%s", "%v", "%d", "%-7s %-7s %5d\n", "no verbs", "%s\n", ""}

	// override: 0 = untouched, 1 = DisableColor, 2 = EnableColor
	overrides := []int{0, 1, 2}
	globals := []bool{false, true}

	emit := func(attrs []color.Attribute, op, format, argspec string, global bool, override int, result []byte, n int) {
		fmt.Fprintf(out, "%s\t%s\t%s\t%s\t%v\t%d\t%s\t%d\n",
			encodeAttrs(attrs), op,
			hex.EncodeToString([]byte(format)), argspec,
			global, override,
			hex.EncodeToString(result), n,
		)
	}

	newColor := func(attrs []color.Attribute, override int) *color.Color {
		c := color.New(attrs...)
		switch override {
		case 1:
			c.DisableColor()
		case 2:
			c.EnableColor()
		}
		return c
	}

	for _, global := range globals {
		color.NoColor = global
		for _, override := range overrides {
			for _, attrs := range attrSets {
				for _, args := range argSets {
					argspec, vals := encodeArgs(args)

					// --- string-returning operations -------------------
					c := newColor(attrs, override)
					emit(attrs, "sprint", "", argspec, global, override, []byte(c.Sprint(vals...)), -1)

					c = newColor(attrs, override)
					emit(attrs, "sprintln", "", argspec, global, override, []byte(c.Sprintln(vals...)), -1)

					c = newColor(attrs, override)
					emit(attrs, "sprintfunc", "", argspec, global, override, []byte(c.SprintFunc()(vals...)), -1)

					c = newColor(attrs, override)
					emit(attrs, "sprintlnfunc", "", argspec, global, override, []byte(c.SprintlnFunc()(vals...)), -1)

					// --- writer operations -----------------------------
					c = newColor(attrs, override)
					var buf bytes.Buffer
					n, _ := c.Fprint(&buf, vals...)
					emit(attrs, "fprint", "", argspec, global, override, buf.Bytes(), n)

					c = newColor(attrs, override)
					buf.Reset()
					n, _ = c.Fprintln(&buf, vals...)
					emit(attrs, "fprintln", "", argspec, global, override, buf.Bytes(), n)

					// --- Print/Println through the global Output --------
					c = newColor(attrs, override)
					var outBuf bytes.Buffer
					color.Output = &outBuf
					n, _ = c.Print(vals...)
					emit(attrs, "print", "", argspec, global, override, outBuf.Bytes(), n)

					c = newColor(attrs, override)
					outBuf.Reset()
					color.Output = &outBuf
					n, _ = c.Println(vals...)
					emit(attrs, "println", "", argspec, global, override, outBuf.Bytes(), n)

					// --- format-string operations ----------------------
					for _, format := range formats {
						c = newColor(attrs, override)
						emit(attrs, "sprintf", format, argspec, global, override, []byte(c.Sprintf(format, vals...)), -1)

						c = newColor(attrs, override)
						buf.Reset()
						n, _ = c.Fprintf(&buf, format, vals...)
						emit(attrs, "fprintf", format, argspec, global, override, buf.Bytes(), n)

						c = newColor(attrs, override)
						outBuf.Reset()
						color.Output = &outBuf
						n, _ = c.Printf(format, vals...)
						emit(attrs, "printf", format, argspec, global, override, outBuf.Bytes(), n)
					}

					// --- SetWriter / UnsetWriter ------------------------
					c = newColor(attrs, override)
					buf.Reset()
					c.SetWriter(&buf)
					buf.WriteString("X")
					c.UnsetWriter(&buf)
					emit(attrs, "setwriter", "", argspec, global, override, buf.Bytes(), -1)
				}

				// --- Set()/Unset() through the global Output ------------
				outBuf := &bytes.Buffer{}
				color.Output = outBuf
				c := newColor(attrs, override)
				c.Set()
				outBuf.WriteString("X")
				color.Unset()
				emit(attrs, "setunset", "", "", global, override, outBuf.Bytes(), -1)
			}
		}
	}

	// -- package level helper functions -------------------------------------
	color.NoColor = false
	type helper struct {
		name string
		fn   func(string, ...interface{}) string
	}
	helpers := []helper{
		{"BlackString", color.BlackString},
		{"RedString", color.RedString},
		{"GreenString", color.GreenString},
		{"YellowString", color.YellowString},
		{"BlueString", color.BlueString},
		{"MagentaString", color.MagentaString},
		{"CyanString", color.CyanString},
		{"WhiteString", color.WhiteString},
		{"HiBlackString", color.HiBlackString},
		{"HiRedString", color.HiRedString},
		{"HiGreenString", color.HiGreenString},
		{"HiYellowString", color.HiYellowString},
		{"HiBlueString", color.HiBlueString},
		{"HiMagentaString", color.HiMagentaString},
		{"HiCyanString", color.HiCyanString},
		{"HiWhiteString", color.HiWhiteString},
	}
	for _, h := range helpers {
		for _, format := range formats {
			for _, args := range argSets {
				argspec, vals := encodeArgs(args)
				res := h.fn(format, vals...)
				fmt.Fprintf(out, "%s\t%s\t%s\t%s\t%v\t%d\t%s\t%d\n",
					"", "helper:"+h.name,
					hex.EncodeToString([]byte(format)), argspec,
					false, 0,
					hex.EncodeToString([]byte(res)), -1,
				)
			}
		}
	}

	// -- print helpers (Black, Red, ... ) writing to color.Output ------------
	type printHelper struct {
		name string
		fn   func(string, ...interface{})
	}
	printHelpers := []printHelper{
		{"Black", color.Black}, {"Red", color.Red}, {"Green", color.Green},
		{"Yellow", color.Yellow}, {"Blue", color.Blue}, {"Magenta", color.Magenta},
		{"Cyan", color.Cyan}, {"White", color.White},
		{"HiBlack", color.HiBlack}, {"HiRed", color.HiRed}, {"HiGreen", color.HiGreen},
		{"HiYellow", color.HiYellow}, {"HiBlue", color.HiBlue},
		{"HiMagenta", color.HiMagenta}, {"HiCyan", color.HiCyan}, {"HiWhite", color.HiWhite},
	}
	for _, h := range printHelpers {
		for _, format := range formats {
			for _, args := range argSets {
				argspec, vals := encodeArgs(args)
				outBuf := &bytes.Buffer{}
				color.Output = outBuf
				h.fn(format, vals...)
				fmt.Fprintf(out, "%s\t%s\t%s\t%s\t%v\t%d\t%s\t%d\n",
					"", "printhelper:"+h.name,
					hex.EncodeToString([]byte(format)), argspec,
					false, 0,
					hex.EncodeToString(outBuf.Bytes()), -1,
				)
			}
		}
	}

	// -- Equals -------------------------------------------------------------
	for _, a := range attrSets {
		for _, b := range attrSets {
			eq := color.New(a...).Equals(color.New(b...))
			fmt.Fprintf(out, "%s\t%s\t%s\t%s\t%v\t%d\t%s\t%d\n",
				encodeAttrs(a), "equals",
				hex.EncodeToString([]byte(encodeAttrs(b))), "",
				false, 0,
				hex.EncodeToString([]byte(fmt.Sprintf("%v", eq))), -1,
			)
		}
	}
}
