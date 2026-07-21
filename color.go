package color

import (
	"io"
	"os"
	"sync"
)

var (
	NoColor = noColorIsSet() || os.Getenv("TERM") == "dumb" || !stdoutIsTerminal()

	Output = stdOut()

	Error = stdErr()

	colorsCache   = make(map[Attribute]*Color)
	colorsCacheMu sync.Mutex
)

func noColorIsSet() bool { _ = "STUB: not implemented"; return false }

func stdoutIsTerminal() bool { _ = "STUB: not implemented"; return false }

func stdOut() io.Writer { _ = "STUB: not implemented"; return *new(io.Writer) }

func stdErr() io.Writer { _ = "STUB: not implemented"; return *new(io.Writer) }

type Color struct {
	params  []Attribute
	noColor *bool
}

type Attribute int

const escape = "\x1b"

const (
	Reset Attribute = iota
	Bold
	Faint
	Italic
	Underline
	BlinkSlow
	BlinkRapid
	ReverseVideo
	Concealed
	CrossedOut
)

const (
	ResetBold Attribute = iota + 22
	ResetItalic
	ResetUnderline
	ResetBlinking
	_
	ResetReversed
	ResetConcealed
	ResetCrossedOut
)

var mapResetAttributes map[Attribute]Attribute = map[Attribute]Attribute{
	Bold:         ResetBold,
	Faint:        ResetBold,
	Italic:       ResetItalic,
	Underline:    ResetUnderline,
	BlinkSlow:    ResetBlinking,
	BlinkRapid:   ResetBlinking,
	ReverseVideo: ResetReversed,
	Concealed:    ResetConcealed,
	CrossedOut:   ResetCrossedOut,
}

const (
	FgBlack Attribute = iota + 30
	FgRed
	FgGreen
	FgYellow
	FgBlue
	FgMagenta
	FgCyan
	FgWhite

	foreground
)

const (
	FgHiBlack Attribute = iota + 90
	FgHiRed
	FgHiGreen
	FgHiYellow
	FgHiBlue
	FgHiMagenta
	FgHiCyan
	FgHiWhite
)

const (
	BgBlack Attribute = iota + 40
	BgRed
	BgGreen
	BgYellow
	BgBlue
	BgMagenta
	BgCyan
	BgWhite

	background
)

const (
	BgHiBlack Attribute = iota + 100
	BgHiRed
	BgHiGreen
	BgHiYellow
	BgHiBlue
	BgHiMagenta
	BgHiCyan
	BgHiWhite
)

func New(value ...Attribute) *Color { _ = "STUB: not implemented"; return nil }

func RGB(r, g, b int) *Color { _ = "STUB: not implemented"; return nil }

func BgRGB(r, g, b int) *Color { _ = "STUB: not implemented"; return nil }

func (c *Color) AddRGB(r, g, b int) *Color { _ = "STUB: not implemented"; return nil }

func (c *Color) AddBgRGB(r, g, b int) *Color { _ = "STUB: not implemented"; return nil }

func Set(p ...Attribute) *Color { _ = "STUB: not implemented"; return nil }

func Unset() { _ = "STUB: not implemented"; return }

func (c *Color) Set() *Color { _ = "STUB: not implemented"; return nil }

func (c *Color) unset() { _ = "STUB: not implemented"; return }

func (c *Color) SetWriter(w io.Writer) *Color { _ = "STUB: not implemented"; return nil }

func (c *Color) setWriter(w io.Writer) (int, error) { _ = "STUB: not implemented"; return 0, nil }

func (c *Color) UnsetWriter(w io.Writer) { _ = "STUB: not implemented"; return }

func (c *Color) unsetWriter(w io.Writer) (int, error) { _ = "STUB: not implemented"; return 0, nil }

func (c *Color) Add(value ...Attribute) *Color { _ = "STUB: not implemented"; return nil }

func (c *Color) Fprint(w io.Writer, a ...interface{}) (n int, err error) {
	_ = "STUB: not implemented"
	return 0, nil
}

func (c *Color) Print(a ...interface{}) (n int, err error) {
	_ = "STUB: not implemented"
	return 0, nil
}

func (c *Color) Fprintf(w io.Writer, format string, a ...interface{}) (n int, err error) {
	_ = "STUB: not implemented"
	return 0, nil
}

func (c *Color) Printf(format string, a ...interface{}) (n int, err error) {
	_ = "STUB: not implemented"
	return 0, nil
}

func (c *Color) Fprintln(w io.Writer, a ...interface{}) (n int, err error) {
	_ = "STUB: not implemented"
	return 0, nil
}

