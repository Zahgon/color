//! Port of `color_test.go`.
//!
//! Every test function, table entry, input, assertion and expected output is
//! carried over one-for-one.
//!
//! ## Test isolation
//!
//! `go test` runs the functions of a package sequentially, so the Go suite is
//! free to mutate process-global state (`color.NoColor`, `color.Output`,
//! `os.Stdout`, `NO_COLOR`) and to rely on values left behind by an earlier
//! test — `TestColor` sets `NoColor = false` and every later test inherits it.
//!
//! Rust's harness runs tests in parallel threads, so each test here
//! * takes [`serial`], a process-wide lock, and
//! * restores the baseline global state that the corresponding Go test would
//!   have observed at its point in the sequential run.
//!
//! The result is order-independent while remaining faithful to Go.

use std::io::Write;
use std::sync::{Mutex, MutexGuard, OnceLock};

use color::{
    gofmt, vals, Attribute, Color, SharedBuffer, Value, Writer, BG_BLACK, BG_BLUE, BG_CYAN,
    BG_GREEN, BG_MAGENTA, BG_RED, BG_WHITE, BG_YELLOW, BOLD, FG_BLACK, FG_BLUE, FG_CYAN, FG_GREEN,
    FG_HI_BLACK, FG_HI_BLUE, FG_HI_CYAN, FG_HI_GREEN, FG_HI_MAGENTA, FG_HI_RED, FG_HI_WHITE,
    FG_HI_YELLOW, FG_MAGENTA, FG_RED, FG_WHITE, FG_YELLOW, NO_ARGS, UNDERLINE,
};

// ---------------------------------------------------------------------------
// Harness helpers
// ---------------------------------------------------------------------------

fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Serializes access to the package globals and resets them to the baseline
/// the Go suite runs with.
fn serial() -> MutexGuard<'static, ()> {
    let guard = test_lock().lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var("NO_COLOR");
    color::set_stdout_available(true);
    color::set_stderr_available(true);
    // `TestColor` assigns `NoColor = false` and nothing restores it, so every
    // subsequent Go test observes colorized output.
    color::set_no_color(false);
    color::set_output(color::std_out());
    color::set_error(color::std_err());
    guard
}

/// Captures what a *visual* test emits through `color.Output` / `color.Error`
/// and replays it through the `print!` macros when the test ends.
///
/// `TestColorVisual`, `TestNoFormat` and `TestRGB` exist to put escape
/// sequences on a terminal, and in Go they write straight to `os.Stdout`.
/// `go test` tolerates that because it parses nothing; libtest does not.
/// libtest captures only the `print!`/`eprint!` macros, so a write issued
/// directly to the process handle lands *inside* libtest's own
/// `test NAME ... ok` report line. Three such lines were being corrupted, which
/// is enough to make the whole run unparsable — the suite is green and no tool
/// downstream can tell.
///
/// Routing the package writers through a buffer and replaying it through
/// `print!` keeps the visual output (`cargo test -- --nocapture` shows exactly
/// what Go shows) while leaving the harness's report intact. The bytes are
/// unchanged: only the handle they reach the terminal through differs.
struct VisualCapture {
    out: SharedBuffer,
    err: SharedBuffer,
}

impl VisualCapture {
    /// Redirects `color.Output` and `color.Error` into fresh buffers.
    fn install() -> Self {
        let capture = Self {
            out: SharedBuffer::new(),
            err: SharedBuffer::new(),
        };
        color::set_output(capture.out.writer());
        color::set_error(capture.err.writer());
        capture
    }

    /// Replays the captured bytes to the real streams via the macros libtest
    /// captures, and returns what was written to `color.Output`.
    fn replay(&self) -> String {
        let out = self.out.read_all();
        let err = self.err.read_all();
        print!("{out}");
        eprint!("{err}");
        out
    }
}

/// Asserts that a visual test actually drove the colorizing path.
///
/// The Go originals assert nothing, which is why a port can silently reduce
/// them to no-ops. `emitted` is required to be non-empty and to carry the
/// `ESC[` introducer every one of these tests is there to produce.
fn assert_colorized(emitted: &str, what: &str) {
    assert!(!emitted.is_empty(), "{what} emitted nothing");
    assert!(
        emitted.contains('\x1b'),
        "{what} emitted no escape sequence: {emitted:?}"
    );
}

