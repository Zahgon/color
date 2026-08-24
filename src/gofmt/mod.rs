//! A port of the subset of Go's `fmt` package that `fatih/color` relies on.
//!
//! `color` builds every one of its outputs on top of `fmt.Sprint`,
//! `fmt.Sprintln` and `fmt.Sprintf`, so byte-for-byte compatible output
//! requires byte-for-byte compatible formatting — including the quirks:
//!
//!   * `Sprint` inserts a space between operands *only* when neither adjacent
//!     operand has `reflect.Kind == String`.
//!   * `Sprintln` always inserts spaces and appends a newline.
//!   * Malformed directives produce Go's diagnostic strings such as
//!     `%!d(string=foo)`, `%!s(MISSING)`, `%!(NOVERB)` and
//!     `%!(EXTRA int=3)` instead of panicking.
//!
//! The implementation mirrors `fmt/print.go` and `fmt/format.go` closely
//! enough that the control flow can be compared side by side.

mod float;
mod printable;

use crate::value::Value;
use printable::is_go_printable;

const PERCENT_BANG: &str = "%!";
const MISSING_STRING: &str = "(MISSING)";
const EXTRA_STRING: &str = "%!(EXTRA ";
const BAD_INDEX_STRING: &str = "%!(BADINDEX)";
const NO_VERB_STRING: &str = "%!(NOVERB)";
const BAD_WIDTH_STRING: &str = "%!(BADWIDTH)";
const BAD_PREC_STRING: &str = "%!(BADPREC)";
const NIL_ANGLE_STRING: &str = "<nil>";
/// `fmt`'s rendering of a nil element inside a `[]interface{}` under `%#v`:
/// `nilParenString` appended to the interface type's name.
const NIL_INTERFACE_STRING: &str = "interface {}(nil)";
/// The concrete type behind `errors.New`, which is what `printValue` reflects
/// over once `handleMethods` declines a verb.
const ERROR_STRING_TYPE: &str = "*errors.errorString";

const LDIGITS: &[u8] = b"0123456789abcdefx";
const UDIGITS: &[u8] = b"0123456789ABCDEFX";

/// Go's `parsenum` overflow guard (`fmt/print.go`).
const MAX_PARSE_NUM: i32 = 1_000_000;

// ---------------------------------------------------------------------------
// Formatting flags — port of `fmt.fmt`
// ---------------------------------------------------------------------------

#[derive(Default, Clone, Copy)]
struct Flags {
    wid: i32,
    wid_present: bool,
    prec: i32,
    prec_present: bool,

    minus: bool,
    plus: bool,
    sharp: bool,
    space: bool,
    zero: bool,

    /// `%+v` — set while the `v` verb is being processed.
    plus_v: bool,
    /// `%#v` — set while the `v` verb is being processed.
    sharp_v: bool,
}

impl Flags {
    fn clear(&mut self) {
        *self = Flags::default();
    }

    /// Port of `fmt.writePadding`.
    fn write_padding(&self, buf: &mut String, n: i32) {
        if n <= 0 {
            return;
        }
        let pad = if self.zero { '0' } else { ' ' };
        for _ in 0..n {
            buf.push(pad);
        }
    }

    /// Port of `fmt.padString`: pads `s` to the requested width, counting
    /// *runes* rather than bytes exactly as Go does.
    fn pad_string(&self, buf: &mut String, s: &str) {
        if !self.wid_present || self.wid == 0 {
            buf.push_str(s);
            return;
        }
        let width = self.wid - s.chars().count() as i32;
        if !self.minus {
            self.write_padding(buf, width);
            buf.push_str(s);
        } else {
            buf.push_str(s);
            self.write_padding(buf, width);
        }
    }

