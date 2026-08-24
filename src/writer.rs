//! The `io.Writer` model, plus the `go-isatty` / `go-colorable` behavior that
//! `color.go` depends on.
//!
//! Go's `io.Writer` is an interface, so `color.Output` can be swapped for a
//! `*bytes.Buffer` in tests and compared against `io.Discard` with `==`. To
//! keep those semantics — in particular *identity* comparison — the writer is
//! modelled as an enum rather than a bare `Box<dyn Write>`.

use std::io::{self, IsTerminal, Read, Write};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// bytes.Buffer / strings.Builder
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct BufferInner {
    data: Vec<u8>,
    /// Read cursor. `bytes.Buffer` consumes bytes as they are read.
    pos: usize,
}

/// A cheaply clonable, shared, growable buffer.
///
/// Plays the role of both `bytes.Buffer` and `strings.Builder` from the Go
/// tests: writes append, reads consume from the front, and all clones share
/// the same underlying storage (Go shares by pointer, `&bytes.Buffer`).
#[derive(Clone, Debug, Default)]
pub struct SharedBuffer {
    inner: Arc<Mutex<BufferInner>>,
}

impl SharedBuffer {
    /// Creates an empty buffer — `new(bytes.Buffer)`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a [`Writer`] handle onto this buffer.
    pub fn writer(&self) -> Writer {
        Writer::Buffer(self.clone())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BufferInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Port of `(*bytes.Buffer).String`: the contents of the *unread* portion.
    pub fn string(&self) -> String {
        let g = self.lock();
        String::from_utf8_lossy(&g.data[g.pos..]).into_owned()
    }

    /// Port of `(*bytes.Buffer).Bytes`.
    pub fn bytes(&self) -> Vec<u8> {
        let g = self.lock();
        g.data[g.pos..].to_vec()
    }

    /// Port of `(*bytes.Buffer).Len`: the number of unread bytes.
    pub fn len(&self) -> usize {
        let g = self.lock();
        g.data.len() - g.pos
    }

    /// Reports whether the unread portion is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Port of `(*bytes.Buffer).Reset`.
    pub fn reset(&self) {
        let mut g = self.lock();
        g.data.clear();
        g.pos = 0;
    }

    /// Port of `(*bytes.Buffer).ReadString(delim)`.
    ///
    /// Reads until the first occurrence of `delim`, returning a string
    /// containing the data up to *and including* the delimiter. If the
    /// delimiter is not found, it returns the data read so far together with
    /// `io.EOF` — exactly like Go, whose callers in the test-suite discard the
    /// error with `line, _ := rb.ReadString('\n')`.
    pub fn read_string(&self, delim: u8) -> (String, Option<io::Error>) {
        let mut g = self.lock();
        let start = g.pos;
        let hay = &g.data[start..];

        match hay.iter().position(|&b| b == delim) {
            Some(idx) => {
                let end = start + idx + 1;
                let out = String::from_utf8_lossy(&g.data[start..end]).into_owned();
                g.pos = end;
                (out, None)
            }
            None => {
                let out = String::from_utf8_lossy(hay).into_owned();
                g.pos = g.data.len();
                // bytes.Buffer reports io.EOF whether or not anything was read.
                (
                    out,
                    Some(io::Error::new(io::ErrorKind::UnexpectedEof, "EOF")),
                )
            }
        }
    }

    /// Port of `io.ReadAll(buf)`: drains and returns the unread portion.
    pub fn read_all(&self) -> String {
        let mut g = self.lock();
        let out = String::from_utf8_lossy(&g.data[g.pos..]).into_owned();
        g.pos = g.data.len();
        out
    }
}

impl Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.lock().data.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Read for SharedBuffer {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let mut g = self.lock();
        let n = (g.data.len() - g.pos).min(out.len());
        out[..n].copy_from_slice(&g.data[g.pos..g.pos + n]);
        g.pos += n;
        Ok(n)
    }
}

// ---------------------------------------------------------------------------
// io.Writer
// ---------------------------------------------------------------------------

