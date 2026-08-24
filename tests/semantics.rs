//! Behavioral tests for semantics that `color_test.go` leaves unverified but
//! that this port claims to reproduce exactly.
//!
//! Each test here was added because a deliberately injected bug (mutation
//! testing) survived the migrated Go suite — i.e. these cover real behavior
//! that nothing else pins down.

use std::io::{self, Write};
use std::sync::{Mutex, MutexGuard, OnceLock};

use color::{vals, Color, SharedBuffer, Value, BOLD, FG_CYAN, FG_RED, NO_ARGS, UNDERLINE};

fn serial() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let g = LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    std::env::remove_var("NO_COLOR");
    color::set_no_color(false);
    color::set_stdout_available(true);
    color::set_stderr_available(true);
    g
}

// ---------------------------------------------------------------------------
// colorsCache
// ---------------------------------------------------------------------------

/// Go memoizes the helper colors in `colorsCache`, so a `Color` created before
/// `NO_COLOR` was set keeps `noColor == nil` and therefore keeps following the
/// *global* flag rather than the environment.
///
/// Without the cache each call would build a fresh `Color`, and `New()` would
/// stamp `noColor = true` from the environment — a visible difference.
#[test]
fn cached_helper_colors_are_reused_across_env_changes() {
    let _g = serial();

    // Prime the cache while NO_COLOR is unset.
    let before = color::red_string("x", NO_ARGS);
    assert_eq!(before, "\x1b[31mx\x1b[0m");

    // Setting NO_COLOR now must NOT retroactively affect the cached color,
    // because the cached instance has no per-color override.
    std::env::set_var("NO_COLOR", "1");
    let after = color::red_string("x", NO_ARGS);
    std::env::remove_var("NO_COLOR");

    assert_eq!(
        after, before,
        "cached helper colors must be reused; a fresh Color would have picked \
         up NO_COLOR at construction and dropped the escape codes"
    );

    // A *newly constructed* color, by contrast, does honour the environment.
    std::env::set_var("NO_COLOR", "1");
    let fresh = Color::new(&[FG_RED]).sprint(&vals!["x"]);
    std::env::remove_var("NO_COLOR");
    assert_eq!(fresh, "x", "New() reads NO_COLOR at construction time");
}

/// The global `NoColor` flag still reaches cached colors, since they carry no
/// per-color override.
#[test]
fn cached_helper_colors_follow_the_global_flag() {
    let _g = serial();

    assert_eq!(color::green_string("x", NO_ARGS), "\x1b[32mx\x1b[0m");
    color::set_no_color(true);
    assert_eq!(color::green_string("x", NO_ARGS), "x");
    color::set_no_color(false);
    assert_eq!(color::green_string("x", NO_ARGS), "\x1b[32mx\x1b[0m");
}

// ---------------------------------------------------------------------------
// (n int, err error)
// ---------------------------------------------------------------------------

/// A writer that accepts `budget` bytes in total and then fails.
///
/// It obeys the `io.Writer` contract that Go's `fmt` relies on — *"Write must
/// return a non-nil error if it returns n < len(p)"* — so the numbers below can
/// be compared directly against the Go implementation.
struct Failing {
    budget: usize,
}

impl Write for Failing {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() > self.budget {
            let n = self.budget;
            self.budget = 0;
            // Short write *with* an error, as the contract requires.
            if n == 0 {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "boom"));
            }
            return Ok(n);
        }
        self.budget -= buf.len();
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Go's `Fprint` returns the bytes that actually reached the writer alongside
/// the error. `WriteError::written` must accumulate across the three internal
/// writes (set sequence, payload, reset sequence).
///
/// Reference values produced by the Go implementation with the equivalent
/// writer:
///
/// ```text
/// Fprint   budget=7 -> n=7 err=boom
/// Fprint   budget=2 -> n=2 err=boom
/// Fprint   budget=8 -> n=8 err=boom
/// Fprint   budget=100 -> n=12 err=<nil>
/// ```
#[test]
fn fprint_reports_partial_byte_count_on_failure() {
    let _g = serial();
    let c = Color::new(&[FG_RED]);

    // "\x1b[31m" is 5 bytes; allow 5 + 2 of the payload, then fail.
    let mut w = Failing { budget: 7 };
    let err = c
        .fprint(&mut w, &vals!["foobar"])
        .expect_err("the writer must fail");
    assert_eq!(err.written, 7, "partial count must survive the error");
    assert_eq!(err.source.kind(), io::ErrorKind::BrokenPipe);

    // Failing inside the leading escape sequence.
    let mut w = Failing { budget: 2 };
    let err = c.fprint(&mut w, &vals!["x"]).expect_err("must fail");
    assert_eq!(err.written, 2);

    // Failing only on the trailing reset: 5 (set) + 3 (payload) = 8 succeed.
    let mut w = Failing { budget: 8 };
    let err = c.fprint(&mut w, &vals!["foo"]).expect_err("must fail");
    assert_eq!(err.written, 8);

    // Enough budget: full output, no error. Go reports n=12.
    let mut w = Failing { budget: 100 };
    assert_eq!(c.fprint(&mut w, &vals!["foo"]).unwrap(), 12);
}