    /// Port of `fmt.truncateString`.
    fn truncate_string<'a>(&self, s: &'a str) -> std::borrow::Cow<'a, str> {
        if !self.prec_present {
            return std::borrow::Cow::Borrowed(s);
        }
        let n = self.prec as usize;
        match s.char_indices().nth(n) {
            Some((idx, _)) => std::borrow::Cow::Borrowed(&s[..idx]),
            None => std::borrow::Cow::Borrowed(s),
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Port of `fmt.Sprint`.
///
/// Formats using the default formats for its operands and returns the
/// resulting string. Spaces are added between operands when neither is a
/// string.
pub fn sprint(a: &[Value]) -> String {
    let mut buf = String::new();
    let mut prev_string = false;
    for (arg_num, arg) in a.iter().enumerate() {
        let is_string = !arg.is_nil() && arg.is_string_kind();
        if arg_num > 0 && !is_string && !prev_string {
            buf.push(' ');
        }
        print_arg(&mut buf, arg, 'v', &mut Flags::default());
        prev_string = is_string;
    }
    buf
}

/// Port of `fmt.Sprintln`.
///
/// Spaces are always added between operands and a newline is appended.
pub fn sprintln(a: &[Value]) -> String {
    let mut buf = String::new();
    for (arg_num, arg) in a.iter().enumerate() {
        if arg_num > 0 {
            buf.push(' ');
        }
        print_arg(&mut buf, arg, 'v', &mut Flags::default());
    }
    buf.push('\n');
    buf
}

/// Port of `fmt.Sprintf`, including all of Go's error diagnostics.
pub fn sprintf(format: &str, a: &[Value]) -> String {
    let mut buf = String::new();
    let f = format.as_bytes();
    let end = f.len();

    let mut arg_num: usize = 0;
    let mut reordered = false;
    let mut i = 0usize;

    'format_loop: while i < end {
        let mut good_arg_num = true;

        // Copy the literal run preceding the next '%'.
        let lasti = i;
        while i < end && f[i] != b'%' {
            i += 1;
        }
        if i > lasti {
            buf.push_str(&format[lasti..i]);
        }
        if i >= end {
            break;
        }
        i += 1; // consume '%'

        let mut fl = Flags::default();
        fl.clear();

        // --- flags -------------------------------------------------------
        while i < end {
            match f[i] {
                b'#' => fl.sharp = true,
                b'0' => fl.zero = !fl.minus,
                b'+' => fl.plus = true,
                b'-' => {
                    fl.minus = true;
                    fl.zero = false;
                }
                b' ' => fl.space = true,
                _ => break,
            }
            i += 1;
        }

        // --- explicit argument index, width ------------------------------
        let mut after_index;
        (arg_num, i, after_index) =
            argument_number(arg_num, f, i, a.len(), &mut reordered, &mut good_arg_num);

        if i < end && f[i] == b'*' {
            i += 1;
            match int_from_arg(a, arg_num) {
                Some(v) => {
                    fl.wid = v;
                    fl.wid_present = true;
                    arg_num += 1;
                }
                None => {
                    fl.wid_present = false;
                    if arg_num < a.len() {
                        arg_num += 1;
                    }
                    buf.push_str(BAD_WIDTH_STRING);
                }
            }
            if fl.wid < 0 {
                fl.wid = -fl.wid;
                fl.minus = true;
                fl.zero = false;
            }
            after_index = false;
        } else {
            let (num, isnum, newi) = parsenum(f, i, end);
            fl.wid = num;
            fl.wid_present = isnum;
            i = newi;
            if after_index && fl.wid_present {
                good_arg_num = false;
            }
        }

        // --- precision ---------------------------------------------------
        if i + 1 < end && f[i] == b'.' {
            i += 1;
            if after_index {
                good_arg_num = false;
            }
            (arg_num, i, after_index) =
                argument_number(arg_num, f, i, a.len(), &mut reordered, &mut good_arg_num);

            if i < end && f[i] == b'*' {
                i += 1;
                match int_from_arg(a, arg_num) {
                    Some(v) => {
                        fl.prec = v;
                        fl.prec_present = true;
                        arg_num += 1;
                    }
                    None => {
                        fl.prec_present = false;
                        if arg_num < a.len() {
                            arg_num += 1;
                        }
                        buf.push_str(BAD_PREC_STRING);
                    }
                }
                if fl.prec < 0 {
                    fl.prec = 0;
                    fl.prec_present = false;
                    buf.push_str(BAD_PREC_STRING);
                }
                after_index = false;
            } else {
                let (num, isnum, newi) = parsenum(f, i, end);
                fl.prec = num;
                fl.prec_present = isnum;
                i = newi;
                if !fl.prec_present {
                    fl.prec = 0;
                    fl.prec_present = true;
                }
            }
        }

        if !after_index {
            (arg_num, i, _) =
                argument_number(arg_num, f, i, a.len(), &mut reordered, &mut good_arg_num);
        }

        if i >= end {
            buf.push_str(NO_VERB_STRING);
            break;
        }

        // --- verb --------------------------------------------------------
        let verb = decode_rune(format, i);
        i += verb.len_utf8();

        if verb == '%' {
            buf.push('%');
            continue 'format_loop;
        }
        if !good_arg_num {
            buf.push_str(PERCENT_BANG);
            buf.push(verb);
            buf.push_str(&BAD_INDEX_STRING[2..]);
            continue 'format_loop;
        }
        if arg_num >= a.len() {
            buf.push_str(PERCENT_BANG);
            buf.push(verb);
            buf.push_str(MISSING_STRING);
            continue 'format_loop;
        }
        if verb == 'v' {
            fl.sharp_v = fl.sharp;
            fl.sharp = false;
            fl.plus_v = fl.plus;
            fl.plus = false;
        }
        print_arg(&mut buf, &a[arg_num], verb, &mut fl);
        arg_num += 1;
    }

    // --- trailing extra arguments -----------------------------------------
    if !reordered && arg_num < a.len() {
        buf.push_str(EXTRA_STRING);
        for (k, arg) in a[arg_num..].iter().enumerate() {
            if k > 0 {
                buf.push_str(", ");
            }
            if arg.is_nil() {
                buf.push_str(NIL_ANGLE_STRING);
            } else {
                buf.push_str(arg.go_type_name());
                buf.push('=');
                print_arg(&mut buf, arg, 'v', &mut Flags::default());
            }
        }
        buf.push(')');
    }

    buf
}

// ---------------------------------------------------------------------------
// Format-string scanning helpers
// ---------------------------------------------------------------------------

/// Port of `fmt.parsenum`.
fn parsenum(s: &[u8], start: usize, end: usize) -> (i32, bool, usize) {
    if start >= end || !s[start].is_ascii_digit() {
        return (0, false, start);
    }
    let mut num: i32 = 0;
    let mut isnum = false;
    let mut newi = start;
    while newi < end && s[newi].is_ascii_digit() {
        if num > MAX_PARSE_NUM {
            // Overflow; crazy long number most likely.
            return (0, false, end);
        }
        num = num * 10 + (s[newi] - b'0') as i32;
        isnum = true;
        newi += 1;
    }
    (num, isnum, newi)
}