/// The `testColors` table, shared by `TestColor`, `TestNoColor` and
/// `TestNoColor_Env` exactly as in Go.
const TEST_COLORS: &[(&str, Attribute)] = &[
    ("black", FG_BLACK),
    ("red", FG_RED),
    ("green", FG_GREEN),
    ("yellow", FG_YELLOW),
    ("blue", FG_BLUE),
    ("magent", FG_MAGENTA),
    ("cyan", FG_CYAN),
    ("white", FG_WHITE),
    ("hblack", FG_HI_BLACK),
    ("hred", FG_HI_RED),
    ("hgreen", FG_HI_GREEN),
    ("hyellow", FG_HI_YELLOW),
    ("hblue", FG_HI_BLUE),
    ("hmagent", FG_HI_MAGENTA),
    ("hcyan", FG_HI_CYAN),
    ("hwhite", FG_HI_WHITE),
];

/// Port of the `readRaw` helper: `io.ReadAll(r)`.
fn read_raw(r: &SharedBuffer) -> String {
    r.read_all()
}

// ---------------------------------------------------------------------------
// TestColor
// ---------------------------------------------------------------------------

/// Testing colors is kinda different. First we test for given colors and their
/// escaped formatted results. Next we create some visual tests to be tested.
/// Each visual test includes the color name to be compared.
#[test]
fn test_color() {
    let _g = serial();

    let rb = SharedBuffer::new();
    color::set_output(rb.writer());

    color::set_no_color(false);

    for (text, code) in TEST_COLORS {
        Color::new(&[*code]).print(&vals![*text]).unwrap();

        let (line, _) = rb.read_string(b'\n');
        let scanned_line = gofmt::sprintf("%q", &vals![line.clone()]);
        let colored = gofmt::sprintf("\x1b[%dm%s\x1b[0m", &vals![code.code(), *text]);
        let escaped_form = gofmt::sprintf("%q", &vals![colored]);

        print!("{}", gofmt::sprintf("%s\t: %s\n", &vals![*text, line]));

        assert_eq!(
            scanned_line, escaped_form,
            "Expecting {escaped_form}, got '{scanned_line}'\n"
        );
    }

    for (text, code) in TEST_COLORS {
        let line = Color::new(&[*code]).sprintf("%s", &vals![*text]);
        let scanned_line = gofmt::sprintf("%q", &vals![line.clone()]);
        let colored = gofmt::sprintf("\x1b[%dm%s\x1b[0m", &vals![code.code(), *text]);
        let escaped_form = gofmt::sprintf("%q", &vals![colored]);

        print!("{}", gofmt::sprintf("%s\t: %s\n", &vals![*text, line]));

        assert_eq!(
            scanned_line, escaped_form,
            "Expecting {escaped_form}, got '{scanned_line}'\n"
        );
    }
}

// ---------------------------------------------------------------------------
// TestColorEquals
// ---------------------------------------------------------------------------

#[test]
fn test_color_equals() {
    let _g = serial();

    let fgblack1 = Color::new(&[FG_BLACK]);
    let fgblack2 = Color::new(&[FG_BLACK]);
    let bgblack = Color::new(&[BG_BLACK]);
    let fgbgblack = Color::new(&[FG_BLACK, BG_BLACK]);
    let fgblackbgred = Color::new(&[FG_BLACK, BG_RED]);
    let fgred = Color::new(&[FG_RED]);
    let bgred = Color::new(&[BG_RED]);

    assert!(fgblack1.equals(&fgblack2), "Two black colors are not equal");
    assert!(
        !fgblack1.equals(&bgblack),
        "Fg and bg black colors are equal"
    );
    assert!(
        !fgblack1.equals(&fgbgblack),
        "Fg black equals fg/bg black color"
    );
    assert!(!fgblack1.equals(&fgred), "Fg black equals Fg red");
    assert!(!fgblack1.equals(&bgred), "Fg black equals Bg red");
    assert!(
        !fgblack1.equals(&fgblackbgred),
        "Fg black equals fg black bg red"
    );
}

// ---------------------------------------------------------------------------
// TestColorEquals_DuplicateAttributes
// ---------------------------------------------------------------------------

