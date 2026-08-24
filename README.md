# color

Color lets you use colorized outputs in terms of [ANSI Escape
Codes](http://en.wikipedia.org/wiki/ANSI_escape_code#Colors) in Rust. It has
support for Windows too! The API can be used in several ways, pick one that
suits you.

This crate is a **faithful, behavior-preserving port of
[github.com/fatih/color](https://github.com/fatih/color) v1.19.0**. Every
exported function, constant, type and side effect of the Go package has an
equivalent here, and the output is verified byte-for-byte against the original
Go implementation (see [Verification](#verification)).

## Install

```toml
[dependencies]
color = "1.19"
```

## Examples

### Standard colors

```rust
use color::{cyan, blue, red, magenta, vals, NO_ARGS};

// Print with default helper functions
cyan("Prints text in cyan.", NO_ARGS);

// A newline will be appended automatically
blue("Prints %s in blue.", &vals!["text"]);

// These are using the default foreground colors
red("We have red", NO_ARGS);
magenta("And many others ..", NO_ARGS);
```

### RGB colors

If your terminal supports 24-bit colors, you can use RGB color codes.

```rust
use color::{bg_rgb, rgb, vals};

rgb(255, 128, 0).println(&vals!["foreground orange"]).unwrap();
rgb(230, 42, 42).println(&vals!["foreground red"]).unwrap();

bg_rgb(255, 128, 0).println(&vals!["background orange"]).unwrap();
bg_rgb(230, 42, 42).println(&vals!["background red"]).unwrap();
```

### Mix and reuse colors

```rust
use color::{bg_rgb, rgb, vals, Color, BG_WHITE, BOLD, FG_CYAN, FG_RED, UNDERLINE};

// Create a new color object
let mut c = Color::new(&[FG_CYAN]);
c.add(&[UNDERLINE]);
c.println(&vals!["Prints cyan text with an underline."]).unwrap();

// Or just add them to new()
let d = Color::new(&[FG_CYAN, BOLD]);
d.printf("This prints bold cyan %s\n", &vals!["too!."]).unwrap();

// Mix up foreground and background colors, create new mixes!
let mut red = Color::new(&[FG_RED]);

// NB: like the Go original, `add` mutates the receiver and returns it.
let bold_red = red.add(&[BOLD]).clone();
bold_red.println(&vals!["This will print text in bold red."]).unwrap();

let white_background = red.add(&[BG_WHITE]).clone();
white_background.println(&vals!["Red text with white background."]).unwrap();

// Mix with RGB color codes
let mut c = rgb(255, 128, 0);
c.add_bg_rgb(0, 0, 0);
c.println(&vals!["orange with black background"]).unwrap();

let mut c = bg_rgb(255, 128, 0);
c.add_rgb(255, 255, 255);
c.println(&vals!["orange background with white foreground"]).unwrap();
```

### Use your own output (`io::Write`)

```rust
use color::{vals, Color, FG_BLUE};

let mut my_writer: Vec<u8> = Vec::new();

Color::new(&[FG_BLUE]).fprintln(&mut my_writer, &vals!["blue color!"]).unwrap();

let blue = Color::new(&[FG_BLUE]);
blue.fprint(&mut my_writer, &vals!["This will print text in blue."]).unwrap();
```

### Custom print functions (`print_func`)

```rust
use color::{vals, Color, BOLD, FG_GREEN, FG_RED};

// Create a custom print function for convenience
let red = Color::new(&[FG_RED]).printf_func();
red("Warning", &[]);
red("Error: %s", &vals!["disk full"]);

// Mix up multiple attributes
let notice = Color::new(&[BOLD, FG_GREEN]).println_func();
notice(&vals!["Don't forget this..."]);
```

### Custom fprint functions (`fprint_func`)

```rust
use color::{vals, Color, BOLD, FG_BLUE, FG_GREEN};

let mut my_writer: Vec<u8> = Vec::new();

let blue = Color::new(&[FG_BLUE]).fprintf_func();
blue(&mut my_writer, "important notice: %s", &vals!["***"]);

// Mix up with multiple attributes
let success = Color::new(&[BOLD, FG_GREEN]).fprintln_func();
success(&mut my_writer, &vals!["Don't forget this..."]);
```

### Insert into noncolor strings (`sprint_func`)

```rust
use color::{gofmt, green_string, red_string, vals, Color, BG_GREEN, FG_RED, FG_WHITE, FG_YELLOW, NO_ARGS};
use std::io::Write;

// Create sprint_xxx functions to mix strings with other non-colorized strings:
let yellow = Color::new(&[FG_YELLOW]).sprint_func();
let red = Color::new(&[FG_RED]).sprint_func();
print!("{}", gofmt::sprintf(
    "This is a %s and this is %s.\n",
    &vals![yellow(&vals!["warning"]), red(&vals!["error"])],
));

let info = Color::new(&[FG_WHITE, BG_GREEN]).sprint_func();
print!("{}", gofmt::sprintf("This %s rocks!\n", &vals![info(&vals!["package"])]));

// Use helper functions
println!("This {} should be not neglected.", red_string("warning", NO_ARGS));

// Windows supported too! Just don't forget to change the output to color::output()
let mut out = color::output();
write!(out, "{}", gofmt::sprintf("Windows support: %s", &vals![green_string("PASS", NO_ARGS)])).unwrap();
```

### Plug into existing code

```rust
use color::{set, unset, BOLD, FG_MAGENTA, FG_YELLOW};

// Use handy standard colors
set(&[FG_YELLOW]);

println!("Existing text will now be in yellow");
println!("This one {}", "too");

unset(); // Don't forget to unset

// You can mix up parameters
set(&[FG_MAGENTA, BOLD]);
println!("All text will now be bold magenta.");
unset();
```

### Disable/Enable color

There might be a case where you want to explicitly disable/enable color output.
Colorized output is automatically disabled for non-tty output streams (for
example if the output were piped directly to `less`).

The crate also disables color output if the [`NO_COLOR`](https://no-color.org)
environment variable is set to a non-empty string.

`Color` has support to disable/enable colors programmatically both globally and
for single color definitions. For example suppose you have a CLI app and a
`--no-color` bool flag. You can easily disable the color output with:

```rust
use color::set_no_color;

let flag_no_color = true;
if flag_no_color {
    set_no_color(true); // disables colorized output
}
```

It also has support for single color definitions (local). You can
disable/enable color output on the fly:

```rust
use color::{vals, Color, FG_CYAN};

let mut c = Color::new(&[FG_CYAN]);
c.println(&vals!["Prints cyan text"]).unwrap();

c.disable_color();
c.println(&vals!["This is printed without any color"]).unwrap();

c.enable_color();
c.println(&vals!["This prints again cyan..."]).unwrap();
```

## GitHub Actions

To output color in GitHub Actions (or other CI systems that support ANSI
colors), make sure to call `color::set_no_color(false)` so that it bypasses the
check for non-tty output streams.

## Mapping from the Go API

| Go                              | Rust                                    |
|---------------------------------|-----------------------------------------|
| `color.New(FgRed, Bold)`        | `Color::new(&[FG_RED, BOLD])`           |
| `c.Add(Underline)`              | `c.add(&[UNDERLINE])`                   |
| `c.Sprintf(f, a...)`            | `c.sprintf(f, &vals![a])`               |
| `c.SprintFunc()`                | `c.sprint_func()`                       |
| `color.RedString("x")`          | `color::red_string("x", NO_ARGS)`       |
| `color.NoColor = true`          | `color::set_no_color(true)`             |
| `color.Output = w`              | `color::set_output(w)`                  |
| `color.Output`                  | `color::output()`                       |
| `io.Writer`                     | `impl std::io::Write` / `Writer`        |
| `...interface{}`                | `&[Value]`, built with `vals![]`        |
| `(n int, err error)`            | `Result<usize>` (`WriteError` keeps `n`)|

Go's variadic `...interface{}` has no direct Rust equivalent, so print
arguments are modelled by the [`Value`] enum and built with the `vals!` macro.
`Value` carries the Go *dynamic type* alongside the data, which is what allows
`fmt`'s spacing rules (`Sprint` only separates non-strings) and its diagnostics
(`%!d(string=foo)`) to be reproduced exactly.

Format strings are Go format strings, interpreted by the bundled [`gofmt`]
module — not Rust's `format!` syntax.

## Verification

Behavioral equivalence is not assumed, it is measured. Two generators under
`tools/` drive the real Go code and dump golden files that the Rust test-suite
replays:

| Harness                  | Cases     | Covers                                                                       |
|--------------------------|-----------|------------------------------------------------------------------------------|
| `tools/gofmt_golden`     | 148,500   | Go `fmt` verbs × flags × widths × precisions × argument types                 |
| `tools/color_golden`     | 152,208   | attribute sets × operations × arguments × `NoColor` × per-color overrides     |
| `tools/fuzz_parity`      | unbounded | randomized formats, arguments and attribute sets — inputs nobody hand-picked  |
| `tests/color_test.rs`    | 22        | a 1:1 port of every test in `color_test.go`                                   |

Regenerate the golden files (requires Go) with:

```sh
cd tools/gofmt_golden && go run . > ../../tests/testdata/gofmt_golden.tsv
cd tools/color_golden && go run . > ../../tests/testdata/color_golden.tsv
```

Then:

```sh
cargo test
```

The fixed matrices are deterministic, so the `parity` CI job regenerates them
from Go and fails on any drift. To go beyond them, generate a randomized corpus
and point the replay tests at it:

```sh
cd tools/fuzz_parity && go run . -seed 1 -n 20000 -fmt /tmp/f.tsv -color /tmp/c.tsv
GOFMT_GOLDEN=/tmp/f.tsv COLOR_GOLDEN=/tmp/c.tsv cargo test --test gofmt_parity --test color_parity
```

`fuzz_parity` evaluates each candidate twice, against two equal-valued but
separately allocated argument lists, and drops any case whose Go output differs
between the two — that is, any case that records a pointer address (see
`%p` below). Such a case has no stable reference value in Go either.

## Known representational differences

These stem from language-level differences and are unobservable through the
public API:

* **Strings.** A Go `string` is an arbitrary byte sequence; a Rust `String` is
  guaranteed UTF-8. `Value::Bytes` holding invalid UTF-8 cannot round-trip
  through `%s`/`%q`. Every string entering this crate is UTF-8 by construction.
* **Pointer addresses.** Go prints a memory address for `%p`, and — through
  `fmtPointer` — for `%d`/`%b`/`%o`/`%#v` applied to a pointer-shaped value
  reached by reflection, which in this crate means a `Value::Error` nested
  inside a `Value::Slice`. Addresses are neither meaningful nor stable here (Go
  itself prints a different one on every run), so those cases report the
  `%!verb(type=value)` diagnostic instead. Every other verb, including the
  same error at the top level or under `%v`/`%s`/`%q`/`%x`/`%X`, is
  byte-identical to Go.
* **Narrow integers.** Rust's `i8`/`i16`/`u16`/`u32`/`f32` widen to Go's
  `int`/`uint`/`float64` when converted to a `Value`, so `%T` reports the
  widened name. `u8` maps to Go's `uint8` because `[]byte` elements depend on
  it.
* **`unicode.IsPrint`.** Approximated with `char` classification from `std`
  rather than embedding Unicode tables; identical for all ASCII and for the
  common ranges.

## Credits

* [Fatih Arslan](https://github.com/fatih) — author of the original Go package
* Windows support via @mattn: [colorable](https://github.com/mattn/go-colorable)

## License

The MIT License (MIT) — see [`LICENSE.md`](LICENSE.md) for more details.

[`Value`]: https://docs.rs/color/latest/color/enum.Value.html
[`gofmt`]: https://docs.rs/color/latest/color/gofmt/index.html