/// Port of `pp.argNumber` — parses an explicit `[n]` argument index.
///
/// Returns `(new_arg_num, new_i, found_index)`.
fn argument_number(
    arg_num: usize,
    format: &[u8],
    mut i: usize,
    num_args: usize,
    reordered: &mut bool,
    good_arg_num: &mut bool,
) -> (usize, usize, bool) {
    if format.len() <= i || format[i] != b'[' {
        return (arg_num, i, false);
    }
    *reordered = true;

    // Port of `parseArgNumber`.
    let start = i;
    i += 1;
    let mut j = i;
    while j < format.len() && format[j] != b']' {
        j += 1;
    }
    if j >= format.len() {
        // No closing bracket: Go returns (0, 1, false).
        *good_arg_num = false;
        return (arg_num, start + 1, false);
    }
    let (width, ok, _) = parsenum(format, i, j);
    if !ok || width == 0 {
        *good_arg_num = false;
        return (arg_num, j + 1, false);
    }
    // Arg numbers are one-indexed and skip the format string itself.
    let index = (width - 1) as usize;
    if index < num_args {
        return (index, j + 1, true);
    }
    *good_arg_num = false;
    (arg_num, j + 1, true)
}

/// Port of `fmt.intFromArg`: extracts an `int` for a `*` width/precision.
///
/// Returns `None` for a non-integer argument *and* for integers rejected by
/// `tooLarge`, which is what makes `fmt.Sprintf("%*d", 123456789)` produce
/// `%!(BADWIDTH)` rather than a gigantic pad.
fn int_from_arg(a: &[Value], arg_num: usize) -> Option<i32> {
    // `num, isInt` in Go. A value that does not round-trip through `int`
    // leaves `isInt` false — it does *not* silently fall back to zero.
    let (num, is_int): (i64, bool) = match a.get(arg_num)? {
        // `int64(int(n)) == n` — always true for a 64-bit `int`.
        Value::Int(v) => (*v, true),
        // `int64(n) >= 0 && uint64(int(n)) == n` — rejects anything with the
        // sign bit set, e.g. uint64(math.MaxUint64).
        Value::Uint(v) => {
            if *v <= i64::MAX as u64 {
                (*v as i64, true)
            } else {
                (0, false)
            }
        }
        Value::Uint8(v) => (*v as i64, true),
        Value::Rune(c) => (*c as i64, true),
        // Every other kind hits Go's `default:` branch — "Already 0, false."
        _ => (0, false),
    };

    if !is_int {
        return None;
    }

    // tooLarge: `const max int = 1e6; return x > max || x < -max`
    if num > MAX_PARSE_NUM as i64 || num < -(MAX_PARSE_NUM as i64) {
        return None;
    }
    Some(num as i32)
}

/// Decodes the rune starting at byte offset `i`.
fn decode_rune(s: &str, i: usize) -> char {
    s[i..].chars().next().unwrap_or('\u{fffd}')
}

// ---------------------------------------------------------------------------
// Value printing — port of `pp.printArg` and `fmt.fmtXxx`
// ---------------------------------------------------------------------------

/// Port of `pp.printArg` — the `depth == 0` entry point.
fn print_arg(buf: &mut String, arg: &Value, verb: char, fl: &mut Flags) {
    print_arg_at(buf, arg, verb, fl, 0);
}

/// Port of `pp.printArg` / `pp.printValue`, which are one recursive pair in Go.
///
/// `depth` is Go's `printValue` depth: `0` for a top-level operand and `1` or
/// more for a value reached by reflecting into a composite. Several rules key
/// off it — a nil interface *element* prints `<nil>` whatever the verb, while a
/// nil *operand* takes the `badVerb` path — so it has to be threaded through
/// rather than assumed.
fn print_arg_at(buf: &mut String, arg: &Value, verb: char, fl: &mut Flags, depth: usize) {
    // Port of `pp.printArg`'s `if arg == nil` branch, which runs before the
    // special-cased `%T`/`%p` handling.
    if arg.is_nil() {
        if depth > 0 {
            // `printValue`'s `reflect.Interface` case: an interface holding nil
            // is written literally, with no verb check and no padding.
            buf.push_str(if fl.sharp_v {
                NIL_INTERFACE_STRING
            } else {
                NIL_ANGLE_STRING
            });
            return;
        }
        match verb {
            'T' | 'v' => fl.pad_string(buf, NIL_ANGLE_STRING),
            _ => bad_verb(buf, arg, verb, fl),
        }
        return;
    }

    // > %T (the value's type) and %p (its address) are special; we always do
    // > them first.
    match verb {
        'T' => {
            // `p.fmt.fmtS(...)`: truncate to the precision, then pad.
            let name = arg.go_type_name();
            let t = fl.truncate_string(name);
            fl.pad_string(buf, &t);
            return;
        }
        'p' => {
            // Go prints an address for pointer-shaped kinds. `color` never
            // prints pointers, and an address would be meaningless in Rust, so
            // every value reports the same diagnostic Go emits for the
            // non-pointer kinds: `%!p(type=value)`.
            bad_verb(buf, arg, verb, fl);
            return;
        }
        _ => {}
    }

    match arg {
        Value::Nil => unreachable!("handled above"),
        Value::Bool(v) => fmt_bool(buf, *v, verb, fl, arg),
        Value::Int(v) => fmt_integer(buf, *v as u64, true, verb, fl, arg),
        Value::Rune(c) => fmt_integer(buf, *c as u32 as u64, true, verb, fl, arg),
        Value::Uint(v) => fmt_integer(buf, *v, false, verb, fl, arg),
        Value::Uint8(v) => fmt_integer(buf, *v as u64, false, verb, fl, arg),
        Value::Float(v) => fmt_float(buf, *v, verb, fl, arg),
        Value::Str(s) => fmt_string(buf, s, verb, fl, arg),
        Value::Error(s) => fmt_error(buf, s, verb, fl, depth),
        Value::Bytes(b) => fmt_bytes(buf, b, verb, fl, depth),
        Value::Slice(items) => fmt_slice(buf, items, verb, fl, depth),
    }
}