#[test]
fn test_color_equals_duplicate_attributes() {
    let _g = serial();

    let mut ordered = Color::new(&[FG_RED, BOLD]);
    ordered.add(&[FG_RED]);
    let mut reordered = Color::new(&[BOLD, FG_RED]);
    reordered.add(&[FG_RED]);
    let mut different_counts = Color::new(&[FG_RED, BOLD]);
    different_counts.add(&[BOLD]);

    assert!(
        ordered.equals(&reordered),
        "Colors with the same attributes in different orders are not equal"
    );
    assert!(
        !ordered.equals(&different_counts),
        "Colors with different duplicate attribute counts are equal"
    );
}

// ---------------------------------------------------------------------------
// TestNoColor
// ---------------------------------------------------------------------------

#[test]
fn test_no_color() {
    let _g = serial();

    let rb = SharedBuffer::new();
    color::set_output(rb.writer());

    for (text, code) in TEST_COLORS {
        let mut p = Color::new(&[*code]);
        p.disable_color();
        p.print(&vals![*text]).unwrap();

        let (line, _) = rb.read_string(b'\n');
        assert_eq!(&line, text, "Expecting {text}, got '{line}'\n");
    }

    // global check
    color::set_no_color(true);
    // t.Cleanup(func() { NoColor = false })
    struct Cleanup;
    impl Drop for Cleanup {
        fn drop(&mut self) {
            color::set_no_color(false);
        }
    }
    let _cleanup = Cleanup;

    for (text, code) in TEST_COLORS {
        let p = Color::new(&[*code]);
        p.print(&vals![*text]).unwrap();

        let (line, _) = rb.read_string(b'\n');
        assert_eq!(&line, text, "Expecting {text}, got '{line}'\n");
    }
}

// ---------------------------------------------------------------------------
// TestNoColor_Env
// ---------------------------------------------------------------------------

#[test]
fn test_no_color_env() {
    let _g = serial();

    let rb = SharedBuffer::new();
    color::set_output(rb.writer());

    std::env::set_var("NO_COLOR", "1");
    struct Cleanup;
    impl Drop for Cleanup {
        fn drop(&mut self) {
            std::env::remove_var("NO_COLOR");
        }
    }
    let _cleanup = Cleanup;

    for (text, code) in TEST_COLORS {
        let p = Color::new(&[*code]);
        p.print(&vals![*text]).unwrap();

        let (line, _) = rb.read_string(b'\n');
        assert_eq!(&line, text, "Expecting {text}, got '{line}'\n");
    }
}

// ---------------------------------------------------------------------------
// Test_noColorIsSet
// ---------------------------------------------------------------------------

/// One row of Go's `Test_noColorIsSet` table, including its `t.Cleanup`.
///
/// Go drives each row through `t.Run`, which makes every row a separately
/// named, separately reported, independently failing unit — `go test -v` lists
/// `Test_noColorIsSet/default`, `/NO_COLOR=1` and `/NO_COLOR=`. Rust has no
/// subtests, so the faithful port is one `#[test]` per `t.Run` rather than one
/// test looping over the table: a loop stops at the first failed assertion and
/// hides every row behind it, and reports one test where Go reports three.
fn check_no_color_is_set(name: &str, act: impl FnOnce(), want: bool) {
    act();
    let got = color::internal::no_color_is_set();
    // t.Cleanup(func() { os.Unsetenv("NO_COLOR") })
    std::env::remove_var("NO_COLOR");

    assert_eq!(got, want, "[{name}] noColorIsSet() = {got}, want {want}");
}

/// `Test_noColorIsSet/default`
#[test]
fn test_no_color_is_set_default() {
    let _g = serial();
    check_no_color_is_set("default", || {}, false);
}

/// `Test_noColorIsSet/NO_COLOR=1`
#[test]
fn test_no_color_is_set_no_color_1() {
    let _g = serial();
    check_no_color_is_set("NO_COLOR=1", || std::env::set_var("NO_COLOR", "1"), true);
}

/// `Test_noColorIsSet/NO_COLOR=`
#[test]
fn test_no_color_is_set_no_color_empty() {
    let _g = serial();
    check_no_color_is_set("NO_COLOR=", || std::env::set_var("NO_COLOR", ""), false);
}

// ---------------------------------------------------------------------------
// TestStdoutIsTerminal_NilStdout / TestStdOut_NilStdout / TestStdErr_NilStderr
// ---------------------------------------------------------------------------

