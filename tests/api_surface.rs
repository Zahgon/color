//! API-completeness check.
//!
//! Names every exported identifier of `github.com/fatih/color` v1.19.0 and its
//! Rust counterpart. This file exists to fail *compilation* if any part of the
//! ported surface is removed, renamed or changes shape.
//!
//! The Go declarations are quoted in comments so the mapping can be audited
//! against the original source.

use std::io::Write;
use std::sync::{Mutex, MutexGuard, OnceLock};

use color::*;

/// Serializes the tests that mutate the package globals and points `Output` at
/// a scratch buffer so the checks never write to the real stdout.
fn serial() -> (MutexGuard<'static, ()>, SharedBuffer) {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let guard = LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let scratch = SharedBuffer::new();
    set_output(scratch.writer());
    (guard, scratch)
}

/// `type Attribute int` and all four constant blocks.
#[test]
fn attributes_are_complete() {
    // Base attributes
    let base: [Attribute; 10] = [
        RESET,
        BOLD,
        FAINT,
        ITALIC,
        UNDERLINE,
        BLINK_SLOW,
        BLINK_RAPID,
        REVERSE_VIDEO,
        CONCEALED,
        CROSSED_OUT,
    ];
    assert_eq!(base.map(|a| a.code()), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);

    // Reset attributes (note: 26 is the blank `_` identifier in Go)
    let resets: [Attribute; 7] = [
        RESET_BOLD,
        RESET_ITALIC,
        RESET_UNDERLINE,
        RESET_BLINKING,
        RESET_REVERSED,
        RESET_CONCEALED,
        RESET_CROSSED_OUT,
    ];
    assert_eq!(resets.map(|a| a.code()), [22, 23, 24, 25, 27, 28, 29]);

    // Foreground
    let fg: [Attribute; 8] = [
        FG_BLACK, FG_RED, FG_GREEN, FG_YELLOW, FG_BLUE, FG_MAGENTA, FG_CYAN, FG_WHITE,
    ];
    assert_eq!(fg.map(|a| a.code()), [30, 31, 32, 33, 34, 35, 36, 37]);

    // Foreground hi-intensity
    let fg_hi: [Attribute; 8] = [
        FG_HI_BLACK,
        FG_HI_RED,
        FG_HI_GREEN,
        FG_HI_YELLOW,
        FG_HI_BLUE,
        FG_HI_MAGENTA,
        FG_HI_CYAN,
        FG_HI_WHITE,
    ];
    assert_eq!(fg_hi.map(|a| a.code()), [90, 91, 92, 93, 94, 95, 96, 97]);

    // Background
    let bg: [Attribute; 8] = [
        BG_BLACK, BG_RED, BG_GREEN, BG_YELLOW, BG_BLUE, BG_MAGENTA, BG_CYAN, BG_WHITE,
    ];
    assert_eq!(bg.map(|a| a.code()), [40, 41, 42, 43, 44, 45, 46, 47]);

    // Background hi-intensity
    let bg_hi: [Attribute; 8] = [
        BG_HI_BLACK,
        BG_HI_RED,
        BG_HI_GREEN,
        BG_HI_YELLOW,
        BG_HI_BLUE,
        BG_HI_MAGENTA,
        BG_HI_CYAN,
        BG_HI_WHITE,
    ];
    assert_eq!(
        bg_hi.map(|a| a.code()),
        [100, 101, 102, 103, 104, 105, 106, 107]
    );
}