/// Port of the `error` path through `handleMethods` and `printValue`.
///
/// `errors.New` returns a `*errors.errorString` — a *pointer* to a one-field
/// struct — and Go's two reflection cases for a pointer differ by depth:
///
/// * at depth 0 `printValue` dereferences it, giving `&{…}` with the verb
///   applied to the `s` field: `%d` → `&{%!d(string=boom)}`;
/// * deeper in (inside a `[]interface{}`) it falls through to `fmtPointer`,
///   which reports the whole pointer as bad: `%c` →
///   `%!c(*errors.errorString=&{boom})`.
///
/// # Divergence
///
/// `fmtPointer` renders `%d`/`%b`/`%o`/`%p`, and `%#v`, as the pointer's
/// *address*. Addresses are neither meaningful nor stable in this port (`%p`
/// already reports `badVerb` for the same reason), so those verbs take the
/// diagnostic branch here too. Every other verb is byte-identical to Go.
fn fmt_error(buf: &mut String, s: &str, verb: char, fl: &Flags, depth: usize) {
    // `handleMethods`: `error` is rendered through `Error()` for the string-ish
    // verbs, at any depth.
    if matches!(verb, 'v' | 's' | 'q' | 'x' | 'X') && !fl.sharp_v {
        fmt_string(buf, s, verb, fl, &Value::Error(s.to_string()));
        return;
    }

    if depth > 0 {
        // `fmtPointer`'s `default:` — `badVerb` on the pointer itself. Go
        // re-enters `printArg` with `erroring` set, so the value is rendered
        // with `%v` at depth 0: `&` plus the dereferenced struct.
        buf.push_str(PERCENT_BANG);
        buf.push(verb);
        buf.push('(');
        buf.push_str(ERROR_STRING_TYPE);
        buf.push_str("=&{");
        // printValue reaches the `s` field with `%v` while the directive's
        // flags are still installed, so width/precision apply to it.
        fl.pad_string(buf, &fl.truncate_string(s));
        buf.push_str("})");
        return;
    }

    // Depth 0: `printValue` dereferences the pointer.
    //   %d  -> &{%!d(string=boom)}
    //   %#v -> &errors.errorString{s:"boom"}
    if fl.sharp_v {
        // printValue recurses into the field, so width/precision apply to the
        // quoted field value.
        buf.push_str("&errors.errorString{s:");
        fmt_q(buf, s, fl);
        buf.push('}');
    } else {
        // Go reflects into the struct, so the diagnostic names the field's type
        // (`string`), not the error's.
        buf.push_str("&{");
        fmt_string(buf, s, verb, fl, &Value::Str(s.to_string()));
        buf.push('}');
    }
}

/// Port of `pp.badVerb`: `%!verb(type=value)` or `%!verb(<nil>)`.
///
/// Go re-enters `printArg` while `p.fmt` still holds the directive's flags, so
/// width and precision apply to the embedded value too (`%.0d` on a string
/// yields `%!d(string=)`). The active flags are therefore threaded through.
fn bad_verb(buf: &mut String, arg: &Value, verb: char, fl: &Flags) {
    buf.push_str(PERCENT_BANG);
    buf.push(verb);
    buf.push('(');
    if arg.is_nil() {
        buf.push_str(NIL_ANGLE_STRING);
    } else {
        buf.push_str(arg.go_type_name());
        buf.push('=');
        print_arg(buf, arg, 'v', &mut { *fl });
    }
    buf.push(')');
}

/// Port of `fmt.fmtBoolean`.
fn fmt_bool(buf: &mut String, v: bool, verb: char, fl: &Flags, arg: &Value) {
    match verb {
        't' | 'v' => fl.pad_string(buf, if v { "true" } else { "false" }),
        _ => bad_verb(buf, arg, verb, fl),
    }
}

/// Port of `pp.fmtString` / `fmt.fmtS`, `fmt.fmtQ`, `fmt.fmtSbx`.
fn fmt_string(buf: &mut String, s: &str, verb: char, fl: &Flags, arg: &Value) {
    match verb {
        'v' => {
            if fl.sharp_v {
                // `p.fmt.fmtQ(v)` — truncate first, then quote.
                fmt_q(buf, s, fl);
            } else {
                let t = fl.truncate_string(s);
                fl.pad_string(buf, &t);
            }
        }
        's' => {
            let t = fl.truncate_string(s);
            fl.pad_string(buf, &t);
        }
        'q' => fmt_q(buf, s, fl),
        'x' => fmt_sbx(buf, s.as_bytes(), fl, LDIGITS),
        'X' => fmt_sbx(buf, s.as_bytes(), fl, UDIGITS),
        _ => bad_verb(buf, arg, verb, fl),
    }
}

/// Port of `fmt.fmtQ`: truncate to the precision, then quote, honouring the
/// `#` (raw string) and `+` (ASCII-only) flags.
fn fmt_q(buf: &mut String, s: &str, fl: &Flags) {
    let t = fl.truncate_string(s);
    let quoted = if fl.sharp && can_backquote(&t) {
        format!("`{}`", t)
    } else if fl.plus {
        go_quote_to_ascii(&t)
    } else {
        go_quote(&t)
    };
    fl.pad_string(buf, &quoted);
}