/// The `io.Writer` interface.
///
/// Variants map onto the concrete writers `color` uses:
///
/// | Rust                | Go                              |
/// |---------------------|---------------------------------|
/// | [`Writer::Discard`] | `io.Discard`                    |
/// | [`Writer::Stdout`]  | `colorable.NewColorableStdout()`|
/// | [`Writer::Stderr`]  | `colorable.NewColorableStderr()`|
/// | [`Writer::Buffer`]  | `*bytes.Buffer`/`*strings.Builder` |
/// | [`Writer::Custom`]  | any user-supplied `io.Writer`   |
#[derive(Clone, Default)]
pub enum Writer {
    /// `io.Discard` — all writes succeed and are thrown away.
    #[default]
    Discard,
    /// Colorable standard output.
    Stdout,
    /// Colorable standard error.
    Stderr,
    /// An in-memory buffer.
    Buffer(SharedBuffer),
    /// An arbitrary user-supplied writer.
    Custom(Arc<Mutex<dyn Write + Send>>),
}

impl Writer {
    /// Wraps any `Write + Send` value into a [`Writer::Custom`].
    pub fn custom<W: Write + Send + 'static>(w: W) -> Writer {
        Writer::Custom(Arc::new(Mutex::new(w)))
    }

    /// Reports whether this writer is `io.Discard`.
    pub fn is_discard(&self) -> bool {
        matches!(self, Writer::Discard)
    }
}