/// Every method on `*Color`.
#[test]
fn color_methods_are_complete() {
    let (_g, _scratch) = serial();
    let mut sink: Vec<u8> = Vec::new();
    let args = vals!["x"];

    // func New(value ...Attribute) *Color
    let mut c = Color::new(&[FG_RED]);

    // func (c *Color) Add(value ...Attribute) *Color
    let _: &mut Color = c.add(&[BOLD]);
    // func (c *Color) AddRGB(r, g, b int) *Color
    let _: &mut Color = c.add_rgb(1, 2, 3);
    // func (c *Color) AddBgRGB(r, g, b int) *Color
    let _: &mut Color = c.add_bg_rgb(1, 2, 3);

    // func (c *Color) Set() *Color
    let _: &Color = c.set();
    // func (c *Color) SetWriter(w io.Writer) *Color
    let _: &Color = c.set_writer(&mut sink);
    // func (c *Color) UnsetWriter(w io.Writer)
    c.unset_writer(&mut sink);

    // func (c *Color) Fprint(w io.Writer, a ...interface{}) (int, error)
    let _: Result<usize> = c.fprint(&mut sink, &args);
    // func (c *Color) Fprintf(w io.Writer, format string, a ...interface{}) (int, error)
    let _: Result<usize> = c.fprintf(&mut sink, "%s", &args);
    // func (c *Color) Fprintln(w io.Writer, a ...interface{}) (int, error)
    let _: Result<usize> = c.fprintln(&mut sink, &args);

    // func (c *Color) Print / Printf / Println
    let _: Result<usize> = c.print(&args);
    let _: Result<usize> = c.printf("%s", &args);
    let _: Result<usize> = c.println(&args);

    // func (c *Color) Sprint / Sprintf / Sprintln
    let _: String = c.sprint(&args);
    let _: String = c.sprintf("%s", &args);
    let _: String = c.sprintln(&args);

    // The *Func factories.
    c.fprint_func()(&mut sink, &args);
    c.print_func()(&args);
    c.fprintf_func()(&mut sink, "%s", &args);
    c.printf_func()("%s", &args);
    c.fprintln_func()(&mut sink, &args);
    c.println_func()(&args);
    let _: String = c.sprint_func()(&args);
    let _: String = c.sprintf_func()("%s", &args);
    let _: String = c.sprintln_func()(&args);

    // func (c *Color) DisableColor() / EnableColor()
    c.disable_color();
    c.enable_color();

    // func (c *Color) Equals(c2 *Color) bool
    let _: bool = c.equals(&Color::new(&[FG_RED]));
    // Equals with the nil-receiver semantics of the Go method.
    assert!(color::equals(None, None));
    assert!(!color::equals(Some(&c), None));
}

/// Package-level functions.
#[test]
fn package_functions_are_complete() {
    let (_g, _scratch) = serial();

    // func New / RGB / BgRGB
    let _: Color = color::new(&[FG_RED]);
    let _: Color = rgb(255, 128, 0);
    let _: Color = bg_rgb(255, 128, 0);

    // func Set(p ...Attribute) *Color ; func Unset()
    let _: Color = set(&[FG_RED]);
    unset();

    // Package variables NoColor / Output / Error.
    let _: bool = no_color();
    set_no_color(no_color());
    let _: Writer = output();
    set_output(output());
    let _: Writer = color::error();
    set_error(color::error());

    // Unexported helpers exercised by color_test.go.
    let _: bool = color::internal::no_color_is_set();
    let _: bool = stdout_is_terminal();
    let _: Writer = std_out();
    let _: Writer = std_err();
}