/// Port of `pp.fmtBytes`.
fn fmt_bytes(buf: &mut String, b: &[u8], verb: char, fl: &mut Flags, depth: usize) {
    match verb {
        'v' | 'd' => {
            if fl.sharp_v {
                // `pp.fmtBytes` is handed the type name by its caller: the
                // `case []byte:` in `printArg` passes the literal "[]byte",
                // while `printValue` passes `reflect.Type.String()`, which
                // spells the same type "[]uint8".
                //   %#v of a []byte operand        -> []byte{0x61}
                //   %#v of a []byte inside a slice -> []uint8{0x61}
                buf.push_str(if depth == 0 { "[]byte{" } else { "[]uint8{" });
                for (i, c) in b.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    let mut elem_flags = *fl;
                    print_arg_at(buf, &Value::Uint8(*c), 'v', &mut elem_flags, depth + 1);
                }
                buf.push('}');
            } else {
                buf.push('[');
                for (i, c) in b.iter().enumerate() {
                    if i > 0 {
                        buf.push(' ');
                    }
                    fmt_integer(buf, *c as u64, false, verb, fl, &Value::Uint8(*c));
                }
                buf.push(']');
            }
        }
        's' => {
            let s = String::from_utf8_lossy(b).into_owned();
            let t = fl.truncate_string(&s);
            fl.pad_string(buf, &t);
        }
        'q' => {
            let s = String::from_utf8_lossy(b).into_owned();
            fmt_q(buf, &s, fl);
        }
        'x' => fmt_sbx(buf, b, fl, LDIGITS),
        'X' => fmt_sbx(buf, b, fl, UDIGITS),
        // Every other verb falls through to `printValue`, which applies the
        // verb to each `uint8` element.
        _ => {
            buf.push('[');
            for (i, c) in b.iter().enumerate() {
                if i > 0 {
                    buf.push(' ');
                }
                let mut elem_flags = *fl;
                print_arg_at(buf, &Value::Uint8(*c), verb, &mut elem_flags, depth + 1);
            }
            buf.push(']');
        }
    }
}

/// Port of `pp.printValue` for `[]interface{}`: the verb is applied to every
/// element and the results are wrapped in `[...]` (or in Go-syntax braces for
/// `%#v`).
fn fmt_slice(buf: &mut String, items: &[Value], verb: char, fl: &mut Flags, depth: usize) {
    if fl.sharp_v {
        buf.push_str("[]interface {}{");
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                buf.push_str(", ");
            }
            let mut elem_flags = *fl;
            print_arg_at(buf, item, verb, &mut elem_flags, depth + 1);
        }
        buf.push('}');
        return;
    }

    buf.push('[');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            buf.push(' ');
        }
        let mut elem_flags = *fl;
        print_arg_at(buf, item, verb, &mut elem_flags, depth + 1);
    }
    buf.push(']');
}

/// Port of `fmt.fmtInteger`.
fn fmt_integer(buf: &mut String, u: u64, is_signed: bool, verb: char, fl: &Flags, arg: &Value) {
    // `pp.fmt0x64` temporarily forces the `#` flag on; everything else uses the
    // caller's flags untouched.
    let mut sharp_override = false;
    let (base, digits, do_prefix) = match verb {
        'v' => {
            // Port of `pp.fmt0x64`: `%#v` renders unsigned integers in
            // Go-syntax hexadecimal, honouring width/precision as usual.
            if fl.sharp_v && !is_signed {
                sharp_override = true;
                (16u64, LDIGITS, true)
            } else {
                (10u64, LDIGITS, false)
            }
        }
        'd' => (10, LDIGITS, false),
        'b' => (2, LDIGITS, true),
        'o' | 'O' => (8, LDIGITS, true),
        'x' => (16, LDIGITS, true),
        'X' => (16, UDIGITS, true),
        'c' => {
            fmt_c(buf, u, fl);
            return;
        }
        'q' => {
            fmt_qc(buf, u, fl);
            return;
        }
        'U' => {
            fmt_unicode(buf, u, fl);
            return;
        }
        _ => {
            bad_verb(buf, arg, verb, fl);
            return;
        }
    };

    let negative = is_signed && (u as i64) < 0;
    let mut u = if negative {
        (u as i64).unsigned_abs()
    } else {
        u
    };

    // Two ways to ask for extra leading zero digits: %.3d or %03d.
    let mut prec: i32 = 0;
    if fl.prec_present {
        prec = fl.prec;
        if prec == 0 && u == 0 {
            // Precision of 0 and value of 0 means "print nothing" but padding
            // still applies (with spaces, never zeros).
            let mut f2 = *fl;
            f2.zero = false;
            f2.write_padding(buf, f2.wid);
            return;
        }
    } else if fl.zero && !fl.minus && fl.wid_present {
        prec = fl.wid;
        if negative || fl.plus || fl.space {
            prec -= 1;
        }
    }

    // Generate the digits, least significant first.
    let mut tmp: Vec<u8> = Vec::with_capacity(24);
    if u == 0 {
        tmp.push(b'0');
    }
    while u > 0 {
        tmp.push(digits[(u % base) as usize]);
        u /= base;
    }
    while (tmp.len() as i32) < prec {
        tmp.push(b'0');
    }

    // Prefixes (`%#b`, `%#o`, `%#x`, `%O`).
    if (fl.sharp || sharp_override) && do_prefix {
        match base {
            2 => {
                tmp.push(b'b');
                tmp.push(b'0');
            }
            8 => {
                if tmp.last() != Some(&b'0') {
                    tmp.push(b'0');
                }
            }
            16 => {
                tmp.push(digits[16]);
                tmp.push(b'0');
            }
            _ => {}
        }
    }
    if verb == 'O' {
        tmp.push(b'o');
        tmp.push(b'0');
    }

    if negative {
        tmp.push(b'-');
    } else if fl.plus {
        tmp.push(b'+');
    } else if fl.space {
        tmp.push(b' ');
    }

    tmp.reverse();
    let s = String::from_utf8_lossy(&tmp).into_owned();

    // Left padding with zeros has already been handled like precision, so the
    // outer padding always uses spaces.
    let mut f2 = *fl;
    f2.zero = false;
    f2.pad_string(buf, &s);
}

