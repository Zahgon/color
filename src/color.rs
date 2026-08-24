//! Port of `color.go`.
//!
//! Every exported (and unexported) item from the Go file has a counterpart
//! here, keeping the same control flow, the same side effects and the same
//! observable output.

use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::attribute::{map_reset_attributes, Attribute, BACKGROUND, FOREGROUND, RESET};
use crate::gofmt;
use crate::value::Value;
use crate::writer::{self, Writer};

/// `const escape = "\x1b"`
const ESCAPE: &str = "\x1b";

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// The error half of Go's `(n int, err error)` print contract.
///
/// Go's `fmt.Fprint` family reports both how many bytes reached the writer and
/// what went wrong. `io::Result<usize>` can only carry one of the two, so the
/// partial count travels along with the error.
#[derive(Debug)]
pub struct WriteError {
    /// Bytes successfully written before the failure — Go's `n`.
    pub written: usize,
    /// The underlying I/O error — Go's `err`.
    pub source: io::Error,
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (after {} bytes)", self.source, self.written)
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl From<WriteError> for io::Error {
    fn from(e: WriteError) -> io::Error {
        e.source
    }
}

/// Result of the printing functions: the byte count on success, or the byte
/// count plus the error on failure.
pub type Result<T> = std::result::Result<T, WriteError>;

/// Writes `s` to `w`, mirroring `fmt.Fprint(w, s)`'s `(n, err)` return.
fn fprint_str<W: Write + ?Sized>(w: &mut W, s: &str) -> Result<usize> {
    let (n, r) = writer::write_string(w, s);
    match r {
        Ok(()) => Ok(n),
        Err(source) => Err(WriteError { written: n, source }),
    }
}

/// Adds `extra` bytes to the count carried by a [`WriteError`].
fn advance(err: WriteError, already: usize) -> WriteError {
    WriteError {
        written: already + err.written,
        source: err.source,
    }
}

// ---------------------------------------------------------------------------
// Package level variables
// ---------------------------------------------------------------------------

static NO_COLOR: OnceLock<AtomicBool> = OnceLock::new();
static OUTPUT: OnceLock<Mutex<Writer>> = OnceLock::new();
static ERROR: OnceLock<Mutex<Writer>> = OnceLock::new();

/// `colorsCache` + `colorsCacheMu`.
///
/// > colorsCache is used to reduce the count of created Color objects and
/// > allows to reuse already created objects with required Attribute.
static COLORS_CACHE: OnceLock<Mutex<HashMap<Attribute, Arc<Color>>>> = OnceLock::new();

fn no_color_cell() -> &'static AtomicBool {
    NO_COLOR.get_or_init(|| {
        // NoColor = noColorIsSet() || os.Getenv("TERM") == "dumb" || !stdoutIsTerminal()
        let initial = no_color_is_set()
            || std::env::var("TERM").map(|t| t == "dumb").unwrap_or(false)
            || !writer::stdout_is_terminal();
        AtomicBool::new(initial)
    })
}

/// Reads the global `NoColor` flag.
///
/// > NoColor defines if the output is colorized or not. It's dynamically set to
/// > false or true based on the stdout's file descriptor referring to a terminal
/// > or not. It's also set to true if the NO_COLOR environment variable is
/// > set (regardless of its value). This is a global option and affects all
/// > colors. For more control over each color block use the methods
/// > [`Color::disable_color`] individually.
///
/// # Initialization timing
///
/// Go evaluates this expression during package initialization, i.e. before
/// `main` runs. Rust has no life-before-`main`, so the value is computed on
/// first access instead. The distinction is only observable if a program
/// mutates `NO_COLOR`/`TERM` or replaces its own stdout *after* start but
/// *before* the first use of this crate; call [`set_no_color`] explicitly if
/// you need to pin the decision.
pub fn no_color() -> bool {
    no_color_cell().load(Ordering::SeqCst)
}

/// Assigns the global `NoColor` flag — the equivalent of `color.NoColor = v`.
pub fn set_no_color(v: bool) {
    no_color_cell().store(v, Ordering::SeqCst);
}

fn output_cell() -> &'static Mutex<Writer> {
    OUTPUT.get_or_init(|| Mutex::new(writer::std_out()))
}

fn error_cell() -> &'static Mutex<Writer> {
    ERROR.get_or_init(|| Mutex::new(writer::std_err()))
}