/// Reference values from the Go implementation:
///
/// ```text
/// Fprintf  budget=6 -> n=6 err=boom
/// Fprintln budget=4 -> n=4 err=boom
/// ```
#[test]
fn fprintf_and_fprintln_report_partial_byte_counts() {
    let _g = serial();
    let c = Color::new(&[FG_RED]);

    let mut w = Failing { budget: 6 };
    let err = c
        .fprintf(&mut w, "%s", &vals!["abcdef"])
        .expect_err("must fail");
    assert_eq!(err.written, 6);

    let mut w = Failing { budget: 4 };
    let err = c.fprintln(&mut w, &vals!["abcdef"]).expect_err("must fail");
    assert_eq!(err.written, 4);
}

/// Documents the single deliberate divergence in the write path.
///
/// Go's `fmt.Fprint` issues exactly one `w.Write` call and returns its result,
/// which is safe because Go's `io.Writer` contract forbids a short write
/// without an error. Rust's `io::Write::write` explicitly *permits* short
/// writes, so this port loops until the buffer is drained (the `write_all`
/// idiom). For every contract-conforming writer the two are indistinguishable;
/// they differ only for a writer that short-writes and reports success, which
/// would be a contract violation in Go.
#[test]
fn short_writes_without_error_are_retried() {
    let _g = serial();

    /// Writes at most 3 bytes per call, never errors.
    struct Trickle {
        sink: Vec<u8>,
    }
    impl Write for Trickle {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let n = buf.len().min(3);
            self.sink.extend_from_slice(&buf[..n]);
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let c = Color::new(&[FG_RED]);
    let mut w = Trickle { sink: Vec::new() };
    let n = c.fprint(&mut w, &vals!["foobar"]).unwrap();

    // Nothing is lost, and the reported count is the full length.
    assert_eq!(String::from_utf8(w.sink).unwrap(), "\x1b[31mfoobar\x1b[0m");
    assert_eq!(n, 15);
}

/// On success the count equals the bytes written, for every writer op.
#[test]
fn success_byte_counts_equal_output_length() {
    let _g = serial();
    let c = Color::new(&[FG_RED, BOLD]);

    for (name, run) in [
        (
            "fprint",
            Box::new(|c: &Color, b: &SharedBuffer| c.fprint(&mut b.clone(), &vals!["a", "b"]))
                as Box<dyn Fn(&Color, &SharedBuffer) -> color::Result<usize>>,
        ),
        (
            "fprintln",
            Box::new(|c: &Color, b: &SharedBuffer| c.fprintln(&mut b.clone(), &vals!["a", "b"])),
        ),
        (
            "fprintf",
            Box::new(|c: &Color, b: &SharedBuffer| {
                c.fprintf(&mut b.clone(), "%s-%d", &vals!["a", 7])
            }),
        ),
    ] {
        let buf = SharedBuffer::new();
        let n = run(&c, &buf).unwrap();
        assert_eq!(n, buf.string().len(), "{name} byte count mismatch");
    }
}

/// `WriteError` converts into a plain `io::Error`, keeping the source kind.
#[test]
fn write_error_converts_to_io_error() {
    let _g = serial();
    let c = Color::new(&[FG_RED]);
    let mut w = Failing { budget: 1 };
    let err = c.fprint(&mut w, &vals!["x"]).expect_err("must fail");

    let msg = format!("{err}");
    assert!(msg.contains("after 1 bytes"), "got {msg:?}");

    let io_err: io::Error = err.into();
    assert_eq!(io_err.kind(), io::ErrorKind::BrokenPipe);
}

// ---------------------------------------------------------------------------
// Color accessors & equality
// ---------------------------------------------------------------------------

#[test]
fn params_exposes_the_sgr_sequence() {
    let _g = serial();
    let mut c = Color::new(&[FG_CYAN, UNDERLINE]);
    assert_eq!(c.params(), &[FG_CYAN, UNDERLINE]);

    c.add_rgb(1, 2, 3);
    assert_eq!(
        c.params().iter().map(|a| a.code()).collect::<Vec<_>>(),
        vec![36, 4, 38, 2, 1, 2, 3]
    );

    c.add_bg_rgb(9, 8, 7);
    assert_eq!(
        c.params().iter().map(|a| a.code()).collect::<Vec<_>>(),
        vec![36, 4, 38, 2, 1, 2, 3, 48, 2, 9, 8, 7]
    );
}

#[test]
fn partial_eq_matches_the_equals_method() {
    let _g = serial();
    let a = Color::new(&[FG_RED, BOLD]);
    let b = Color::new(&[BOLD, FG_RED]);
    let c = Color::new(&[FG_RED]);

    assert_eq!(a, b, "Equals is order-insensitive");
    assert_ne!(a, c);
    assert_eq!(a == b, a.equals(&b));

    // The nil-receiver behavior of the Go method.
    assert!(color::equals(None, None));
    assert!(!color::equals(Some(&a), None));
    assert!(!color::equals(None, Some(&a)));
    assert!(color::equals(Some(&a), Some(&b)));
}

/// `DisableColor`/`EnableColor` take precedence over the global flag in both
/// directions.
#[test]
fn per_color_override_beats_the_global_flag() {
    let _g = serial();
    let mut c = Color::new(&[FG_RED]);

    color::set_no_color(true);
    assert_eq!(c.sprint(&vals!["x"]), "x");
    c.enable_color();
    assert_eq!(
        c.sprint(&vals!["x"]),
        "\x1b[31mx\x1b[0m",
        "EnableColor must override NoColor = true"
    );

    color::set_no_color(false);
    assert_eq!(c.sprint(&vals!["x"]), "\x1b[31mx\x1b[0m");
    c.disable_color();
    assert_eq!(
        c.sprint(&vals!["x"]),
        "x",
        "DisableColor must override NoColor = false"
    );
}

// ---------------------------------------------------------------------------
// Writer-targeting helpers
// ---------------------------------------------------------------------------

#[test]
fn set_writer_and_unset_writer_bracket_the_payload() {
    let _g = serial();
    let c = Color::new(&[FG_RED]);
    let buf = SharedBuffer::new();
    let mut w = buf.clone();

    c.set_writer(&mut w);
    w.write_all(b"payload").unwrap();
    c.unset_writer(&mut w);

    assert_eq!(buf.string(), "\x1b[31mpayload\x1b[0m");
}

#[test]
fn set_writer_is_a_no_op_when_color_is_disabled() {
    let _g = serial();
    let mut c = Color::new(&[FG_RED]);
    c.disable_color();

    let buf = SharedBuffer::new();
    let mut w = buf.clone();
    c.set_writer(&mut w);
    w.write_all(b"payload").unwrap();
    c.unset_writer(&mut w);

    assert_eq!(buf.string(), "payload");
}

/// The global `Error` writer is a separate slot from `Output`.
#[test]
fn error_writer_is_independent_of_output() {
    let _g = serial();
    let out = SharedBuffer::new();
    let err = SharedBuffer::new();
    color::set_output(out.writer());
    color::set_error(err.writer());

    assert_eq!(color::output(), out.writer());
    assert_eq!(color::error(), err.writer());
    assert_ne!(color::output(), color::error());

    Color::new(&[FG_RED]).print(&vals!["x"]).unwrap();
    assert_eq!(out.string(), "\x1b[31mx\x1b[0m");
    assert_eq!(err.string(), "", "Print must not touch the Error writer");

    color::set_output(color::std_out());
    color::set_error(color::std_err());
}

/// `Unset()` consults the *global* flag only — not any per-color override.
#[test]
fn package_unset_follows_the_global_flag_only() {
    let _g = serial();
    let buf = SharedBuffer::new();
    color::set_output(buf.writer());

    color::set_no_color(true);
    color::unset();
    assert_eq!(buf.string(), "", "Unset is a no-op while NoColor is set");

    color::set_no_color(false);
    color::unset();
    assert_eq!(buf.string(), "\x1b[0m");

    color::set_output(color::std_out());
}

// ---------------------------------------------------------------------------
// Value conversions
// ---------------------------------------------------------------------------

#[test]
fn value_conversions_preserve_go_types() {
    let _g = serial();

    let s = String::from("owned");
    assert_eq!(Value::from(&s), Value::Str("owned".into()));
    assert_eq!(
        Value::from(std::borrow::Cow::Borrowed("cow")),
        Value::Str("cow".into())
    );
    assert_eq!(
        Value::from(&b"ab"[..]),
        Value::Bytes(vec![b'a', b'b']),
        "&[u8] maps to Go []byte"
    );
    assert_eq!(
        Value::from(vec![Value::Int(1)]),
        Value::Slice(vec![Value::Int(1)])
    );
    assert_eq!(Value::from(&Value::Int(3)), Value::Int(3));
    assert_eq!(Value::from(Some(5i32)), Value::Int(5));
    assert_eq!(Value::from(Option::<i32>::None), Value::Nil);
    assert_eq!(Value::from(1u8), Value::Uint8(1), "u8 is Go's uint8");
    assert_eq!(Value::from(1u16), Value::Uint(1));

    // Display renders the value the way %v would.
    assert_eq!(Value::Int(-3).to_string(), "-3");
    assert_eq!(Value::Nil.to_string(), "<nil>");
    assert_eq!(Value::Bytes(vec![1, 2]).to_string(), "[1 2]");
    assert_eq!(Value::Error("boom".into()).to_string(), "boom");
    assert_eq!(Value::Float(1.5).to_string(), "1.5");
}