/// Go's `utf8.MaxRune`.
const MAX_RUNE: u64 = 0x0010_FFFF;

/// Decodes `c` the way Go's `rune(c)` + `utf8.EncodeRune` pair does: anything
/// above `utf8.MaxRune` (and any surrogate) becomes `utf8.RuneError`.
fn rune_or_error(c: u64) -> char {
    if c > MAX_RUNE {
        return '\u{fffd}';
    }
    char::from_u32(c as u32).unwrap_or('\u{fffd}')
}

/// Port of `fmt.fmtC`.
fn fmt_c(buf: &mut String, u: u64, fl: &Flags) {
    fl.pad_string(buf, &rune_or_error(u).to_string());
}

/// Port of `fmt.fmtQc`.
///
/// Note that this never produces a bad-verb diagnostic: out-of-range values are
/// coerced to `utf8.RuneError` first.
fn fmt_qc(buf: &mut String, u: u64, fl: &Flags) {
    let r = rune_or_error(u);
    let quoted = if fl.plus {
        quote_with(&r.to_string(), '\'', true)
    } else {
        go_quote_rune(r)
    };
    fl.pad_string(buf, &quoted);
}

/// Port of `fmt.fmtUnicode`: `U+0041` (and `U+0041 'A'` with `#`).
fn fmt_unicode(buf: &mut String, u: u64, fl: &Flags) {
    // > prec := 4
    // > if f.precPresent && f.prec > 4 { prec = f.prec }
    let mut prec = 4usize;
    if fl.prec_present && fl.prec > 4 {
        prec = fl.prec as usize;
    }

    // Zero-padded by hand rather than through `{:0>width$}`: `prec` comes from
    // the format string and can reach `parsenum`'s 1e6 ceiling, while Rust's
    // formatting width is a `u16` and panics above 65535.
    let hex = format!("{:X}", u);
    let mut s = String::with_capacity(2 + prec.max(hex.len()));
    s.push_str("U+");
    for _ in hex.len()..prec {
        s.push('0');
    }
    s.push_str(&hex);
    if fl.sharp && u <= MAX_RUNE {
        if let Some(c) = char::from_u32(u as u32) {
            if is_go_printable(c) {
                // > buf = append(buf, ' ', '\'')
                // > buf = utf8.AppendRune(buf, rune(u))
                // > buf = append(buf, '\'')
                //
                // The rune is appended verbatim — `fmtUnicode` does not run it
                // through `strconv.QuoteRune`, so `%#U` of U+0027 is `'''`.
                s.push(' ');
                s.push('\'');
                s.push(c);
                s.push('\'');
            }
        }
    }
    let mut f2 = *fl;
    f2.zero = false;
    // Precision has been consumed as a minimum digit count, so it must not also
    // truncate the result.
    f2.prec_present = false;
    f2.pad_string(buf, &s);
}

/// Port of `fmt.fmtFloat`.
///
/// Follows `fmt/format.go` statement for statement, including its convention of
/// always keeping an explicit sign byte at `num[0]`.
fn fmt_float(buf: &mut String, v: f64, verb: char, fl: &Flags, arg: &Value) {
    // `pp.fmtFloat` normalizes the verb and picks the default precision.
    let (fmt_char, default_prec) = match verb {
        'v' => ('g', -1),
        'b' | 'g' | 'G' | 'x' | 'X' => (verb, -1),
        'f' | 'e' | 'E' => (verb, 6),
        'F' => ('f', 6),
        _ => {
            bad_verb(buf, arg, verb, fl);
            return;
        }
    };

    // > Explicit precision in format specifier overrules default precision.
    let prec = if fl.prec_present {
        fl.prec
    } else {
        default_prec
    };

    // > Format number, reserving space for leading + sign if needed.
    let formatted = float::format_float(v, fmt_char, prec);
    let mut num = if formatted.starts_with('-') || formatted.starts_with('+') {
        formatted
    } else {
        format!("+{}", formatted)
    };

    // > f.space means to add a leading space instead of a "+" sign unless
    // > the sign is explicitly asked for by f.plus.
    if fl.space && num.starts_with('+') && !fl.plus {
        num.replace_range(0..1, " ");
    }

    // > Special handling for infinities and NaN, which don't look like a
    // > number so shouldn't be padded with zeros.
    let second = num.as_bytes()[1];
    if second == b'I' || second == b'N' {
        let mut f2 = *fl;
        f2.zero = false;
        // > Remove sign before NaN if not asked for.
        let s = if second == b'N' && !fl.space && !fl.plus {
            num[1..].to_string()
        } else {
            num.clone()
        };
        f2.pad_string(buf, &s);
        return;
    }

    // > The sharp flag forces printing a decimal point for non-binary formats
    // > and retains trailing zeros, which we may need to restore.
    if fl.sharp && fmt_char != 'b' {
        num = sharpen_float(num, fmt_char, prec);
    }

    // > We want a sign if asked for and if the sign is not positive.
    if fl.plus || !num.starts_with('+') {
        // > If we're zero padding to the left we want the sign before the
        // > leading zeros.
        if fl.zero && !fl.minus && fl.wid_present && fl.wid > num.len() as i32 {
            buf.push(num.as_bytes()[0] as char);
            fl.write_padding(buf, fl.wid - num.len() as i32);
            buf.push_str(&num[1..]);
            return;
        }
        fl.pad_string(buf, &num);
        return;
    }
    // > No sign to show and the number is positive; just print the unsigned number.
    fl.pad_string(buf, &num[1..]);
}