#[test]
fn test_stdout_is_terminal_nil_stdout() {
    let _g = serial();

    // stdout := os.Stdout; os.Stdout = nil
    color::set_stdout_available(false);
    struct Cleanup;
    impl Drop for Cleanup {
        fn drop(&mut self) {
            color::set_stdout_available(true);
        }
    }
    let _cleanup = Cleanup;

    assert!(
        !color::stdout_is_terminal(),
        "stdoutIsTerminal() = true, want false"
    );
}

#[test]
fn test_std_out_nil_stdout() {
    let _g = serial();

    color::set_stdout_available(false);
    struct Cleanup;
    impl Drop for Cleanup {
        fn drop(&mut self) {
            color::set_stdout_available(true);
        }
    }
    let _cleanup = Cleanup;

    let got = color::std_out();
    assert_eq!(
        got,
        Writer::Discard,
        "stdOut() = {got:?}, want {:?}",
        Writer::Discard
    );
}

#[test]
fn test_std_err_nil_stderr() {
    let _g = serial();

    color::set_stderr_available(false);
    struct Cleanup;
    impl Drop for Cleanup {
        fn drop(&mut self) {
            color::set_stderr_available(true);
        }
    }
    let _cleanup = Cleanup;

    let got = color::std_err();
    assert_eq!(
        got,
        Writer::Discard,
        "stdErr() = {got:?}, want {:?}",
        Writer::Discard
    );
}

// ---------------------------------------------------------------------------
// TestColorVisual
// ---------------------------------------------------------------------------

#[test]
fn test_color_visual() {
    let _g = serial();
    let visual = VisualCapture::install();

    // First Visual Test

    Color::new(&[FG_RED]).printf("red\t", NO_ARGS).unwrap();
    Color::new(&[BG_RED]).print(&vals!["         "]).unwrap();
    Color::new(&[FG_RED, BOLD]).println(&vals![" red"]).unwrap();

    Color::new(&[FG_GREEN]).printf("green\t", NO_ARGS).unwrap();
    Color::new(&[BG_GREEN]).print(&vals!["         "]).unwrap();
    Color::new(&[FG_GREEN, BOLD])
        .println(&vals![" green"])
        .unwrap();

    Color::new(&[FG_YELLOW])
        .printf("yellow\t", NO_ARGS)
        .unwrap();
    Color::new(&[BG_YELLOW]).print(&vals!["         "]).unwrap();
    Color::new(&[FG_YELLOW, BOLD])
        .println(&vals![" yellow"])
        .unwrap();

    Color::new(&[FG_BLUE]).printf("blue\t", NO_ARGS).unwrap();
    Color::new(&[BG_BLUE]).print(&vals!["         "]).unwrap();
    Color::new(&[FG_BLUE, BOLD])
        .println(&vals![" blue"])
        .unwrap();

    Color::new(&[FG_MAGENTA])
        .printf("magenta\t", NO_ARGS)
        .unwrap();
    Color::new(&[BG_MAGENTA])
        .print(&vals!["         "])
        .unwrap();
    Color::new(&[FG_MAGENTA, BOLD])
        .println(&vals![" magenta"])
        .unwrap();

    Color::new(&[FG_CYAN]).printf("cyan\t", NO_ARGS).unwrap();
    Color::new(&[BG_CYAN]).print(&vals!["         "]).unwrap();
    Color::new(&[FG_CYAN, BOLD])
        .println(&vals![" cyan"])
        .unwrap();

    Color::new(&[FG_WHITE]).printf("white\t", NO_ARGS).unwrap();
    Color::new(&[BG_WHITE]).print(&vals!["         "]).unwrap();
    Color::new(&[FG_WHITE, BOLD])
        .println(&vals![" white"])
        .unwrap();
    println!();

    // Second Visual test
    color::black("black", NO_ARGS);
    color::red("red", NO_ARGS);
    color::green("green", NO_ARGS);
    color::yellow("yellow", NO_ARGS);
    color::blue("blue", NO_ARGS);
    color::magenta("magenta", NO_ARGS);
    color::cyan("cyan", NO_ARGS);
    color::white("white", NO_ARGS);
    color::hi_black("hblack", NO_ARGS);
    color::hi_red("hred", NO_ARGS);
    color::hi_green("hgreen", NO_ARGS);
    color::hi_yellow("hyellow", NO_ARGS);
    color::hi_blue("hblue", NO_ARGS);
    color::hi_magenta("hmagenta", NO_ARGS);
    color::hi_cyan("hcyan", NO_ARGS);
    color::hi_white("hwhite", NO_ARGS);

    // Third visual test
    println!();
    color::set(&[FG_BLUE]);
    println!("is this blue?");
    color::unset();

    color::set(&[FG_MAGENTA]);
    println!("and this magenta?");
    color::unset();

    // Fourth Visual test
    println!();
    let blue = Color::new(&[FG_BLUE]).println_func();
    blue(&vals!["blue text with custom print func"]);

    let red = Color::new(&[FG_RED]).printf_func();
    red("red text with a printf func: %d\n", &vals![123]);

    let put = Color::new(&[FG_YELLOW]).sprint_func();
    let warn = Color::new(&[FG_RED]).sprint_func();

    let mut out = color::output();
    write!(
        out,
        "{}",
        gofmt::sprintf(
            "this is a %s and this is %s.\n",
            &vals![put(&vals!["warning"]), warn(&vals!["error"])]
        )
    )
    .unwrap();

    let info = Color::new(&[FG_WHITE, BG_GREEN]).sprint_func();
    write!(
        out,
        "{}",
        gofmt::sprintf("this %s rocks!\n", &vals![info(&vals!["package"])])
    )
    .unwrap();

    // Go writes this one to `os.Stderr`; the capture stands in for it and is
    // replayed through `eprint!`, so it still arrives on stderr.
    let notice = Color::new(&[FG_BLUE]).fprint_func();
    let mut stderr = color::error();
    notice(&mut stderr, &vals!["just a blue notice to stderr"]);

    // Fifth Visual Test
    println!();

    let mut out = color::output();
    for s in [
        color::black_string("black", NO_ARGS),
        color::red_string("red", NO_ARGS),
        color::green_string("green", NO_ARGS),
        color::yellow_string("yellow", NO_ARGS),
        color::blue_string("blue", NO_ARGS),
        color::magenta_string("magenta", NO_ARGS),
        color::cyan_string("cyan", NO_ARGS),
        color::white_string("white", NO_ARGS),
        color::hi_black_string("hblack", NO_ARGS),
        color::hi_red_string("hred", NO_ARGS),
        color::hi_green_string("hgreen", NO_ARGS),
        color::hi_yellow_string("hyellow", NO_ARGS),
        color::hi_blue_string("hblue", NO_ARGS),
        color::hi_magenta_string("hmagenta", NO_ARGS),
        color::hi_cyan_string("hcyan", NO_ARGS),
        color::hi_white_string("hwhite", NO_ARGS),
    ] {
        write!(out, "{}", gofmt::sprintln(&vals![s])).unwrap();
    }

    assert_colorized(&visual.replay(), "TestColorVisual");
}