/// The 16 `Xxx` print helpers and the 16 `XxxString` helpers.
#[test]
fn helper_functions_are_complete() {
    let (_g, _scratch) = serial();

    type PrintFn = fn(&str, &[Value]);
    type StringFn = fn(&str, &[Value]) -> String;

    let prints: [(&str, PrintFn); 16] = [
        ("Black", black),
        ("Red", red),
        ("Green", green),
        ("Yellow", yellow),
        ("Blue", blue),
        ("Magenta", magenta),
        ("Cyan", cyan),
        ("White", white),
        ("HiBlack", hi_black),
        ("HiRed", hi_red),
        ("HiGreen", hi_green),
        ("HiYellow", hi_yellow),
        ("HiBlue", hi_blue),
        ("HiMagenta", hi_magenta),
        ("HiCyan", hi_cyan),
        ("HiWhite", hi_white),
    ];

    let strings: [(&str, StringFn); 16] = [
        ("BlackString", black_string),
        ("RedString", red_string),
        ("GreenString", green_string),
        ("YellowString", yellow_string),
        ("BlueString", blue_string),
        ("MagentaString", magenta_string),
        ("CyanString", cyan_string),
        ("WhiteString", white_string),
        ("HiBlackString", hi_black_string),
        ("HiRedString", hi_red_string),
        ("HiGreenString", hi_green_string),
        ("HiYellowString", hi_yellow_string),
        ("HiBlueString", hi_blue_string),
        ("HiMagentaString", hi_magenta_string),
        ("HiCyanString", hi_cyan_string),
        ("HiWhiteString", hi_white_string),
    ];

    assert_eq!(prints.len(), 16);
    assert_eq!(strings.len(), 16);

    for (_, f) in prints {
        f("x", NO_ARGS);
    }
    for (name, f) in strings {
        let s = f("x", NO_ARGS);
        assert!(s.contains('x'), "{name} dropped its argument");
    }
}

/// `io.Writer` / `bytes.Buffer` counterparts.
#[test]
fn writer_surface_is_complete() {
    let buf = SharedBuffer::new();
    let mut w = buf.writer();
    w.write_all(b"hello\nworld").unwrap();
    w.flush().unwrap();

    assert_eq!(buf.len(), 11);
    assert!(!buf.is_empty());
    let (line, err) = buf.read_string(b'\n');
    assert_eq!(line, "hello\n");
    assert!(err.is_none());
    assert_eq!(buf.string(), "world");
    assert_eq!(buf.bytes(), b"world");
    assert_eq!(buf.read_all(), "world");
    buf.reset();
    assert!(buf.is_empty());

    // io.Discard / colorable stdout / stderr / arbitrary writers.
    assert!(Writer::Discard.is_discard());
    let _: Writer = new_colorable_stdout();
    let _: Writer = new_colorable_stderr();
    let _: Writer = Writer::custom(Vec::<u8>::new());

    // os.Stdout == nil / os.Stderr == nil emulation.
    assert!(stdout_available());
    assert!(stderr_available());
}

/// The bundled `fmt` subset.
#[test]
fn gofmt_surface_is_complete() {
    assert_eq!(gofmt::sprint(&vals!["a", "b"]), "ab");
    assert_eq!(gofmt::sprintln(&vals!["a", "b"]), "a b\n");
    assert_eq!(gofmt::sprintf("%s", &vals!["a"]), "a");
    assert_eq!(gofmt::go_quote("a\nb"), "\"a\\nb\"");
}

/// `Value` covers every Go dynamic type the formatter distinguishes.
#[test]
fn value_surface_is_complete() {
    let values = [
        Value::Nil,
        Value::Bool(true),
        Value::Int(1),
        Value::Uint(1),
        Value::Uint8(1),
        Value::Float(1.0),
        Value::Str("s".into()),
        Value::Rune('r'),
        Value::Bytes(vec![1]),
        Value::Slice(vec![Value::Int(1)]),
        Value::Error("e".into()),
    ];
    let names: Vec<&str> = values.iter().map(|v| v.go_type_name()).collect();
    assert_eq!(
        names,
        [
            "<nil>",
            "bool",
            "int",
            "uint",
            "uint8",
            "float64",
            "string",
            "int32",
            "[]uint8",
            "[]interface {}",
            "*errors.errorString",
        ]
    );

    // Only `string` has reflect.Kind == String, which drives Sprint's spacing.
    assert!(Value::Str("x".into()).is_string_kind());
    assert!(!Value::Bytes(vec![b'x']).is_string_kind());
    assert!(Value::Nil.is_nil());

    // From conversions.
    let _ = vals![
        "str",
        String::from("string"),
        1i8,
        1i16,
        1i32,
        1i64,
        1isize,
        1u8,
        1u16,
        1u32,
        1u64,
        1usize,
        1.0f32,
        1.0f64,
        true,
        'c',
        Option::<i32>::None,
        vec![1u8, 2],
    ];
}
