//! Edge cases for the Go `fmt` port that the generated golden matrix does not
//! reach (escape classes, NaN/Inf padding, `BADINDEX`, subnormals, `%b` on
//! floats, …).
//!
//! Every expected value in this file was produced by running the equivalent
//! `fmt.Printf` against the real Go toolchain; the reference output is quoted
//! next to each group.

use color::gofmt::{go_quote, sprint, sprintf, sprintln};
use color::{vals, Value};

/// ```text
/// %q "\a\b\f\r\v\t\n\x00\x7f"  ->  "\a\b\f\r\v\t\n\x00\x7f"
/// %q "世é😀"                   ->  "世é😀"
/// %+q "世é"                    ->  "\u4e16\u00e9"
/// %#q "plain" / "has`tick"     ->  `plain` / "has`tick"
/// %q ""  / %#q ""              ->  "" / ``
/// ```
#[test]
fn quoting_covers_every_escape_class() {
    assert_eq!(
        sprintf("%q", &vals!["\u{7}\u{8}\u{c}\r\u{b}\t\n\u{0}\u{7f}"]),
        r#""\a\b\f\r\v\t\n\x00\x7f""#
    );
    // Printable non-ASCII is emitted verbatim.
    assert_eq!(sprintf("%q", &vals!["世é😀"]), "\"世é😀\"");
    // `+` forces ASCII-only escaping via \u / \U (never \x for non-controls).
    assert_eq!(sprintf("%+q", &vals!["世é"]), r#""\u4e16\u00e9""#);
    assert_eq!(sprintf("%+q", &vals!["😀"]), r#""\U0001f600""#);
    // `#` uses a raw string when possible.
    assert_eq!(sprintf("%#q", &vals!["plain"]), "`plain`");
    assert_eq!(sprintf("%#q", &vals!["has`tick"]), "\"has`tick\"");
    assert_eq!(sprintf("%q", &vals![""]), "\"\"");
    assert_eq!(sprintf("%#q", &vals![""]), "``");
    // The standalone helper.
    assert_eq!(go_quote("a\tb"), r#""a\tb""#);
}

/// ```text
/// [%8f] [%-8f] [%08f] [%+f] [% f] on NaN -> [     NaN] [NaN     ] [     NaN] [+NaN] [ NaN]
/// [%8f] [%-8f] [%08f] on +Inf, [%+f] on -Inf -> [    +Inf] [+Inf    ] [    +Inf] [-Inf]
/// ```
#[test]
fn nan_and_inf_are_padded_with_spaces_never_zeros() {
    let nan = f64::NAN;
    assert_eq!(sprintf("%8f", &vals![nan]), "     NaN");
    assert_eq!(sprintf("%-8f", &vals![nan]), "NaN     ");
    assert_eq!(
        sprintf("%08f", &vals![nan]),
        "     NaN",
        "the zero flag must be ignored for NaN"
    );
    assert_eq!(sprintf("%+f", &vals![nan]), "+NaN");
    assert_eq!(sprintf("% f", &vals![nan]), " NaN");

    assert_eq!(sprintf("%8f", &vals![f64::INFINITY]), "    +Inf");
    assert_eq!(sprintf("%-8f", &vals![f64::INFINITY]), "+Inf    ");
    assert_eq!(sprintf("%08f", &vals![f64::INFINITY]), "    +Inf");
    assert_eq!(sprintf("%+f", &vals![f64::NEG_INFINITY]), "-Inf");
}

/// ```text
/// %[9]d with 1 arg      -> %!d(BADINDEX)
/// %[1]d%[9]d with 1 arg -> 1%!d(BADINDEX)
/// ```
#[test]
fn out_of_range_argument_index_reports_badindex() {
    assert_eq!(sprintf("%[9]d", &vals![1]), "%!d(BADINDEX)");
    assert_eq!(sprintf("%[1]d%[9]d", &vals![1]), "1%!d(BADINDEX)");
    // An explicit index suppresses the trailing EXTRA report.
    assert_eq!(sprintf("%[1]d", &vals![1, 2, 3]), "1");
}

/// ```text
/// [% #x] [%# x] on "abc" -> [0x61 0x62 0x63] [0x61 0x62 0x63]
/// ```
#[test]
fn hex_encoding_honours_sharp_and_space_together() {
    assert_eq!(sprintf("% #x", &vals!["abc"]), "0x61 0x62 0x63");
    assert_eq!(sprintf("%# x", &vals!["abc"]), "0x61 0x62 0x63");
    assert_eq!(sprintf("%#x", &vals!["abc"]), "0x616263");
    assert_eq!(sprintf("% x", &vals!["abc"]), "61 62 63");
    // Empty input takes the width-only path.
    assert_eq!(sprintf("%5x", &vals![""]), "     ");
    assert_eq!(sprintf("%x", &vals![""]), "");
}

/// ```text
/// %v | %e | %x  of SmallestNonzeroFloat64 -> 5e-324 | 4.940656e-324 | 0x1p-1074
/// %b of 1.0 / 0.0                         -> 4503599627370496p-52 | 0p-1074
/// %v of MaxFloat64                        -> 1.7976931348623157e+308
/// %v | %f of negative zero                -> -0 | -0.000000
/// %e of 0.0 / 1e-300, %E of 1234.5
/// ```
#[test]
fn float_extremes_match_go() {
    let tiny = f64::from_bits(1); // math.SmallestNonzeroFloat64
    assert_eq!(sprintf("%v", &vals![tiny]), "5e-324");
    assert_eq!(sprintf("%e", &vals![tiny]), "4.940656e-324");
    assert_eq!(sprintf("%x", &vals![tiny]), "0x1p-1074");

    assert_eq!(sprintf("%b", &vals![1.0]), "4503599627370496p-52");
    assert_eq!(sprintf("%b", &vals![0.0]), "0p-1074");

    assert_eq!(sprintf("%v", &vals![f64::MAX]), "1.7976931348623157e+308");
    assert_eq!(sprintf("%v", &vals![-f64::MAX]), "-1.7976931348623157e+308");

    assert_eq!(sprintf("%v", &vals![-0.0f64]), "-0");
    assert_eq!(sprintf("%f", &vals![-0.0f64]), "-0.000000");

    assert_eq!(sprintf("%e", &vals![0.0]), "0.000000e+00");
    assert_eq!(sprintf("%E", &vals![1234.5]), "1.234500E+03");
    assert_eq!(sprintf("%e", &vals![1e-300]), "1.000000e-300");
}

/// ```text
/// %c | %q | %U of -1 -> � | '�' | U+FFFFFFFFFFFFFFFF
/// ```
#[test]
fn out_of_range_runes_clamp_to_replacement_char() {
    assert_eq!(sprintf("%c", &vals![-1]), "\u{fffd}");
    assert_eq!(sprintf("%q", &vals![-1]), "'\u{fffd}'");
    assert_eq!(sprintf("%U", &vals![-1]), "U+FFFFFFFFFFFFFFFF");
    // Surrogate code points are not valid runes either.
    assert_eq!(sprintf("%c", &vals![0xD800]), "\u{fffd}");

    // Values above utf8.MaxRune whose *low 32 bits* happen to be a valid rune
    // must still clamp — truncating to u32 would wrongly yield 'A'.
    //
    // ```text
    // 0x100000041 -> %c="�"  %q='�'  %U=U+100000041
    // 0x200000041 -> %c="�"  %q='�'  %U=U+200000041
    // 0x110000    -> %c="�"  %q='�'  %U=U+110000
    // 0x10ffff    -> %c="\U0010ffff"
    // ```
    for v in [0x1_0000_0041i64, 0x2_0000_0041, 0x11_0000] {
        assert_eq!(
            sprintf("%c", &vals![v]),
            "\u{fffd}",
            "{v:#x} is above utf8.MaxRune and must clamp"
        );
        assert_eq!(sprintf("%q", &vals![v]), "'\u{fffd}'");
    }
    assert_eq!(sprintf("%U", &vals![0x1_0000_0041i64]), "U+100000041");
    // The boundary itself is a valid rune.
    assert_eq!(sprintf("%c", &vals![0x10FFFFi64]), "\u{10ffff}");
}

/// ```text
/// [%*.*s] with 8, 2, "hello" -> [      he]
/// ```
#[test]
fn star_width_and_precision_combine() {
    assert_eq!(sprintf("%*.*s", &vals![8, 2, "hello"]), "      he");
    assert_eq!(sprintf("%-*.*s|", &vals![8, 2, "hello"]), "he      |");
    // A negative star width flips to left alignment, as in Go.
    assert_eq!(sprintf("%*d|", &vals![-5, 42]), "42   |");
}

/// `%T` reports the Go dynamic type and honours width/precision.
#[test]
fn type_verb_reports_go_types() {
    assert_eq!(sprintf("%T", &vals!["s"]), "string");
    assert_eq!(sprintf("%T", &vals![1]), "int");
    assert_eq!(sprintf("%T", &vals![1u8]), "uint8");
    assert_eq!(sprintf("%T", &vals![1.0]), "float64");
    assert_eq!(sprintf("%T", &vals!['c']), "int32");
    assert_eq!(sprintf("%T", &[Value::Nil]), "<nil>");
    assert_eq!(sprintf("%T", &[Value::Bytes(vec![1])]), "[]uint8");
    assert_eq!(
        sprintf("%T", &[Value::Slice(vec![Value::Int(1)])]),
        "[]interface {}"
    );
    assert_eq!(
        sprintf("%T", &[Value::Error("e".into())]),
        "*errors.errorString"
    );
    assert_eq!(sprintf("%.3T", &vals!["s"]), "str");
    assert_eq!(sprintf("%10T", &vals![1]), "       int");
}

/// `%p` has no meaningful Rust equivalent and always reports the diagnostic Go
/// emits for non-pointer kinds.
#[test]
fn pointer_verb_reports_bad_verb() {
    assert_eq!(sprintf("%p", &vals![1]), "%!p(int=1)");
    assert_eq!(sprintf("%p", &vals!["s"]), "%!p(string=s)");
    assert_eq!(sprintf("%p", &[Value::Nil]), "%!p(<nil>)");
}

/// Sprint/Sprintln spacing with the full range of dynamic types.
#[test]
fn sprint_spacing_uses_reflect_kind_string() {
    // Only `string` counts as a string kind.
    assert_eq!(sprint(&vals!["a", "b"]), "ab");
    assert_eq!(
        sprint(&[Value::Bytes(b"a".to_vec()), Value::Str("b".into())]),
        "[97]b"
    );
    assert_eq!(
        sprint(&[Value::Error("e".into()), Value::Error("f".into())]),
        "e f",
        "error is not a string kind, so a space is inserted"
    );
    assert_eq!(sprint(&[Value::Nil, Value::Nil]), "<nil> <nil>");
    assert_eq!(sprint(&[Value::Nil, Value::Str("x".into())]), "<nil>x");
    assert_eq!(sprintln(&[Value::Nil, Value::Str("x".into())]), "<nil> x\n");
}

/// Malformed directives never panic.
#[test]
fn malformed_directives_are_reported_not_panicked() {
    assert_eq!(sprintf("%", &[]), "%!(NOVERB)");
    assert_eq!(sprintf("abc%", &[]), "abc%!(NOVERB)");
    assert_eq!(sprintf("%z", &vals![1]), "%!z(int=1)");
    assert_eq!(sprintf("%!", &[]), "%!!(MISSING)");
    assert_eq!(sprintf("%*d", &vals!["notanint", 1]), "%!(BADWIDTH)1");
    assert_eq!(
        sprintf("%.*f", &vals!["notanint", 1.0]),
        "%!(BADPREC)1.000000"
    );
    assert_eq!(
        sprintf("%1000000000000d", &vals![1]),
        "%!(NOVERB)%!(EXTRA int=1)",
        "an over-long width aborts the scan but the EXTRA report still runs"
    );
    // Unicode verb.
    assert_eq!(sprintf("%世", &vals![1]), "%!世(int=1)");
}

/// Escape of the quote character and the backslash itself, plus the
/// non-printable classes Rust's `char` API does not flag as control.
///
/// ```text
/// %q  `a"b\c`          -> "a\"b\\c"
/// %q  '\'' / '\\'      -> '\'' / '\\'
/// %q  U+00A0/2028/FEFF -> "\u00a0" "\u2028" "\ufeff"
/// %q  U+200B           -> "\u200b"
/// ```
#[test]
fn quote_escapes_delimiters_and_invisible_runes() {
    assert_eq!(sprintf("%q", &vals![r#"a"b\c"#]), r#""a\"b\\c""#);
    assert_eq!(sprintf("%q", &vals!['\'']), r"'\''");
    assert_eq!(sprintf("%q", &vals!['\\']), r"'\\'");

    // Non-ASCII whitespace and format characters are not `unicode.IsPrint`.
    assert_eq!(sprintf("%q", &vals!["\u{a0}"]), r#""\u00a0""#);
    assert_eq!(sprintf("%q", &vals!["\u{2028}"]), r#""\u2028""#);
    assert_eq!(sprintf("%q", &vals!["\u{feff}"]), r#""\ufeff""#);
    assert_eq!(sprintf("%q", &vals!["\u{200b}"]), r#""\u200b""#);
}

/// ```text
/// %O 8 -> 0o10   |  %#o 8 -> 010
/// %#U 'A' -> U+0041 'A'  |  %#U 0x00a0 -> U+00A0   (not printable, no quote)
/// ```
#[test]
fn octal_and_unicode_prefix_forms_match_go() {
    assert_eq!(sprintf("%O", &vals![8]), "0o10");
    assert_eq!(sprintf("%#o", &vals![8]), "010");
    assert_eq!(sprintf("%o", &vals![8]), "10");
    assert_eq!(sprintf("%#U", &vals!['A']), "U+0041 'A'");
    assert_eq!(
        sprintf("%#U", &vals![0x00a0]),
        "U+00A0",
        "a non-printable rune gets no quoted suffix"
    );
}

/// An explicit argument index followed by an explicit width is rejected by Go,
/// while `[n]` combined with a `*` width is accepted.
///
/// ```text
/// %[1]5d  with 42     -> %!d(BADINDEX)
/// %[1]*d  with 5, 42  -> "   42"
/// %[1]d.%[1]d with 42 -> "42.42"
/// ```
#[test]
fn argument_index_interacts_with_width_like_go() {
    assert_eq!(sprintf("%[1]5d", &vals![42]), "%!d(BADINDEX)");
    assert_eq!(sprintf("%[1]*d", &vals![5, 42]), "   42");
    assert_eq!(sprintf("%[1]d.%[1]d", &vals![42]), "42.42");
}

/// `intFromArg` accepts any integer kind but rejects values beyond `tooLarge`.
///
/// ```text
/// %*d with uint8(5), 42        -> "   42"
/// %*d with uint64(MaxUint64)   -> %!(BADWIDTH)42
/// ```
#[test]
fn star_width_accepts_every_integer_kind() {
    assert_eq!(sprintf("%*d", &[Value::Uint8(5), Value::Int(42)]), "   42");
    assert_eq!(sprintf("%*d", &[Value::Uint(5), Value::Int(42)]), "   42");
    assert_eq!(
        sprintf("%*d", &[Value::Rune('\u{5}'), Value::Int(42)]),
        "   42"
    );
    assert_eq!(
        sprintf("%*d", &[Value::Uint(u64::MAX), Value::Int(42)]),
        "%!(BADWIDTH)42"
    );
}

// ---------------------------------------------------------------------------
// Composite values: `printValue`'s depth-sensitive rules
// ---------------------------------------------------------------------------

/// A `nil` *element* is written by `printValue`'s `reflect.Interface` case,
/// which ignores both the verb and the width — unlike a `nil` *operand*, which
/// `printArg` sends down the `badVerb` path.
///
/// ```text
/// %v  / %s / %f / %d / %c / %x / %! of []interface{}{nil}  ->  [<nil>]  (all)
/// %6.v                                                     ->  [<nil>]
/// %#v                                     ->  []interface {}{interface {}(nil)}
/// %d  of []interface{}{[]interface{}{nil}} ->  [[<nil>]]
/// %s  of nil (operand)                     ->  %!s(<nil>)
/// ```
#[test]
fn nil_inside_a_slice_prints_bare_nil_for_every_verb() {
    let one_nil = [Value::Slice(vec![Value::Nil])];
    for verb in ["%v", "%s", "%f", "%d", "%c", "%x", "%!", "%6.v", "%+v"] {
        assert_eq!(
            sprintf(verb, &one_nil),
            "[<nil>]",
            "{verb} of a nil element"
        );
    }
    assert_eq!(
        sprintf("%#v", &one_nil),
        "[]interface {}{interface {}(nil)}",
        "%#v names the interface type"
    );
    assert_eq!(
        sprintf("%#v", &[Value::Slice(vec![Value::Nil, Value::Int(1)])]),
        "[]interface {}{interface {}(nil), 1}"
    );
    // Nesting deeper keeps the same rule.
    assert_eq!(
        sprintf("%d", &[Value::Slice(vec![Value::Slice(vec![Value::Nil])])]),
        "[[<nil>]]"
    );
    // A nil *operand* is unchanged: `printArg` still reports a bad verb.
    assert_eq!(sprintf("%s", &[Value::Nil]), "%!s(<nil>)");
    assert_eq!(sprintf("%v", &[Value::Nil]), "<nil>");
}

/// `errors.New` yields a `*errors.errorString`. `printValue` dereferences a
/// pointer only at depth 0, so the diagnostic names the *string field* for an
/// operand and the *pointer* for an element.
///
/// ```text
/// %n of errors.New("a")                  ->  &{%!n(string=a)}
/// %n of []interface{}{errors.New("a")}   ->  [%!n(*errors.errorString=&{a})]
/// %v / %s / %q / %x of the element       ->  [a] [a] ["a"] [61]
/// %.3n / %20n of the element  ->  [%!n(…=&{abc})] / [%!n(…=&{        abcdefgh})]
/// ```
#[test]
fn error_inside_a_slice_reports_the_pointer_not_the_field() {
    let err = |s: &str| Value::Slice(vec![Value::Error(s.into())]);

    // handleMethods still wins for the string-ish verbs, at any depth.
    assert_eq!(sprintf("%v", &[err("a")]), "[a]");
    assert_eq!(sprintf("%s", &[err("a")]), "[a]");
    assert_eq!(sprintf("%q", &[err("a")]), "[\"a\"]");
    assert_eq!(sprintf("%x", &[err("a")]), "[61]");
    assert_eq!(sprintf("%X", &[err("a")]), "[61]");

    // Every other verb reaches `fmtPointer`, whose `badVerb` names the pointer.
    for verb in ["%n", "%c", "%e", "%E", "%!", "%t", "%U", "%g", "%O"] {
        assert_eq!(
            sprintf(verb, &[err("a")]),
            format!("[%!{}(*errors.errorString=&{{a}})]", &verb[1..]),
            "{verb} of an error element"
        );
    }
    assert_eq!(
        sprintf("% #n", &[err("")]),
        "[%!n(*errors.errorString=&{})]"
    );

    // The directive's width/precision reach the struct field.
    assert_eq!(
        sprintf("%.3n", &[err("abcdefgh")]),
        "[%!n(*errors.errorString=&{abc})]"
    );
    assert_eq!(
        sprintf("%20n", &[err("abcdefgh")]),
        "[%!n(*errors.errorString=&{            abcdefgh})]"
    );
    assert_eq!(
        sprintf("%-20n|", &[err("abcdefgh")]),
        "[%!n(*errors.errorString=&{abcdefgh            })]|"
    );
    assert_eq!(
        sprintf("%020n", &[err("abcdefgh")]),
        "[%!n(*errors.errorString=&{000000000000abcdefgh})]"
    );

    // An error *operand* is unchanged: the pointer is dereferenced first.
    assert_eq!(
        sprintf("%n", &[Value::Error("a".into())]),
        "&{%!n(string=a)}"
    );
    assert_eq!(
        sprintf("%d", &[Value::Error("a".into())]),
        "&{%!d(string=a)}"
    );
    assert_eq!(
        sprintf("%.3n", &[Value::Error("abcdefgh".into())]),
        "&{%!n(string=abc)}"
    );
    assert_eq!(
        sprintf("%#v", &[Value::Error("a".into())]),
        "&errors.errorString{s:\"a\"}"
    );
}

/// `pp.fmtBytes` is handed its type name by the caller, and the two callers
/// spell the same type differently.
///
/// ```text
/// %#v of []byte{0x5d,0x74}                  ->  []byte{0x5d, 0x74}
/// %#v of []interface{}{[]byte{0x5d,0x74}}   ->  []interface {}{[]uint8{0x5d, 0x74}}
/// ```
#[test]
fn byte_slice_type_name_depends_on_depth() {
    let b = Value::Bytes(vec![0x5d, 0x74]);
    assert_eq!(
        sprintf("%#v", std::slice::from_ref(&b)),
        "[]byte{0x5d, 0x74}"
    );
    assert_eq!(
        sprintf("%#v", &[Value::Slice(vec![b.clone()])]),
        "[]interface {}{[]uint8{0x5d, 0x74}}"
    );
    assert_eq!(
        sprintf("%#v", &[Value::Slice(vec![Value::Slice(vec![b.clone()])])]),
        "[]interface {}{[]interface {}{[]uint8{0x5d, 0x74}}}"
    );
    // Non-`#v` rendering is depth-independent.
    assert_eq!(
        sprintf("%v", &[Value::Slice(vec![b.clone()])]),
        "[[93 116]]"
    );
    assert_eq!(sprintf("%s", &[Value::Slice(vec![b])]), "[]t]");
}

// ---------------------------------------------------------------------------
// Precision limits
// ---------------------------------------------------------------------------

/// Go accepts any precision up to `parsenum`'s 1e6 ceiling and pads the result
/// with zeros once the value's exact decimal expansion runs out. Rust's own
/// formatting machinery caps width/precision at `u16::MAX` and *panics* past
/// it, so these paths are generated at a capped width and zero-extended.
///
/// ```text
/// %.70000f 1e20  -> len 70022, "100000000000000000000." + 70000 zeros
/// %.100e   1e20  -> "1." + 100 digits + "e+20"
/// %.900e   0.1   -> exact expansion of the double, then zeros
/// %.20x    1.0   -> 0x1.00000000000000000000p+00
/// %.30X    3.5   -> 0X1.C00000000000000000000000000000P+01
/// %.10U    'A'   -> U+0000000041
/// ```
#[test]
fn precision_beyond_the_rust_format_limit_matches_go() {
    // %f — the exact expansion is 1e20 with a trailing '.', then zeros.
    let s = sprintf("%.70000f", &vals![1e20]);
    assert_eq!(s.len(), 70022);
    assert!(s.starts_with("100000000000000000000."));
    assert!(s[22..].bytes().all(|b| b == b'0'));

    // %e — the exact expansion of 0.1 as a double, then zeros.
    let s = sprintf("%.900e", &vals![0.1]);
    assert_eq!(s.len(), 906);
    assert!(s.starts_with("1.0000000000000000555111512312578270211815834045410156250000"));
    assert!(s.ends_with("e-01"));

    let s = sprintf("%.100e", &vals![1e20]);
    assert_eq!(s.len(), 106);
    assert!(s.starts_with("1.00000000000000000000"));
    assert!(s.ends_with("0e+20"));

    // %g collapses to the shortest form regardless of a large precision.
    assert_eq!(sprintf("%.100g", &vals![1.5]), "1.5");

    // Hex floats pad past the 13 hex digits a float64 fraction holds.
    assert_eq!(
        sprintf("%.20x", &vals![1.0]),
        "0x1.00000000000000000000p+00"
    );
    assert_eq!(
        sprintf("%.30X", &vals![3.5]),
        "0X1.C00000000000000000000000000000P+01"
    );

    // %U treats the precision as a minimum digit count.
    assert_eq!(sprintf("%.10U", &vals!['A']), "U+0000000041");
    assert_eq!(sprintf("%.2U", &vals!['A']), "U+0041");
    let s = sprintf("%.70000U", &vals!['A']);
    assert_eq!(s.len(), 70002);
    assert!(s.ends_with("000041"));

    // A `*` precision is bounded by `tooLarge`, and reaches the same paths.
    let s = sprintf("%.*f", &[Value::Int(1_000_000), Value::Float(1.5)]);
    assert_eq!(s.len(), 1_000_002);
    assert!(s.starts_with("1.5"));
}

/// `%#U` appends the rune *verbatim* between single quotes — `fmtUnicode` does
/// not run it through `strconv.QuoteRune`.
///
/// ```text
/// %#U '\'' -> U+0027 '''      %#U '\\' -> U+005C '\'
/// %#U 'm'  -> U+006D 'm'
/// %##1U of []byte{0x6d,0x27} -> [U+006D 'm' U+0027 ''']
/// ```
#[test]
fn sharp_unicode_does_not_escape_the_quoted_rune() {
    assert_eq!(sprintf("%#U", &vals!['\'']), "U+0027 '''");
    assert_eq!(sprintf("%#U", &vals!['\\']), "U+005C '\\'");
    assert_eq!(sprintf("%#U", &vals!['m']), "U+006D 'm'");
    assert_eq!(
        sprintf("%##1U", &[Value::Bytes(vec![0x6d, 0x27])]),
        "[U+006D 'm' U+0027 ''']"
    );
    // Non-printable runes get no quoted form at all.
    assert_eq!(sprintf("%#U", &vals!['\u{0}']), "U+0000");
}

/// `strconv.CanBackquote` rejects the BOM — *"BOMs are invisible and should not
/// be quoted"* — so `%#q` falls back to an interpreted string for it.
///
/// ```text
/// %#q "\ufeff" -> "\ufeff"      %#q "世界" -> `世界`
/// %#q "abc"    -> `abc`         %#q "a`b"  -> "a`b"
/// ```
#[test]
fn sharp_q_rejects_the_bom_but_keeps_other_multibyte_runes() {
    assert_eq!(sprintf("%#q", &vals!["\u{feff}"]), "\"\\ufeff\"");
    assert_eq!(sprintf("% #q", &vals!["\u{feff}"]), "\"\\ufeff\"");
    assert_eq!(sprintf("%#q", &vals!["世界"]), "`世界`");
    assert_eq!(sprintf("%#q", &vals!["abc"]), "`abc`");
    assert_eq!(sprintf("%#q", &vals!["a`b"]), "\"a`b\"");
    assert_eq!(sprintf("%#q", &vals!["a\nb"]), "\"a\\nb\"");
    assert_eq!(sprintf("%#q", &vals!["a\tb"]), "`a\tb`");
}