// ---------------------------------------------------------------------------
// TestNoFormat
// ---------------------------------------------------------------------------

#[test]
fn test_no_format() {
    let _g = serial();
    // In Go this test inherits `Output` from TestColorVisual.
    let visual = VisualCapture::install();

    print!(
        "{}",
        gofmt::sprintf("%s   %%s = ", &vals![color::black_string("Black", NO_ARGS)])
    );
    color::black("%s", NO_ARGS);

    print!(
        "{}",
        gofmt::sprintf("%s     %%s = ", &vals![color::red_string("Red", NO_ARGS)])
    );
    color::red("%s", NO_ARGS);

    print!(
        "{}",
        gofmt::sprintf("%s   %%s = ", &vals![color::green_string("Green", NO_ARGS)])
    );
    color::green("%s", NO_ARGS);

    print!(
        "{}",
        gofmt::sprintf(
            "%s  %%s = ",
            &vals![color::yellow_string("Yellow", NO_ARGS)]
        )
    );
    color::yellow("%s", NO_ARGS);

    print!(
        "{}",
        gofmt::sprintf("%s    %%s = ", &vals![color::blue_string("Blue", NO_ARGS)])
    );
    color::blue("%s", NO_ARGS);

    print!(
        "{}",
        gofmt::sprintf(
            "%s %%s = ",
            &vals![color::magenta_string("Magenta", NO_ARGS)]
        )
    );
    color::magenta("%s", NO_ARGS);

    print!(
        "{}",
        gofmt::sprintf("%s    %%s = ", &vals![color::cyan_string("Cyan", NO_ARGS)])
    );
    color::cyan("%s", NO_ARGS);

    print!(
        "{}",
        gofmt::sprintf("%s   %%s = ", &vals![color::white_string("White", NO_ARGS)])
    );
    color::white("%s", NO_ARGS);

    print!(
        "{}",
        gofmt::sprintf(
            "%s   %%s = ",
            &vals![color::hi_black_string("HiBlack", NO_ARGS)]
        )
    );
    color::hi_black("%s", NO_ARGS);

    print!(
        "{}",
        gofmt::sprintf(
            "%s     %%s = ",
            &vals![color::hi_red_string("HiRed", NO_ARGS)]
        )
    );
    color::hi_red("%s", NO_ARGS);

    print!(
        "{}",
        gofmt::sprintf(
            "%s   %%s = ",
            &vals![color::hi_green_string("HiGreen", NO_ARGS)]
        )
    );
    color::hi_green("%s", NO_ARGS);

    print!(
        "{}",
        gofmt::sprintf(
            "%s  %%s = ",
            &vals![color::hi_yellow_string("HiYellow", NO_ARGS)]
        )
    );
    color::hi_yellow("%s", NO_ARGS);

    print!(
        "{}",
        gofmt::sprintf(
            "%s    %%s = ",
            &vals![color::hi_blue_string("HiBlue", NO_ARGS)]
        )
    );
    color::hi_blue("%s", NO_ARGS);

    print!(
        "{}",
        gofmt::sprintf(
            "%s %%s = ",
            &vals![color::hi_magenta_string("HiMagenta", NO_ARGS)]
        )
    );
    color::hi_magenta("%s", NO_ARGS);

    print!(
        "{}",
        gofmt::sprintf(
            "%s    %%s = ",
            &vals![color::hi_cyan_string("HiCyan", NO_ARGS)]
        )
    );
    color::hi_cyan("%s", NO_ARGS);

    print!(
        "{}",
        gofmt::sprintf(
            "%s   %%s = ",
            &vals![color::hi_white_string("HiWhite", NO_ARGS)]
        )
    );
    color::hi_white("%s", NO_ARGS);

    assert_colorized(&visual.replay(), "TestNoFormat");
}