/// Reads the global `Output` writer.
///
/// > Output defines the standard output of the print functions. By default,
/// > stdOut() is used.
///
/// The returned [`Writer`] is a handle that shares its destination with the
/// stored one, exactly like copying a Go interface value.
pub fn output() -> Writer {
    output_cell()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Assigns the global `Output` writer — the equivalent of `color.Output = w`.
pub fn set_output(w: Writer) {
    *output_cell().lock().unwrap_or_else(|e| e.into_inner()) = w;
}

/// Reads the global `Error` writer.
///
/// > Error defines the standard error of the print functions. By default,
/// > stdErr() is used.
pub fn error() -> Writer {
    error_cell()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Assigns the global `Error` writer — the equivalent of `color.Error = w`.
pub fn set_error(w: Writer) {
    *error_cell().lock().unwrap_or_else(|e| e.into_inner()) = w;
}

/// Port of `noColorIsSet`.
///
/// > returns true if the environment variable NO_COLOR is set to a non-empty string.
pub(crate) fn no_color_is_set() -> bool {
    match std::env::var("NO_COLOR") {
        Ok(v) => !v.is_empty(),
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// Color
// ---------------------------------------------------------------------------

/// Color defines a custom color object which is defined by SGR parameters.
///
/// Port of Go's
///
/// ```go
/// type Color struct {
///     params  []Attribute
///     noColor *bool
/// }
/// ```
///
/// `*bool` becomes `Option<bool>`: `None` defers to the global
/// [`no_color`] flag, `Some(v)` is an explicit per-color override installed by
/// [`Color::disable_color`] / [`Color::enable_color`].
#[derive(Clone, Debug, Default)]
pub struct Color {
    params: Vec<Attribute>,
    no_color: Option<bool>,
}

impl Color {
    /// Port of `New(value ...Attribute) *Color`.
    ///
    /// > New returns a newly created color object.
    pub fn new(value: &[Attribute]) -> Color {
        let mut c = Color {
            params: Vec::new(),
            no_color: None,
        };

        if no_color_is_set() {
            c.no_color = Some(true);
        }

        c.add(value);
        c
    }

    /// The SGR parameters that make up this color.
    pub fn params(&self) -> &[Attribute] {
        &self.params
    }

    /// Port of `(*Color).AddRGB`.
    ///
    /// > AddRGB is used to chain foreground RGB SGR parameters. Use as many as
    /// > parameters to combine and create custom color objects.
    /// > Example: `.Add(34, 0, 12).Add(255, 128, 0)`.
    pub fn add_rgb(&mut self, r: i32, g: i32, b: i32) -> &mut Self {
        self.params.extend_from_slice(&[
            FOREGROUND,
            Attribute(2),
            Attribute(r),
            Attribute(g),
            Attribute(b),
        ]);
        self
    }

    /// Port of `(*Color).AddBgRGB`.
    ///
    /// > AddBgRGB is used to chain background RGB SGR parameters. Use as many as
    /// > parameters to combine and create custom color objects.
    /// > Example: `.Add(34, 0, 12).Add(255, 128, 0)`.
    pub fn add_bg_rgb(&mut self, r: i32, g: i32, b: i32) -> &mut Self {
        self.params.extend_from_slice(&[
            BACKGROUND,
            Attribute(2),
            Attribute(r),
            Attribute(g),
            Attribute(b),
        ]);
        self
    }

    /// Port of `(*Color).Set`.
    ///
    /// > Set sets the SGR sequence.
    pub fn set(&self) -> &Self {
        if self.is_no_color_set() {
            return self;
        }

        let mut w = output();
        let _ = fprint_str(&mut w, &self.format());
        self
    }

    /// Port of the unexported `(*Color).unset`.
    fn unset_internal(&self) {
        if self.is_no_color_set() {
            return;
        }

        unset();
    }

    /// Port of `(*Color).SetWriter`.
    ///
    /// > SetWriter is used to set the SGR sequence with the given io.Writer. This is
    /// > a low-level function, and users should use the higher-level functions, such
    /// > as `Fprint`, `Print`, etc.
    pub fn set_writer<W: Write + ?Sized>(&self, w: &mut W) -> &Self {
        let _ = self.set_writer_n(w);
        self
    }

    /// Port of the unexported `(*Color).setWriter`.
    fn set_writer_n<W: Write + ?Sized>(&self, w: &mut W) -> Result<usize> {
        if self.is_no_color_set() {
            return Ok(0);
        }

        fprint_str(w, &self.format())
    }

    /// Port of `(*Color).UnsetWriter`.
    ///
    /// > UnsetWriter resets all escape attributes and clears the output with the given
    /// > io.Writer. Usually should be called after `SetWriter`.
    pub fn unset_writer<W: Write + ?Sized>(&self, w: &mut W) {
        let _ = self.unset_writer_n(w);
    }

    /// Port of the unexported `(*Color).unsetWriter`.
    fn unset_writer_n<W: Write + ?Sized>(&self, w: &mut W) -> Result<usize> {
        if self.is_no_color_set() {
            return Ok(0);
        }

        fprint_str(w, &format!("{}[{}m", ESCAPE, RESET))
    }

    /// Port of `(*Color).Add`.
    ///
    /// > Add is used to chain SGR parameters. Use as many as parameters to combine
    /// > and create custom color objects. Example: `Add(color.FgRed, color.Underline)`.
    ///
    /// Like the Go method this mutates the receiver in place and returns it for
    /// chaining.
    pub fn add(&mut self, value: &[Attribute]) -> &mut Self {
        self.params.extend_from_slice(value);
        self
    }

    /// Port of `(*Color).Fprint`.
    ///
    /// > Fprint formats using the default formats for its operands and writes to w.
    /// > Spaces are added between operands when neither is a string.
    /// > It returns the number of bytes written and any write error encountered.
    pub fn fprint<W: Write + ?Sized>(&self, w: &mut W, a: &[Value]) -> Result<usize> {
        let mut n = self.set_writer_n(w)?;

        match fprint_str(w, &gofmt::sprint(a)) {
            Ok(nn) => n += nn,
            Err(e) => return Err(advance(e, n)),
        }

        match self.unset_writer_n(w) {
            Ok(nn) => Ok(n + nn),
            Err(e) => Err(advance(e, n)),
        }
    }

    /// Port of `(*Color).Print`.
    ///
    /// > Print formats using the default formats for its operands and writes to
    /// > standard output. Spaces are added between operands when neither is a
    /// > string. It returns the number of bytes written and any write error
    /// > encountered. This is the standard `fmt.Print()` method wrapped with the
    /// > given color.
    pub fn print(&self, a: &[Value]) -> Result<usize> {
        self.set();
        // `defer c.unset()` — runs after the return value has been computed.
        let res = {
            let mut w = output();
            fprint_str(&mut w, &gofmt::sprint(a))
        };
        self.unset_internal();
        res
    }

    /// Port of `(*Color).Fprintf`.
    ///
    /// > Fprintf formats according to a format specifier and writes to w.
    /// > It returns the number of bytes written and any write error encountered.
    pub fn fprintf<W: Write + ?Sized>(
        &self,
        w: &mut W,
        format: &str,
        a: &[Value],
    ) -> Result<usize> {
        let mut n = self.set_writer_n(w)?;

        match fprint_str(w, &gofmt::sprintf(format, a)) {
            Ok(nn) => n += nn,
            Err(e) => return Err(advance(e, n)),
        }

        match self.unset_writer_n(w) {
            Ok(nn) => Ok(n + nn),
            Err(e) => Err(advance(e, n)),
        }
    }

    /// Port of `(*Color).Printf`.
    ///
    /// > Printf formats according to a format specifier and writes to standard output.
    /// > It returns the number of bytes written and any write error encountered.
    /// > This is the standard `fmt.Printf()` method wrapped with the given color.
    pub fn printf(&self, format: &str, a: &[Value]) -> Result<usize> {
        self.set();
        let res = {
            let mut w = output();
            fprint_str(&mut w, &gofmt::sprintf(format, a))
        };
        self.unset_internal();
        res
    }

    /// Port of `(*Color).Fprintln`.
    ///
    /// > Fprintln formats using the default formats for its operands and writes to w.
    /// > Spaces are always added between operands and a newline is appended.
    pub fn fprintln<W: Write + ?Sized>(&self, w: &mut W, a: &[Value]) -> Result<usize> {
        // fmt.Fprintln(w, c.wrap(sprintln(a...)))
        let s = self.wrap(&sprintln(a));
        fprint_str(w, &format!("{}\n", s))
    }

    /// Port of `(*Color).Println`.
    ///
    /// > Println formats using the default formats for its operands and writes to
    /// > standard output. Spaces are always added between operands and a newline is
    /// > appended. It returns the number of bytes written and any write error
    /// > encountered. This is the standard `fmt.Print()` method wrapped with the
    /// > given color.
    pub fn println(&self, a: &[Value]) -> Result<usize> {
        let s = self.wrap(&sprintln(a));
        let mut w = output();
        fprint_str(&mut w, &format!("{}\n", s))
    }

    /// Port of `(*Color).Sprint`.
    ///
    /// > Sprint is just like Print, but returns a string instead of printing it.
    pub fn sprint(&self, a: &[Value]) -> String {
        self.wrap(&gofmt::sprint(a))
    }

    /// Port of `(*Color).Sprintln`.
    ///
    /// > Sprintln is just like Println, but returns a string instead of printing it.
    pub fn sprintln(&self, a: &[Value]) -> String {
        format!("{}\n", self.wrap(&sprintln(a)))
    }

    /// Port of `(*Color).Sprintf`.
    ///
    /// > Sprintf is just like Printf, but returns a string instead of printing it.
    pub fn sprintf(&self, format: &str, a: &[Value]) -> String {
        self.wrap(&gofmt::sprintf(format, a))
    }

    // -----------------------------------------------------------------------
    // Closure factories
    //
    // Go's closures capture the *Color pointer. The Rust closures capture a
    // clone so that they are `'static` and can outlive the receiver, which is
    // what makes `Color::new(&[FG_BLUE]).println_func()` work the same way
    // `New(FgBlue).PrintlnFunc()` does.
    // -----------------------------------------------------------------------

    /// Port of `(*Color).FprintFunc`.
    ///
    /// > FprintFunc returns a new function that prints the passed arguments as
    /// > colorized with `Fprint()`.
    pub fn fprint_func(&self) -> impl Fn(&mut dyn Write, &[Value]) {
        let c = self.clone();
        move |w, a| {
            let _ = c.fprint(w, a);
        }
    }

    /// Port of `(*Color).PrintFunc`.
    ///
    /// > PrintFunc returns a new function that prints the passed arguments as
    /// > colorized with `Print()`.
    pub fn print_func(&self) -> impl Fn(&[Value]) {
        let c = self.clone();
        move |a| {
            let _ = c.print(a);
        }
    }

    /// Port of `(*Color).FprintfFunc`.
    ///
    /// > FprintfFunc returns a new function that prints the passed arguments as
    /// > colorized with `Fprintf()`.
    pub fn fprintf_func(&self) -> impl Fn(&mut dyn Write, &str, &[Value]) {
        let c = self.clone();
        move |w, format, a| {
            let _ = c.fprintf(w, format, a);
        }
    }

    /// Port of `(*Color).PrintfFunc`.
    ///
    /// > PrintfFunc returns a new function that prints the passed arguments as
    /// > colorized with `Printf()`.
    pub fn printf_func(&self) -> impl Fn(&str, &[Value]) {
        let c = self.clone();
        move |format, a| {
            let _ = c.printf(format, a);
        }
    }

    /// Port of `(*Color).FprintlnFunc`.
    ///
    /// > FprintlnFunc returns a new function that prints the passed arguments as
    /// > colorized with `Fprintln()`.
    pub fn fprintln_func(&self) -> impl Fn(&mut dyn Write, &[Value]) {
        let c = self.clone();
        move |w, a| {
            let _ = c.fprintln(w, a);
        }
    }

    /// Port of `(*Color).PrintlnFunc`.
    ///
    /// > PrintlnFunc returns a new function that prints the passed arguments as
    /// > colorized with `Println()`.
    pub fn println_func(&self) -> impl Fn(&[Value]) {
        let c = self.clone();
        move |a| {
            let _ = c.println(a);
        }
    }

    /// Port of `(*Color).SprintFunc`.
    ///
    /// > SprintFunc returns a new function that returns colorized strings for the
    /// > given arguments with `fmt.Sprint()`. Useful to put into or mix into other
    /// > string.
    /// >
    /// > ```text
    /// > put := New(FgYellow).SprintFunc()
    /// > fmt.Fprintf(color.Output, "This is a %s", put("warning"))
    /// > ```
    pub fn sprint_func(&self) -> impl Fn(&[Value]) -> String {
        let c = self.clone();
        move |a| c.wrap(&gofmt::sprint(a))
    }

    /// Port of `(*Color).SprintfFunc`.
    ///
    /// > SprintfFunc returns a new function that returns colorized strings for the
    /// > given arguments with `fmt.Sprintf()`.
    pub fn sprintf_func(&self) -> impl Fn(&str, &[Value]) -> String {
        let c = self.clone();
        move |format, a| c.wrap(&gofmt::sprintf(format, a))
    }

    /// Port of `(*Color).SprintlnFunc`.
    ///
    /// > SprintlnFunc returns a new function that returns colorized strings for the
    /// > given arguments with `fmt.Sprintln()`.
    pub fn sprintln_func(&self) -> impl Fn(&[Value]) -> String {
        let c = self.clone();
        move |a| format!("{}\n", c.wrap(&sprintln(a)))
    }

    // -----------------------------------------------------------------------
    // Sequence construction
    // -----------------------------------------------------------------------

    /// Port of the unexported `(*Color).sequence`.
    ///
    /// > sequence returns a formatted SGR sequence to be plugged into a `"\x1b[...m"`
    /// > an example output might be: `"1;36"` -> bold cyan
    fn sequence(&self) -> String {
        let format: Vec<String> = self.params.iter().map(|v| v.code().to_string()).collect();
        format.join(";")
    }

    /// Port of the unexported `(*Color).wrap`.
    ///
    /// > wrap wraps the s string with the colors attributes. The string is ready to
    /// > be printed.
    fn wrap(&self, s: &str) -> String {
        if self.is_no_color_set() {
            return s.to_string();
        }

        format!("{}{}{}", self.format(), s, self.unformat())
    }

    /// Port of the unexported `(*Color).format`.
    pub(crate) fn format(&self) -> String {
        format!("{}[{}m", ESCAPE, self.sequence())
    }

    /// Port of the unexported `(*Color).unformat`.
    ///
    /// For each element in the sequence the specific reset escape is used, or
    /// the generic one when the attribute has no dedicated reset code.
    pub(crate) fn unformat(&self) -> String {
        let format: Vec<String> = self
            .params
            .iter()
            .map(|v| match map_reset_attributes(*v) {
                Some(ra) => ra.code().to_string(),
                None => RESET.code().to_string(),
            })
            .collect();

        format!("{}[{}m", ESCAPE, format.join(";"))
    }

    /// Port of `(*Color).DisableColor`.
    ///
    /// > DisableColor disables the color output. Useful to not change any existing
    /// > code and still being able to output. Can be used for flags like
    /// > `--no-color`. To enable back use `EnableColor()` method.
    pub fn disable_color(&mut self) {
        self.no_color = Some(true);
    }

    /// Port of `(*Color).EnableColor`.
    ///
    /// > EnableColor enables the color output. Use it in conjunction with
    /// > `DisableColor()`. Otherwise, this method has no side effects.
    pub fn enable_color(&mut self) {
        self.no_color = Some(false);
    }

    /// Port of the unexported `(*Color).isNoColorSet`.
    fn is_no_color_set(&self) -> bool {
        // check first if we have user set action
        if let Some(v) = self.no_color {
            return v;
        }

        // if not return the global option, which is disabled by default
        no_color()
    }

    /// Port of `(*Color).Equals`.
    ///
    /// > Equals returns a boolean value indicating whether two colors are equal.
    ///
    /// Attribute *multisets* are compared: order does not matter but duplicate
    /// counts do.
    pub fn equals(&self, c2: &Color) -> bool {
        if self.params.len() != c2.params.len() {
            return false;
        }

        let mut counts: HashMap<Attribute, i32> = HashMap::with_capacity(self.params.len());
        for attr in &self.params {
            *counts.entry(*attr).or_insert(0) += 1;
        }

        for attr in &c2.params {
            let entry = counts.entry(*attr).or_insert(0);
            if *entry == 0 {
                return false;
            }
            *entry -= 1;
        }

        true
    }
}

impl PartialEq for Color {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

/// Port of `(*Color).Equals` including its `nil` receiver handling.
///
/// ```go
/// if c == nil && c2 == nil { return true }
/// if c == nil || c2 == nil { return false }
/// ```
pub fn equals(c: Option<&Color>, c2: Option<&Color>) -> bool {
    match (c, c2) {
        (None, None) => true,
        (Some(a), Some(b)) => a.equals(b),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Package level functions
// ---------------------------------------------------------------------------

/// Port of `New(value ...Attribute) *Color`.
///
/// > New returns a newly created color object.
pub fn new(value: &[Attribute]) -> Color {
    Color::new(value)
}

/// Port of `RGB(r, g, b int) *Color`.
///
/// > RGB returns a new foreground color in 24-bit RGB.
pub fn rgb(r: i32, g: i32, b: i32) -> Color {
    Color::new(&[
        FOREGROUND,
        Attribute(2),
        Attribute(r),
        Attribute(g),
        Attribute(b),
    ])
}

/// Port of `BgRGB(r, g, b int) *Color`.
///
/// > BgRGB returns a new background color in 24-bit RGB.
pub fn bg_rgb(r: i32, g: i32, b: i32) -> Color {
    Color::new(&[
        BACKGROUND,
        Attribute(2),
        Attribute(r),
        Attribute(g),
        Attribute(b),
    ])
}

/// Port of `Set(p ...Attribute) *Color`.
///
/// > Set sets the given parameters immediately. It will change the color of
/// > output with the given SGR parameters until `Unset()` is called.
pub fn set(p: &[Attribute]) -> Color {
    let c = Color::new(p);
    c.set();
    c
}

/// Port of `Unset()`.
///
/// > Unset resets all escape attributes and clears the output. Usually should
/// > be called after `Set()`.
pub fn unset() {
    if no_color() {
        return;
    }

    let mut w = output();
    let _ = fprint_str(&mut w, &format!("{}[{}m", ESCAPE, RESET));
}

/// Port of the unexported `sprintln`.
///
/// > sprintln is a helper function to format a string with `fmt.Sprintln` and
/// > trim the trailing newline.
fn sprintln(a: &[Value]) -> String {
    let s = gofmt::sprintln(a);
    // strings.TrimSuffix(s, "\n")
    match s.strip_suffix('\n') {
        Some(t) => t.to_string(),
        None => s,
    }
}

/// Port of the unexported `getCachedColor`.
fn get_cached_color(p: Attribute) -> Arc<Color> {
    let cache = COLORS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());

    guard
        .entry(p)
        .or_insert_with(|| Arc::new(Color::new(&[p])))
        .clone()
}

/// Port of the unexported `colorPrint`.
fn color_print(format: &str, p: Attribute, a: &[Value]) {
    let c = get_cached_color(p);

    let format = if !format.ends_with('\n') {
        format!("{}\n", format)
    } else {
        format.to_string()
    };

    if a.is_empty() {
        let _ = c.print(&[Value::Str(format)]);
    } else {
        let _ = c.printf(&format, a);
    }
}

/// Port of the unexported `colorString`.
fn color_string(format: &str, p: Attribute, a: &[Value]) -> String {
    let c = get_cached_color(p);

    if a.is_empty() {
        return c.sprint_func()(&[Value::Str(format.to_string())]);
    }

    c.sprintf_func()(format, a)
}

/// Generates the `Black`/`BlackString`-style convenience helpers.
macro_rules! helpers {
    ($(
        $(#[$pm:meta])* $print:ident,
        $(#[$sm:meta])* $string:ident,
        $attr:expr
    );* $(;)?) => {
        $(
            $(#[$pm])*
            pub fn $print(format: &str, a: &[Value]) { color_print(format, $attr, a) }

            $(#[$sm])*
            pub fn $string(format: &str, a: &[Value]) -> String { color_string(format, $attr, a) }
        )*
    };
}

use crate::attribute::{
    FG_BLACK, FG_BLUE, FG_CYAN, FG_GREEN, FG_HI_BLACK, FG_HI_BLUE, FG_HI_CYAN, FG_HI_GREEN,
    FG_HI_MAGENTA, FG_HI_RED, FG_HI_WHITE, FG_HI_YELLOW, FG_MAGENTA, FG_RED, FG_WHITE, FG_YELLOW,
};

helpers! {
    /// > Black is a convenient helper function to print with black foreground. A
    /// > newline is appended to format by default.
    black,
    /// > BlackString is a convenient helper function to return a string with black
    /// > foreground.
    black_string, FG_BLACK;

    /// > Red is a convenient helper function to print with red foreground. A
    /// > newline is appended to format by default.
    red,
    /// > RedString is a convenient helper function to return a string with red
    /// > foreground.
    red_string, FG_RED;

    /// > Green is a convenient helper function to print with green foreground. A
    /// > newline is appended to format by default.
    green,
    /// > GreenString is a convenient helper function to return a string with green
    /// > foreground.
    green_string, FG_GREEN;

    /// > Yellow is a convenient helper function to print with yellow foreground.
    /// > A newline is appended to format by default.
    yellow,
    /// > YellowString is a convenient helper function to return a string with yellow
    /// > foreground.
    yellow_string, FG_YELLOW;

    /// > Blue is a convenient helper function to print with blue foreground. A
    /// > newline is appended to format by default.
    blue,
    /// > BlueString is a convenient helper function to return a string with blue
    /// > foreground.
    blue_string, FG_BLUE;

    /// > Magenta is a convenient helper function to print with magenta foreground.
    /// > A newline is appended to format by default.
    magenta,
    /// > MagentaString is a convenient helper function to return a string with magenta
    /// > foreground.
    magenta_string, FG_MAGENTA;

    /// > Cyan is a convenient helper function to print with cyan foreground. A
    /// > newline is appended to format by default.
    cyan,
    /// > CyanString is a convenient helper function to return a string with cyan
    /// > foreground.
    cyan_string, FG_CYAN;

    /// > White is a convenient helper function to print with white foreground. A
    /// > newline is appended to format by default.
    white,
    /// > WhiteString is a convenient helper function to return a string with white
    /// > foreground.
    white_string, FG_WHITE;

    /// > HiBlack is a convenient helper function to print with hi-intensity black
    /// > foreground. A newline is appended to format by default.
    hi_black,
    /// > HiBlackString is a convenient helper function to return a string with
    /// > hi-intensity black foreground.
    hi_black_string, FG_HI_BLACK;

    /// > HiRed is a convenient helper function to print with hi-intensity red
    /// > foreground. A newline is appended to format by default.
    hi_red,
    /// > HiRedString is a convenient helper function to return a string with
    /// > hi-intensity red foreground.
    hi_red_string, FG_HI_RED;

    /// > HiGreen is a convenient helper function to print with hi-intensity green
    /// > foreground. A newline is appended to format by default.
    hi_green,
    /// > HiGreenString is a convenient helper function to return a string with
    /// > hi-intensity green foreground.
    hi_green_string, FG_HI_GREEN;

    /// > HiYellow is a convenient helper function to print with hi-intensity yellow
    /// > foreground. A newline is appended to format by default.
    hi_yellow,
    /// > HiYellowString is a convenient helper function to return a string with
    /// > hi-intensity yellow foreground.
    hi_yellow_string, FG_HI_YELLOW;

    /// > HiBlue is a convenient helper function to print with hi-intensity blue
    /// > foreground. A newline is appended to format by default.
    hi_blue,
    /// > HiBlueString is a convenient helper function to return a string with
    /// > hi-intensity blue foreground.
    hi_blue_string, FG_HI_BLUE;

    /// > HiMagenta is a convenient helper function to print with hi-intensity magenta
    /// > foreground. A newline is appended to format by default.
    hi_magenta,
    /// > HiMagentaString is a convenient helper function to return a string with
    /// > hi-intensity magenta foreground.
    hi_magenta_string, FG_HI_MAGENTA;

    /// > HiCyan is a convenient helper function to print with hi-intensity cyan
    /// > foreground. A newline is appended to format by default.
    hi_cyan,
    /// > HiCyanString is a convenient helper function to return a string with
    /// > hi-intensity cyan foreground.
    hi_cyan_string, FG_HI_CYAN;

    /// > HiWhite is a convenient helper function to print with hi-intensity white
    /// > foreground. A newline is appended to format by default.
    hi_white,
    /// > HiWhiteString is a convenient helper function to return a string with
    /// > hi-intensity white foreground.
    hi_white_string, FG_HI_WHITE;
}