impl PartialEq for Writer {
    /// Interface identity comparison, mirroring Go's `got != io.Discard`.
    ///
    /// `Buffer` and `Custom` compare by *pointer*, exactly like comparing two
    /// Go interface values holding the same pointer.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Writer::Discard, Writer::Discard) => true,
            (Writer::Stdout, Writer::Stdout) => true,
            (Writer::Stderr, Writer::Stderr) => true,
            (Writer::Buffer(a), Writer::Buffer(b)) => Arc::ptr_eq(&a.inner, &b.inner),
            (Writer::Custom(a), Writer::Custom(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl std::fmt::Debug for Writer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Writer::Discard => f.write_str("io.Discard"),
            Writer::Stdout => f.write_str("colorable.Stdout"),
            Writer::Stderr => f.write_str("colorable.Stderr"),
            Writer::Buffer(_) => f.write_str("*bytes.Buffer"),
            Writer::Custom(_) => f.write_str("io.Writer"),
        }
    }
}

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Writer::Discard => Ok(buf.len()),
            Writer::Stdout => {
                ensure_windows_vt_enabled();
                let stdout = io::stdout();
                let mut lock = stdout.lock();
                let n = lock.write(buf)?;
                // Terminals are line buffered in Go; flush so that interleaved
                // writes from the test-suite appear in order.
                lock.flush()?;
                Ok(n)
            }
            Writer::Stderr => {
                ensure_windows_vt_enabled();
                let stderr = io::stderr();
                let mut lock = stderr.lock();
                let n = lock.write(buf)?;
                lock.flush()?;
                Ok(n)
            }
            Writer::Buffer(b) => b.write(buf),
            Writer::Custom(c) => {
                let mut guard = c.lock().unwrap_or_else(|e| e.into_inner());
                guard.write(buf)
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Writer::Discard => Ok(()),
            Writer::Stdout => io::stdout().flush(),
            Writer::Stderr => io::stderr().flush(),
            Writer::Buffer(b) => b.flush(),
            Writer::Custom(c) => {
                let mut guard = c.lock().unwrap_or_else(|e| e.into_inner());
                guard.flush()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Standard handle availability — emulation of `os.Stdout == nil`
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicBool, Ordering};

/// In Go, `os.Stdout` is a package variable that can legally be `nil` (it is
/// `nil` for a Windows service, and the test-suite assigns `nil` to it
/// directly). Rust's `io::stdout()` can never be absent, so the condition is
/// modelled as an overridable flag with the same observable effect.
static STDOUT_AVAILABLE: AtomicBool = AtomicBool::new(true);
static STDERR_AVAILABLE: AtomicBool = AtomicBool::new(true);

/// Sets whether standard output is considered available.
///
/// Equivalent to assigning `os.Stdout = nil` (`false`) or restoring it
/// (`true`).
pub fn set_stdout_available(available: bool) {
    STDOUT_AVAILABLE.store(available, Ordering::SeqCst);
}

/// Reports whether standard output is considered available (`os.Stdout != nil`).
pub fn stdout_available() -> bool {
    STDOUT_AVAILABLE.load(Ordering::SeqCst)
}

/// Sets whether standard error is considered available.
///
/// Equivalent to assigning `os.Stderr = nil` (`false`) or restoring it
/// (`true`).
pub fn set_stderr_available(available: bool) {
    STDERR_AVAILABLE.store(available, Ordering::SeqCst);
}

/// Reports whether standard error is considered available (`os.Stderr != nil`).
pub fn stderr_available() -> bool {
    STDERR_AVAILABLE.load(Ordering::SeqCst)
}

// ---------------------------------------------------------------------------
// isatty
// ---------------------------------------------------------------------------

/// Port of `isatty.IsTerminal(os.Stdout.Fd())`.
fn is_terminal_stdout() -> bool {
    io::stdout().is_terminal()
}

/// Port of `isatty.IsCygwinTerminal(os.Stdout.Fd())`.
///
/// On non-Windows platforms `go-isatty` always returns `false`. On Windows,
/// `IsTerminal` above already covers the modern console; MSYS/Cygwin PTYs are
/// detected heuristically through the environment they always export.
fn is_cygwin_terminal_stdout() -> bool {
    #[cfg(windows)]
    {
        // go-isatty inspects the NT object name of the handle
        // (`\msys-…-pty…` / `\cygwin-…-pty…`). The MSYSTEM/TERM pair is the
        // portable proxy for the same condition.
        if std::env::var_os("MSYSTEM").is_some() {
            return true;
        }
        match std::env::var("TERM") {
            Ok(t) => t == "cygwin" || t == "xterm" || t.starts_with("xterm-"),
            Err(_) => false,
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// The decision logic of `stdoutIsTerminal`, isolated from the actual OS
/// probes so that the `os.Stdout == nil` short circuit can be tested on a
/// machine whose stdout is not a terminal (which is the case under any test
/// runner).
fn stdout_is_terminal_impl(
    available: bool,
    is_tty: impl FnOnce() -> bool,
    is_cygwin: impl FnOnce() -> bool,
) -> bool {
    if !available {
        return false;
    }
    is_tty() || is_cygwin()
}

/// Port of `stdoutIsTerminal`.
///
/// > returns true if os.Stdout is a terminal.
/// > Returns false if os.Stdout is nil (e.g., when running as a Windows service).
pub fn stdout_is_terminal() -> bool {
    stdout_is_terminal_impl(
        stdout_available(),
        is_terminal_stdout,
        is_cygwin_terminal_stdout,
    )
}

/// Port of `stdOut`.
///
/// > returns a writer for color output.
/// > Returns io.Discard if os.Stdout is nil (e.g., when running as a Windows service).
pub fn std_out() -> Writer {
    if !stdout_available() {
        return Writer::Discard;
    }
    new_colorable_stdout()
}

/// Port of `stdErr`.
///
/// > returns a writer for color error output.
/// > Returns io.Discard if os.Stderr is nil (e.g., when running as a Windows service).
pub fn std_err() -> Writer {
    if !stderr_available() {
        return Writer::Discard;
    }
    new_colorable_stderr()
}

/// Port of `colorable.NewColorableStdout()`.
///
/// On POSIX systems `go-colorable` returns `os.Stdout` unchanged; on Windows it
/// returns a writer that understands ANSI sequences. Modern Windows consoles
/// interpret ANSI natively once virtual-terminal processing is enabled, which
/// [`ensure_windows_vt_enabled`] does on first write.
pub fn new_colorable_stdout() -> Writer {
    Writer::Stdout
}

/// Port of `colorable.NewColorableStderr()`.
pub fn new_colorable_stderr() -> Writer {
    Writer::Stderr
}

// ---------------------------------------------------------------------------
// Windows virtual terminal support — port of color_windows.go
// ---------------------------------------------------------------------------

/// Port of the `init()` in `color_windows.go`.
///
/// > Opt-in for ansi color support for current process.
/// > <https://learn.microsoft.com/en-us/windows/console/console-virtual-terminal-sequences#output-sequences>
///
/// Go runs this at package initialization. Rust has no life-before-`main`, so
/// it runs exactly once, lazily, before the first write to a standard handle.
pub fn ensure_windows_vt_enabled() {
    #[cfg(windows)]
    {
        use std::sync::Once;
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
            use windows_sys::Win32::System::Console::{
                GetConsoleMode, GetStdHandle, SetConsoleMode, ENABLE_PROCESSED_OUTPUT,
                ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_OUTPUT_HANDLE,
            };

            // if os.Stdout == nil { return }
            if !stdout_available() {
                return;
            }
            unsafe {
                let handle = GetStdHandle(STD_OUTPUT_HANDLE);
                if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                    return;
                }
                let mut out_mode: u32 = 0;
                if GetConsoleMode(handle, &mut out_mode) == 0 {
                    return;
                }
                out_mode |= ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
                let _ = SetConsoleMode(handle, out_mode);
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Write helpers mirroring fmt.Fprint's (n int, err error) contract
// ---------------------------------------------------------------------------

/// Writes `s` to `w`, returning the number of bytes written.
///
/// Go's `fmt.Fprint` family returns the byte count *and* the error, with the
/// count reflecting the bytes that actually made it out before any failure.
/// `io::Write::write_all` discards that partial count, so the loop is written
/// out explicitly to preserve the contract.
pub(crate) fn write_string<W: Write + ?Sized>(w: &mut W, s: &str) -> (usize, io::Result<()>) {
    let mut buf = s.as_bytes();
    let mut written = 0usize;
    while !buf.is_empty() {
        match w.write(buf) {
            Ok(0) => {
                return (
                    written,
                    Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to write whole buffer",
                    )),
                );
            }
            Ok(n) => {
                written += n;
                buf = &buf[n..];
            }
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return (written, Err(e)),
        }
    }
    (written, Ok(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_read_string_consumes_like_bytes_buffer() {
        let mut b = SharedBuffer::new();
        b.write_all(b"one\ntwo\n").unwrap();

        let (line, err) = b.read_string(b'\n');
        assert_eq!(line, "one\n");
        assert!(err.is_none());

        let (line, err) = b.read_string(b'\n');
        assert_eq!(line, "two\n");
        assert!(err.is_none());

        let (line, err) = b.read_string(b'\n');
        assert_eq!(line, "");
        assert!(err.is_some(), "EOF expected when the delimiter is missing");
    }

    #[test]
    fn buffer_read_string_without_delimiter_returns_rest_and_eof() {
        let mut b = SharedBuffer::new();
        b.write_all(b"no newline").unwrap();
        let (line, err) = b.read_string(b'\n');
        assert_eq!(line, "no newline");
        assert!(err.is_some());
    }

    #[test]
    fn writer_identity_matches_go_interface_comparison() {
        assert_eq!(Writer::Discard, Writer::Discard);
        assert_ne!(Writer::Discard, Writer::Stdout);

        let b = SharedBuffer::new();
        assert_eq!(b.writer(), b.writer());
        assert_ne!(b.writer(), SharedBuffer::new().writer());
    }

    #[test]
    fn shared_buffer_clones_share_storage() {
        let a = SharedBuffer::new();
        let mut b = a.clone();
        b.write_all(b"hello").unwrap();
        assert_eq!(a.string(), "hello");
    }

    /// The `os.Stdout == nil` short circuit must win even when the underlying
    /// handle *is* a terminal — the case the Go test cannot express in Rust
    /// because a nil dereference is impossible here.
    #[test]
    fn stdout_is_terminal_honours_nil_stdout() {
        // available == false short circuits regardless of the probes.
        assert!(!stdout_is_terminal_impl(false, || true, || true));
        assert!(!stdout_is_terminal_impl(false, || true, || false));
        assert!(!stdout_is_terminal_impl(false, || false, || true));
        assert!(!stdout_is_terminal_impl(false, || false, || false));

        // available == true delegates to `IsTerminal || IsCygwinTerminal`.
        assert!(stdout_is_terminal_impl(true, || true, || false));
        assert!(stdout_is_terminal_impl(true, || false, || true));
        assert!(stdout_is_terminal_impl(true, || true, || true));
        assert!(!stdout_is_terminal_impl(true, || false, || false));
    }

    #[test]
    fn discard_swallows_writes_and_reports_full_length() {
        let mut w = Writer::Discard;
        assert_eq!(w.write(b"hello").unwrap(), 5);
        w.flush().unwrap();
        assert!(w.is_discard());
        assert!(!Writer::Stdout.is_discard());
        assert_eq!(Writer::default(), Writer::Discard);
    }

    #[test]
    fn custom_writer_forwards_to_the_wrapped_value() {
        let sink = Arc::new(Mutex::new(Vec::<u8>::new()));
        let mut w = Writer::Custom(sink.clone());
        w.write_all(b"abc").unwrap();
        w.flush().unwrap();
        assert_eq!(&*sink.lock().unwrap(), b"abc");

        // Interface identity: same pointer is equal, a different one is not.
        assert_eq!(Writer::Custom(sink.clone()), Writer::Custom(sink));
        assert_ne!(
            Writer::custom(Vec::<u8>::new()),
            Writer::custom(Vec::<u8>::new())
        );
    }

    #[test]
    fn writer_equality_covers_every_variant_pair() {
        assert_eq!(Writer::Stdout, Writer::Stdout);
        assert_eq!(Writer::Stderr, Writer::Stderr);
        assert_ne!(Writer::Stdout, Writer::Stderr);
        assert_ne!(Writer::Stdout, Writer::Discard);
        assert_ne!(Writer::Stderr, SharedBuffer::new().writer());
    }

    #[test]
    fn writer_debug_names_the_go_counterpart() {
        assert_eq!(format!("{:?}", Writer::Discard), "io.Discard");
        assert_eq!(format!("{:?}", Writer::Stdout), "colorable.Stdout");
        assert_eq!(format!("{:?}", Writer::Stderr), "colorable.Stderr");
        assert_eq!(
            format!("{:?}", SharedBuffer::new().writer()),
            "*bytes.Buffer"
        );
        assert_eq!(
            format!("{:?}", Writer::custom(Vec::<u8>::new())),
            "io.Writer"
        );
    }

    #[test]
    fn shared_buffer_implements_read() {
        let mut b = SharedBuffer::new();
        b.write_all(b"abcdef").unwrap();

        let mut out = [0u8; 4];
        assert_eq!(b.read(&mut out).unwrap(), 4);
        assert_eq!(&out, b"abcd");
        // Reads consume, like bytes.Buffer.
        assert_eq!(b.string(), "ef");

        let mut rest = Vec::new();
        b.read_to_end(&mut rest).unwrap();
        assert_eq!(rest, b"ef");
        assert_eq!(b.read(&mut out).unwrap(), 0);
    }

    #[test]
    fn write_string_reports_partial_count_on_failure() {
        /// A writer that accepts `budget` bytes and then fails, obeying the
        /// `io.Writer` contract (`n < len(p)` implies a non-nil error).
        struct Failing {
            budget: usize,
        }
        impl Write for Failing {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                if buf.len() > self.budget {
                    let n = self.budget;
                    self.budget = 0;
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

        let mut w = Failing { budget: 3 };
        let (n, res) = write_string(&mut w, "abcdef");
        assert_eq!(n, 3, "the bytes that made it out must be reported");
        assert!(res.is_err());

        let mut w = Failing { budget: 100 };
        let (n, res) = write_string(&mut w, "abcdef");
        assert_eq!(n, 6);
        assert!(res.is_ok());
    }
}