// ---------------------------------------------------------------------------
// TestNoFormatString
// ---------------------------------------------------------------------------

#[test]
fn test_no_format_string() {
    let _g = serial();

    type StringFn = fn(&str, &[Value]) -> String;

    let tests: &[(StringFn, &str, &[Value], &str)] = &[
        (color::black_string, "%s", NO_ARGS, "\x1b[30m%s\x1b[0m"),
        (color::red_string, "%s", NO_ARGS, "\x1b[31m%s\x1b[0m"),
        (color::green_string, "%s", NO_ARGS, "\x1b[32m%s\x1b[0m"),
        (color::yellow_string, "%s", NO_ARGS, "\x1b[33m%s\x1b[0m"),
        (color::blue_string, "%s", NO_ARGS, "\x1b[34m%s\x1b[0m"),
        (color::magenta_string, "%s", NO_ARGS, "\x1b[35m%s\x1b[0m"),
        (color::cyan_string, "%s", NO_ARGS, "\x1b[36m%s\x1b[0m"),
        (color::white_string, "%s", NO_ARGS, "\x1b[37m%s\x1b[0m"),
        (color::hi_black_string, "%s", NO_ARGS, "\x1b[90m%s\x1b[0m"),
        (color::hi_red_string, "%s", NO_ARGS, "\x1b[91m%s\x1b[0m"),
        (color::hi_green_string, "%s", NO_ARGS, "\x1b[92m%s\x1b[0m"),
        (color::hi_yellow_string, "%s", NO_ARGS, "\x1b[93m%s\x1b[0m"),
        (color::hi_blue_string, "%s", NO_ARGS, "\x1b[94m%s\x1b[0m"),
        (color::hi_magenta_string, "%s", NO_ARGS, "\x1b[95m%s\x1b[0m"),
        (color::hi_cyan_string, "%s", NO_ARGS, "\x1b[96m%s\x1b[0m"),
        (color::hi_white_string, "%s", NO_ARGS, "\x1b[97m%s\x1b[0m"),
    ];

    for (i, (f, format, args, want)) in tests.iter().enumerate() {
        let s = f(format, args);

        assert_eq!(&s, want, "[{i}] want: {want:?}, got: {s:?}");
    }
}

// ---------------------------------------------------------------------------
// Println / Sprintln / Fprint / Fprintln / Fprintf
// ---------------------------------------------------------------------------