func (c *Color) Println(a ...interface{}) (n int, err error) {
	_ = "STUB: not implemented"
	return 0, nil
}

func (c *Color) Sprint(a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func (c *Color) Sprintln(a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func (c *Color) Sprintf(format string, a ...interface{}) string {
	_ = "STUB: not implemented"
	return ""
}

func (c *Color) FprintFunc() func(w io.Writer, a ...interface{}) {
	_ = "STUB: not implemented"
	return nil
}

func (c *Color) PrintFunc() func(a ...interface{}) { _ = "STUB: not implemented"; return nil }

func (c *Color) FprintfFunc() func(w io.Writer, format string, a ...interface{}) {
	_ = "STUB: not implemented"
	return nil
}

func (c *Color) PrintfFunc() func(format string, a ...interface{}) {
	_ = "STUB: not implemented"
	return nil
}

func (c *Color) FprintlnFunc() func(w io.Writer, a ...interface{}) {
	_ = "STUB: not implemented"
	return nil
}

func (c *Color) PrintlnFunc() func(a ...interface{}) { _ = "STUB: not implemented"; return nil }

func (c *Color) SprintFunc() func(a ...interface{}) string { _ = "STUB: not implemented"; return nil }

func (c *Color) SprintfFunc() func(format string, a ...interface{}) string {
	_ = "STUB: not implemented"
	return nil
}

func (c *Color) SprintlnFunc() func(a ...interface{}) string { _ = "STUB: not implemented"; return nil }

func (c *Color) sequence() string { _ = "STUB: not implemented"; return "" }

func (c *Color) wrap(s string) string { _ = "STUB: not implemented"; return "" }

func (c *Color) format() string { _ = "STUB: not implemented"; return "" }

func (c *Color) unformat() string { _ = "STUB: not implemented"; return "" }

func (c *Color) DisableColor() { _ = "STUB: not implemented"; return }

func (c *Color) EnableColor() { _ = "STUB: not implemented"; return }

func (c *Color) isNoColorSet() bool { _ = "STUB: not implemented"; return false }

func (c *Color) Equals(c2 *Color) bool { _ = "STUB: not implemented"; return false }

func boolPtr(v bool) *bool { _ = "STUB: not implemented"; return nil }

func getCachedColor(p Attribute) *Color { _ = "STUB: not implemented"; return nil }

func colorPrint(format string, p Attribute, a ...interface{}) { _ = "STUB: not implemented"; return }

func colorString(format string, p Attribute, a ...interface{}) string {
	_ = "STUB: not implemented"
	return ""
}

func Black(format string, a ...interface{}) { _ = "STUB: not implemented"; return }

func Red(format string, a ...interface{}) { _ = "STUB: not implemented"; return }

func Green(format string, a ...interface{}) { _ = "STUB: not implemented"; return }

func Yellow(format string, a ...interface{}) { _ = "STUB: not implemented"; return }

func Blue(format string, a ...interface{}) { _ = "STUB: not implemented"; return }

func Magenta(format string, a ...interface{}) { _ = "STUB: not implemented"; return }

func Cyan(format string, a ...interface{}) { _ = "STUB: not implemented"; return }

func White(format string, a ...interface{}) { _ = "STUB: not implemented"; return }

func BlackString(format string, a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func RedString(format string, a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func GreenString(format string, a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func YellowString(format string, a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func BlueString(format string, a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func MagentaString(format string, a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func CyanString(format string, a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func WhiteString(format string, a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func HiBlack(format string, a ...interface{}) { _ = "STUB: not implemented"; return }

func HiRed(format string, a ...interface{}) { _ = "STUB: not implemented"; return }

func HiGreen(format string, a ...interface{}) { _ = "STUB: not implemented"; return }

func HiYellow(format string, a ...interface{}) { _ = "STUB: not implemented"; return }

func HiBlue(format string, a ...interface{}) { _ = "STUB: not implemented"; return }

func HiMagenta(format string, a ...interface{}) { _ = "STUB: not implemented"; return }

func HiCyan(format string, a ...interface{}) { _ = "STUB: not implemented"; return }

func HiWhite(format string, a ...interface{}) { _ = "STUB: not implemented"; return }

func HiBlackString(format string, a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func HiRedString(format string, a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func HiGreenString(format string, a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func HiYellowString(format string, a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func HiBlueString(format string, a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func HiMagentaString(format string, a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func HiCyanString(format string, a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func HiWhiteString(format string, a ...interface{}) string { _ = "STUB: not implemented"; return "" }

func sprintln(a ...interface{}) string { _ = "STUB: not implemented"; return "" }