/// Port of the `#`-flag fix-up inside `fmt.fmtFloat`.
///
/// `num` carries its sign byte at index 0. Note that the `digits` switch below
/// deliberately omits `'X'` — Go's does too, which is why `%#X` on a float
/// yields a bare trailing `.` (`0X1.P+00`).
fn sharpen_float(num: String, verb: char, prec: i32) -> String {
    let mut digits: i32 = match verb {
        'v' | 'g' | 'G' | 'x' => {
            // > If no precision is set explicitly use a precision of 6.
            if prec == -1 {
                6
            } else {
                prec
            }
        }
        _ => 0,
    };

    let mut num = num;
    let mut tail = String::new();
    let mut has_decimal_point = false;
    let mut saw_nonzero_digit = false;

    // > Starting from i = 1 to skip sign at num[0].
    let mut i = 1usize;
    while i < num.len() {
        let c = num.as_bytes()[i];
        match c {
            b'.' => has_decimal_point = true,
            b'p' | b'P' => {
                tail = num[i..].to_string();
                num.truncate(i);
            }
            b'e' | b'E' if verb != 'x' && verb != 'X' => {
                tail = num[i..].to_string();
                num.truncate(i);
            }
            _ => {
                // For %x/%X the exponent marker is a hex digit and falls
                // through to here, exactly as Go's `fallthrough` does.
                if c != b'0' {
                    saw_nonzero_digit = true;
                }
                // > Count significant digits after the first non-zero digit.
                if saw_nonzero_digit {
                    digits -= 1;
                }
            }
        }
        i += 1;
    }

    if !has_decimal_point {
        // > Leading digit 0 should contribute once to digits.
        if num.len() == 2 && num.as_bytes()[1] == b'0' {
            digits -= 1;
        }
        num.push('.');
    }
    while digits > 0 {
        num.push('0');
        digits -= 1;
    }
    num.push_str(&tail);
    num
}

/// Port of `fmt.fmtSbx` — the `%x` / `%X` encoding of strings and byte slices.
fn fmt_sbx(buf: &mut String, b: &[u8], fl: &Flags, digits: &[u8]) {
    let mut length = b.len();
    if fl.prec_present && (fl.prec as usize) < length {
        length = fl.prec as usize;
    }

    let mut width = 2 * length as i32;
    if width > 0 {
        if fl.space {
            if fl.sharp {
                width *= 2;
            }
            width += length as i32 - 1;
        } else if fl.sharp {
            width += 2;
        }
    } else {
        if fl.wid_present {
            fl.write_padding(buf, fl.wid);
        }
        return;
    }

    if fl.wid_present && fl.wid > width && !fl.minus {
        fl.write_padding(buf, fl.wid - width);
    }

    if fl.sharp && !fl.space {
        buf.push('0');
        buf.push(digits[16] as char);
    }
    for (i, c) in b[..length].iter().enumerate() {
        if fl.space && i > 0 {
            buf.push(' ');
        }
        if fl.sharp && fl.space {
            buf.push('0');
            buf.push(digits[16] as char);
        }
        buf.push(digits[(c >> 4) as usize] as char);
        buf.push(digits[(c & 0xF) as usize] as char);
    }

    if fl.wid_present && fl.wid > width && fl.minus {
        fl.write_padding(buf, fl.wid - width);
    }
}

// ---------------------------------------------------------------------------
// strconv.Quote
// ---------------------------------------------------------------------------

/// Port of `strconv.CanBackquote`.
///
/// Multi-byte runes are assumed printable and accepted as-is — with one
/// exception Go calls out explicitly: *"BOMs are invisible and should not be
/// quoted."*
fn can_backquote(s: &str) -> bool {
    // NB: the empty string *can* be back-quoted (`strconv.CanBackquote("")`
    // returns true), hence no emptiness check here.
    !s.chars().any(|c| {
        if c.len_utf8() > 1 {
            return c == '\u{feff}';
        }
        (c < ' ' && c != '\t') || c == '`' || c == '\u{7f}'
    })
}

/// Port of `strconv.Quote`.
pub fn go_quote(s: &str) -> String {
    quote_with(s, '"', false)
}

/// Port of `strconv.QuoteToASCII`.
fn go_quote_to_ascii(s: &str) -> String {
    quote_with(s, '"', true)
}

/// Port of `strconv.QuoteRune`.
fn go_quote_rune(c: char) -> String {
    quote_with(&c.to_string(), '\'', false)
}

fn quote_with(s: &str, quote: char, ascii_only: bool) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        append_escaped_rune(&mut out, c, quote, ascii_only);
    }
    out.push(quote);
    out
}