#[test]
fn test_color_println_newline() {
    let _g = serial();

    let rb = SharedBuffer::new();
    color::set_output(rb.writer());

    let c = Color::new(&[FG_RED]);
    c.println(&vals!["foo"]).unwrap();

    let got = read_raw(&rb);
    let want = "\x1b[31mfoo\x1b[0m\n";

    assert_eq!(
        want, got,
        "Println newline error\n\nwant: {want:?}\n got: {got:?}"
    );
}

#[test]
fn test_color_sprintln_newline() {
    let _g = serial();

    let c = Color::new(&[FG_RED]);

    let got = c.sprintln(&vals!["foo"]);
    let want = "\x1b[31mfoo\x1b[0m\n";

    assert_eq!(
        want, got,
        "Println newline error\n\nwant: {want:?}\n got: {got:?}"
    );
}

#[test]
fn test_color_fprint() {
    let _g = serial();

    let rb = SharedBuffer::new();
    let c = Color::new(&[FG_RED]);

    let n = c
        .fprint(&mut rb.clone(), &vals!["foo", "bar"])
        .unwrap_or_else(|e| panic!("Fprint error: {e}"));
    let got = rb.string();
    let want = "\x1b[31mfoobar\x1b[0m";

    assert_eq!(want, got, "Fprint error\n\nwant: {want:?}\n got: {got:?}");
    assert_eq!(
        n,
        got.len(),
        "Fprint byte count does not match actual bytes written\n\nwant: {}\n got: {n}",
        got.len()
    );
}

#[test]
fn test_color_fprintln() {
    let _g = serial();

    let rb = SharedBuffer::new();
    let c = Color::new(&[FG_RED]);

    let n = c
        .fprintln(&mut rb.clone(), &vals!["foo", "bar"])
        .unwrap_or_else(|e| panic!("Fprint error: {e}"));
    let got = rb.string();
    let want = "\x1b[31mfoo bar\x1b[0m\n";

    assert_eq!(want, got, "Fprintln error\n\nwant: {want:?}\n got: {got:?}");
    assert_eq!(
        n,
        got.len(),
        "Fprintln byte count does not match actual bytes written\n\nwant: {}\n got: {n}",
        got.len()
    );
}

#[test]
fn test_color_fprintf() {
    let _g = serial();

    let rb = SharedBuffer::new();
    let c = Color::new(&[FG_RED]);

    let n = c
        .fprintf(
            &mut rb.clone(),
            "%-7s %-7s %5d\n",
            &vals!["hello", "world", 123],
        )
        .unwrap_or_else(|e| panic!("Fprint error: {e}"));

    let want = "\x1b[31mhello   world     123\n\x1b[0m";

    let got = rb.string();
    assert_eq!(want, got, "Fprintf error\n\nwant: {want:?}\n got: {got:?}");
    assert_eq!(
        n,
        got.len(),
        "Fprintf byte count does not match actual bytes written\n\nwant: {}\n got: {n}",
        got.len()
    );
}

#[test]
fn test_color_fprintln_newline() {
    let _g = serial();

    let rb = SharedBuffer::new();
    let c = Color::new(&[FG_RED]);
    c.fprintln(&mut rb.clone(), &vals!["foo"]).unwrap();

    let got = read_raw(&rb);
    let want = "\x1b[31mfoo\x1b[0m\n";

    assert_eq!(
        want, got,
        "Println newline error\n\nwant: {want:?}\n got: {got:?}"
    );
}

// ---------------------------------------------------------------------------
// Issue regressions
// ---------------------------------------------------------------------------

#[test]
fn test_issue206_1() {
    let _g = serial();

    // visual test, cargo test -- --nocapture
    let underline = Color::new(&[UNDERLINE]).sprint_func();

    let line = gofmt::sprintf(
        "%s %s %s %s",
        &vals![
            "word1",
            underline(&vals!["word2"]),
            "word3",
            underline(&vals!["word4"])
        ],
    );

    let line = color::cyan_string(&line, NO_ARGS);

    println!("{line}");

    let result = gofmt::sprintf("%v", &vals![line]);
    const EXPECTED_RESULT: &str =
        "\x1b[36mword1 \x1b[4mword2\x1b[24m word3 \x1b[4mword4\x1b[24m\x1b[0m";

    assert_eq!(
        result.as_bytes(),
        EXPECTED_RESULT.as_bytes(),
        "Expecting {EXPECTED_RESULT}, got '{result}'\n"
    );
}