/// Port of `strconv.appendEscapedRune`.
fn append_escaped_rune(out: &mut String, c: char, quote: char, ascii_only: bool) {
    if c == quote || c == '\\' {
        out.push('\\');
        out.push(c);
        return;
    }
    if ascii_only {
        if (0x20..0x7f).contains(&(c as u32)) {
            out.push(c);
            return;
        }
    } else if is_go_printable(c) {
        out.push(c);
        return;
    }

    match c {
        '\u{7}' => out.push_str("\\a"),
        '\u{8}' => out.push_str("\\b"),
        '\u{c}' => out.push_str("\\f"),
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        '\u{b}' => out.push_str("\\v"),
        _ => {
            // strconv.appendEscapedRune: \x is only used for the C0 controls
            // and DEL; every other non-printable rune uses \u / \U.
            let u = c as u32;
            if u < 0x20 || u == 0x7f {
                out.push_str(&format!("\\x{:02x}", u));
            } else if u < 0x10000 {
                out.push_str(&format!("\\u{:04x}", u));
            } else {
                out.push_str(&format!("\\U{:08x}", u));
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::approx_constant)] // literals mirror Go's reference outputs
mod tests {
    use super::*;
    use crate::vals;

    #[test]
    fn sprint_space_rules_match_go() {
        // fmt.Sprint("foo", "bar") == "foobar"
        assert_eq!(sprint(&vals!["foo", "bar"]), "foobar");
        // fmt.Sprint(1, 2) == "1 2"
        assert_eq!(sprint(&vals![1, 2]), "1 2");
        // fmt.Sprint("a", 1) == "a1"   (one side is a string)
        assert_eq!(sprint(&vals!["a", 1]), "a1");
        // fmt.Sprint(1, "a", 2) == "1a2"
        assert_eq!(sprint(&vals![1, "a", 2]), "1a2");
        assert_eq!(sprint(&[]), "");
    }

    #[test]
    fn sprintln_matches_go() {
        assert_eq!(sprintln(&vals!["foo", "bar"]), "foo bar\n");
        assert_eq!(sprintln(&vals![1, 2]), "1 2\n");
        assert_eq!(sprintln(&[]), "\n");
    }

    #[test]
    fn sprintf_basic_verbs_match_go() {
        assert_eq!(sprintf("%s", &vals!["x"]), "x");
        assert_eq!(sprintf("%d", &vals![42]), "42");
        assert_eq!(sprintf("%v", &vals![true]), "true");
        assert_eq!(sprintf("%t", &vals![false]), "false");
        assert_eq!(sprintf("%%", &[]), "%");
        assert_eq!(
            sprintf("%-7s %-7s %5d\n", &vals!["hello", "world", 123]),
            "hello   world     123\n"
        );
        assert_eq!(sprintf("%05d", &vals![42]), "00042");
        assert_eq!(sprintf("%+d", &vals![42]), "+42");
        assert_eq!(sprintf("%x", &vals![255]), "ff");
        assert_eq!(sprintf("%#x", &vals![255]), "0xff");
        assert_eq!(sprintf("%X", &vals![255]), "FF");
        assert_eq!(sprintf("%08.3f", &vals![3.14159]), "0003.142");
        assert_eq!(sprintf("%.2s", &vals!["hello"]), "he");
        assert_eq!(sprintf("%c", &vals!['A']), "A");
        assert_eq!(sprintf("%U", &vals!['A']), "U+0041");
    }

    #[test]
    fn sprintf_quoting_matches_go() {
        assert_eq!(sprintf("%q", &vals!["hi"]), "\"hi\"");
        assert_eq!(sprintf("%q", &vals!["a\nb"]), "\"a\\nb\"");
        // The escape byte used all over this crate.
        assert_eq!(
            sprintf("%q", &vals!["\u{1b}[31mfoo\u{1b}[0m"]),
            "\"\\x1b[31mfoo\\x1b[0m\""
        );
        assert_eq!(sprintf("%q", &vals!['A']), "'A'");
    }

    #[test]
    fn sprintf_error_diagnostics_match_go() {
        assert_eq!(sprintf("%s", &[]), "%!s(MISSING)");
        assert_eq!(sprintf("%d", &vals!["foo"]), "%!d(string=foo)");
        assert_eq!(sprintf("%s", &vals![1, 2]), "%!s(int=1)%!(EXTRA int=2)");
        assert_eq!(sprintf("%v", &vals![1, 2]), "1%!(EXTRA int=2)");
        assert_eq!(sprintf("hi %", &[]), "hi %!(NOVERB)");
        assert_eq!(sprintf("%v", &vals![Value::Nil]), "<nil>");
        assert_eq!(sprintf("%s", &vals![Value::Nil]), "%!s(<nil>)");
        assert_eq!(
            sprintf("no verbs", &vals![1, "a"]),
            "no verbs%!(EXTRA int=1, string=a)"
        );
    }

    #[test]
    fn sprintf_arg_index_matches_go() {
        assert_eq!(sprintf("%[2]d %[1]d", &vals![1, 2]), "2 1");
        assert_eq!(sprintf("%[3]d", &vals![1, 2]), "%!d(BADINDEX)");
    }

    #[test]
    fn sprintf_star_width_matches_go() {
        assert_eq!(sprintf("%*d", &vals![5, 42]), "   42");
        assert_eq!(sprintf("%-*d|", &vals![5, 42]), "42   |");
        assert_eq!(sprintf("%.*f", &vals![2, 3.14159]), "3.14");
    }

    #[test]
    fn slice_and_bytes_match_go() {
        assert_eq!(sprintf("%v", &[Value::Slice(vals![1, 2, 3])]), "[1 2 3]");
        assert_eq!(sprintf("%v", &[Value::Bytes(vec![1, 2, 3])]), "[1 2 3]");
        assert_eq!(sprintf("%s", &[Value::Bytes(b"abc".to_vec())]), "abc");
        assert_eq!(sprintf("%x", &[Value::Bytes(b"abc".to_vec())]), "616263");
        assert_eq!(sprintf("%x", &vals!["abc"]), "616263");
        assert_eq!(sprintf("% x", &vals!["abc"]), "61 62 63");
    }

    #[test]
    fn floats_match_go() {
        assert_eq!(sprintf("%v", &vals![1.5]), "1.5");
        assert_eq!(sprintf("%v", &vals![1.0]), "1");
        assert_eq!(sprintf("%f", &vals![1.5]), "1.500000");
        assert_eq!(sprintf("%.2f", &vals![1.005]), "1.00");
        assert_eq!(sprintf("%e", &vals![1234.5678]), "1.234568e+03");
        assert_eq!(sprintf("%v", &vals![1e21]), "1e+21");
        assert_eq!(sprint(&vals![1.0, 2.0]), "1 2");
    }
}