#[test]
fn test_issue206_2() {
    let _g = serial();

    let underline = Color::new(&[UNDERLINE]).sprint_func();
    let bold = Color::new(&[BOLD]).sprint_func();

    let line = gofmt::sprintf(
        "%s %s",
        &vals![
            color::green_string(&underline(&vals!["underlined regular green"]), NO_ARGS),
            color::red_string(&bold(&vals!["bold red"]), NO_ARGS)
        ],
    );

    println!("{line}");

    let result = gofmt::sprintf("%v", &vals![line]);
    const EXPECTED_RESULT: &str = "\x1b[32m\x1b[4munderlined regular green\x1b[24m\x1b[0m \x1b[31m\x1b[1mbold red\x1b[22m\x1b[0m";

    assert_eq!(
        result.as_bytes(),
        EXPECTED_RESULT.as_bytes(),
        "Expecting {EXPECTED_RESULT}, got '{result}'\n"
    );
}

#[test]
fn test_issue218() {
    let _g = serial();

    let scratch = SharedBuffer::new();
    color::set_output(scratch.writer());

    // Adds a newline to the end of the last string to make sure it isn't trimmed.
    let params = vals!["word1", "word2", "word3", "word4\n"];

    let c = Color::new(&[FG_CYAN]);
    c.println(&params).unwrap();

    let result = c.sprintln(&params);
    println!("{}", gofmt::sprintln(&params).trim_end_matches('\n'));
    print!("{result}");

    const EXPECTED_RESULT: &str = "\x1b[36mword1 word2 word3 word4\n\x1b[0m\n";

    assert_eq!(
        result.as_bytes(),
        EXPECTED_RESULT.as_bytes(),
        "Sprintln: Expecting {EXPECTED_RESULT} ({:?}), got '{result} ({:?})'\n",
        EXPECTED_RESULT.as_bytes(),
        result.as_bytes()
    );

    let f = c.sprintln_func();
    let result = f(&params);
    assert_eq!(
        result.as_bytes(),
        EXPECTED_RESULT.as_bytes(),
        "SprintlnFunc: Expecting {EXPECTED_RESULT} ({:?}), got '{result} ({:?})'\n",
        EXPECTED_RESULT.as_bytes(),
        result.as_bytes()
    );

    let buf = SharedBuffer::new();
    c.fprintln(&mut buf.clone(), &params).unwrap();
    let result = buf.string();
    assert_eq!(
        result.as_bytes(),
        EXPECTED_RESULT.as_bytes(),
        "Fprintln: Expecting {EXPECTED_RESULT} ({:?}), got '{result} ({:?})'\n",
        EXPECTED_RESULT.as_bytes(),
        result.as_bytes()
    );
}

// ---------------------------------------------------------------------------
// TestRGB
// ---------------------------------------------------------------------------

/// The `TestRGB` table, verbatim from Go.
const RGB_CASES: [(i32, i32, i32); 2] = [(255, 128, 0), (230, 42, 42)];

/// One row of `TestRGB`.
///
/// Go names the subtest after the row index — `t.Run(fmt.Sprintf("%d", i), ...)`
/// yields `TestRGB/0` and `TestRGB/1` — so, as with `Test_noColorIsSet`, each
/// row is its own `#[test]` here.
fn run_rgb_case(index: usize) {
    let (r, g, b) = RGB_CASES[index];

    color::rgb(r, g, b).println(&vals!["foreground"]).unwrap();

    let mut c = color::rgb(r, g, b);
    c.add_bg_rgb(0, 0, 0);
    c.println(&vals!["with background"]).unwrap();

    color::bg_rgb(r, g, b)
        .println(&vals!["background"])
        .unwrap();

    let mut c = color::bg_rgb(r, g, b);
    c.add_rgb(255, 255, 255);
    c.println(&vals!["with foreground"]).unwrap();
}

/// `TestRGB/0`
#[test]
fn test_rgb_0() {
    let _g = serial();
    let visual = VisualCapture::install();

    run_rgb_case(0);

    assert_colorized(&visual.replay(), "TestRGB/0");
}

/// `TestRGB/1`
#[test]
fn test_rgb_1() {
    let _g = serial();
    let visual = VisualCapture::install();

    run_rgb_case(1);

    assert_colorized(&visual.replay(), "TestRGB/1");
}
