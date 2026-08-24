//! Port of Go's `strconv.FormatFloat` for `float64`.
//!
//! `fmt` delegates every floating point verb to `strconv.AppendFloat`, so
//! reproducing Go's output requires reproducing `strconv`'s formatting rules —
//! in particular the *shortest round-trip* representation used by `%v`/`%g`
//! and the `%g` exponent-vs-decimal switch-over.
//!
//! Rust's `{:e}` / `{:.*e}` / `{:.*}` formatters already produce
//! shortest-round-trip and correctly-rounded fixed-precision digits, so this
//! module reuses them for digit generation and then re-assembles the result
//! according to Go's layout rules.

/// Decimal digit string plus the position of the decimal point.
///
/// Mirrors Go's `decimalSlice{d, nd, dp}`: the value is
/// `0.<digits> * 10^dp`, i.e. the decimal point sits `dp` digits from the
/// left of `digits`.
struct Digits {
    digits: String,
    dp: i32,
}

/// Extracts the shortest round-trip decimal digits of a finite, non-zero,
/// positive `v`.
fn shortest_digits(v: f64) -> Digits {
    // Rust's LowerExp for f64 yields the shortest representation that round
    // trips, e.g. `1.2345e3`, `5e-1`, `1e2`.
    let s = format!("{:e}", v);
    parse_exp_form(&s)
}

/// The most significant decimal digits any finite `f64` can have.
///
/// Every `f64` is `m * 2^e` with `e >= -1074`, so its decimal expansion
/// terminates; 767 digits is the well-known worst case (`strconv` relies on the
/// same bound). Asking for more than this can only ever append zeros.
const MAX_EXACT_SIG_DIGITS: usize = 800;
/// The most fractional decimal digits any finite `f64` can have: the smallest
/// subnormal is `2^-1074`, whose expansion ends after exactly 1074 places.
const MAX_EXACT_FRAC_DIGITS: usize = 1080;

/// Extracts `nsig` significant decimal digits of a finite, non-zero, positive
/// `v`, correctly rounded (round-half-to-even, matching Go's `strconv`).
///
/// Rust's formatting machinery caps precision at `u16::MAX` and *panics* beyond
/// it, while Go happily renders `%.1000000e`. Since no `f64` has more than
/// [`MAX_EXACT_SIG_DIGITS`] significant digits, anything past that point is
/// exactly zero: the digits are generated at the capped width — where no
/// rounding can occur — and zero-extended to the requested length.
fn digits_with_precision(v: f64, nsig: usize) -> Digits {
    let nsig = nsig.max(1);
    let capped = nsig.min(MAX_EXACT_SIG_DIGITS);
    let s = format!("{:.*e}", capped - 1, v);
    let mut d = parse_exp_form(&s);
    if nsig > d.digits.len() {
        let pad = nsig - d.digits.len();
        d.digits.push_str(&"0".repeat(pad));
    }
    d
}

/// Parses Rust's `<mantissa>e<exp>` form into [`Digits`].
fn parse_exp_form(s: &str) -> Digits {
    let (mantissa, exp) = match s.split_once('e') {
        Some((m, e)) => (m, e.parse::<i32>().unwrap_or(0)),
        None => (s, 0),
    };
    let digits: String = mantissa.chars().filter(|c| c.is_ascii_digit()).collect();
    Digits {
        digits,
        dp: exp + 1,
    }
}

/// Removes trailing zeros from the digit string (Go trims them for `%g`
/// without the `#` flag). Never trims below one digit.
fn trim_trailing_zeros(d: &mut Digits) {
    let trimmed = d.digits.trim_end_matches('0').len();
    if trimmed == 0 {
        // The value rounded to zero; Go collapses this to a single "0" digit
        // and resets the decimal point.
        d.digits.truncate(1);
        d.digits.replace_range(.., "0");
        d.dp = 1;
    } else {
        d.digits.truncate(trimmed);
    }
}

/// Renders `%e`-style output from digits: `d.ddde±dd`.
fn fmt_e(d: &Digits, prec: i32, exp_char: char) -> String {
    let digits = d.digits.as_bytes();
    let mut out = String::new();
    out.push(*digits.first().unwrap_or(&b'0') as char);

    if prec > 0 {
        out.push('.');
        for i in 1..=prec as usize {
            out.push(*digits.get(i).unwrap_or(&b'0') as char);
        }
    }

    out.push(exp_char);

    // Go: the exponent of "0" is written as +00.
    let exp = if d.digits == "0" { 0 } else { d.dp - 1 };
    if exp < 0 {
        out.push('-');
    } else {
        out.push('+');
    }
    let exp = exp.unsigned_abs();
    if exp < 10 {
        // Go always emits at least two exponent digits.
        out.push('0');
        out.push_str(&exp.to_string());
    } else {
        out.push_str(&exp.to_string());
    }
    out
}

/// Renders `%f`-style output from digits: `ddddd.dddd`.
fn fmt_f(d: &Digits, prec: i32) -> String {
    let digits = d.digits.as_bytes();
    let nd = digits.len() as i32;
    let mut out = String::new();

    // Integer part.
    if d.dp > 0 {
        for i in 0..d.dp {
            if i < nd {
                out.push(digits[i as usize] as char);
            } else {
                out.push('0');
            }
        }
    } else {
        out.push('0');
    }

    // Fractional part.
    if prec > 0 {
        out.push('.');
        for i in 0..prec {
            let idx = d.dp + i;
            if idx >= 0 && idx < nd {
                out.push(digits[idx as usize] as char);
            } else {
                out.push('0');
            }
        }
    }
    out
}

/// Port of `strconv.FormatFloat(v, fmt, prec, 64)`.
///
/// `prec` of `-1` selects the shortest representation that round trips.
pub fn format_float(v: f64, format: char, prec: i32) -> String {
    // Special values, matching Go's `fmtE`/`fmt` fast paths.
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "+Inf" } else { "-Inf" }.to_string();
    }

    let neg = v.is_sign_negative();
    let abs = v.abs();
    let body = format_abs(abs, format, prec);

    if neg {
        format!("-{}", body)
    } else {
        body
    }
}

/// Formats the absolute (non-negative, finite) value.
fn format_abs(abs: f64, format: char, prec: i32) -> String {
    match format {
        'b' => return format_b(abs),
        'x' | 'X' => return format_hex(abs, format, prec),
        _ => {}
    }

    let is_zero = abs == 0.0;

    match format {
        'e' | 'E' => {
            let exp_char = if format == 'e' { 'e' } else { 'E' };
            if prec < 0 {
                let d = if is_zero {
                    Digits {
                        digits: "0".into(),
                        dp: 1,
                    }
                } else {
                    shortest_digits(abs)
                };
                let p = d.digits.len() as i32 - 1;
                fmt_e(&d, p, exp_char)
            } else {
                let d = if is_zero {
                    Digits {
                        digits: "0".repeat(prec as usize + 1),
                        dp: 1,
                    }
                } else {
                    digits_with_precision(abs, prec as usize + 1)
                };
                fmt_e(&d, prec, exp_char)
            }
        }
        'f' | 'F' => {
            if prec < 0 {
                let d = if is_zero {
                    Digits {
                        digits: "0".into(),
                        dp: 1,
                    }
                } else {
                    shortest_digits(abs)
                };
                // Shortest %f prints every significant digit after the point.
                let p = (d.digits.len() as i32 - d.dp).max(0);
                fmt_f(&d, p)
            } else {
                // Rust's fixed-precision Display matches Go's rounding here and
                // avoids re-deriving the decimal point for huge magnitudes. Its
                // precision is capped at `u16::MAX` (beyond which it panics),
                // so — as in `digits_with_precision` — the digits are generated
                // at a width no `f64` can exceed and then zero-extended.
                let prec = prec as usize;
                let capped = prec.min(MAX_EXACT_FRAC_DIGITS);
                let mut s = format!("{:.*}", capped, abs);
                if prec > capped {
                    s.push_str(&"0".repeat(prec - capped));
                }
                s
            }
        }
        'g' | 'G' => {
            let exp_char = if format == 'g' { 'e' } else { 'E' };
            let shortest = prec < 0;

            let mut d = if is_zero {
                Digits {
                    digits: "0".into(),
                    dp: 1,
                }
            } else if shortest {
                shortest_digits(abs)
            } else {
                // %g with precision 0 is treated as precision 1 by Go.
                let p = if prec == 0 { 1 } else { prec };
                let mut d = digits_with_precision(abs, p as usize);
                trim_trailing_zeros(&mut d);
                d
            };

            let nd = d.digits.len() as i32;

            // strconv/ftoa.go:
            //   eprec := prec
            //   if eprec > digs.nd && digs.nd >= digs.dp { eprec = digs.nd }
            //   if shortest { eprec = 6 }
            //   exp := digs.dp - 1
            //   if exp < -4 || exp >= eprec { ...%e... }
            //   ...%f...
            let mut eprec = if prec == 0 { 1 } else { prec };
            if eprec > nd && nd >= d.dp {
                eprec = nd;
            }
            if shortest {
                eprec = 6;
            }

            let exp = if is_zero { 0 } else { d.dp - 1 };
            if exp < -4 || exp >= eprec {
                if !shortest {
                    trim_trailing_zeros(&mut d);
                }
                let p = d.digits.len() as i32 - 1;
                fmt_e(&d, p, exp_char)
            } else {
                if !shortest {
                    trim_trailing_zeros(&mut d);
                }
                let p = (d.digits.len() as i32 - d.dp).max(0);
                fmt_f(&d, p)
            }
        }
        _ => {
            // Unknown format character: Go returns "%<char>" via fmtBad; the
            // caller (fmt) never reaches this for supported verbs.
            format!("%{}", format)
        }
    }
}

/// Port of `strconv`'s `'b'` format: decimalless binary exponent notation,
/// e.g. `-123456p-78`, where the mantissa is the 53-bit significand.
fn format_b(abs: f64) -> String {
    if abs == 0.0 {
        return "0p-1074".to_string();
    }
    let bits = abs.to_bits();
    let raw_exp = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & ((1u64 << 52) - 1);

    let (mant, exp) = if raw_exp == 0 {
        // Subnormal.
        (frac, -1074i32)
    } else {
        (frac | (1u64 << 52), raw_exp - 1075)
    };
    format!("{}p{}{}", mant, if exp < 0 { "-" } else { "+" }, exp.abs())
}

/// Port of `strconv`'s `'x'`/`'X'` hexadecimal floating point format,
/// e.g. `0x1.fp+04`.
///
/// `prec` is the number of hexadecimal digits after the point; `-1` selects the
/// shortest form that represents the value exactly.
fn format_hex(abs: f64, format: char, prec: i32) -> String {
    let upper = format == 'X';
    let (prefix, p_char) = if upper { ("0X", 'P') } else { ("0x", 'p') };

    if abs == 0.0 {
        let frac = if prec > 0 {
            format!(".{}", "0".repeat(prec as usize))
        } else {
            String::new()
        };
        return format!("{}0{}{}+00", prefix, frac, p_char);
    }

    let bits = abs.to_bits();
    let raw_exp = ((bits >> 52) & 0x7ff) as i32;
    let mut frac = bits & ((1u64 << 52) - 1);
    let mut exp;

    if raw_exp == 0 {
        // Subnormal: normalize so that the leading digit is 1, as Go does.
        exp = -1022;
        while frac & (1u64 << 52) == 0 {
            frac <<= 1;
            exp -= 1;
        }
        frac &= (1u64 << 52) - 1;
    } else {
        exp = raw_exp - 1023;
    }

    let hex_digits: String = if prec < 0 {
        // Shortest exact form: 52 fraction bits == 13 hex digits, trimmed.
        let mut hex = format!("{:013x}", frac);
        while hex.ends_with('0') {
            hex.pop();
        }
        hex
    } else {
        // Round the 53-bit significand (implicit leading 1 included) to
        // `1 + prec*4` bits, using round-half-to-even like strconv.
        let n = (prec as u32) * 4;
        let mut m: u64 = (1u64 << 52) | frac;
        if n < 52 {
            let shift = 52 - n;
            let kept = m >> shift;
            let rem = m & ((1u64 << shift) - 1);
            let half = 1u64 << (shift - 1);
            let mut kept = kept;
            if rem > half || (rem == half && (kept & 1) == 1) {
                kept += 1;
            }
            // A carry out of the leading bit renormalizes: 2.0 == 1.0 * 2^1.
            if kept >> (n + 1) != 0 {
                kept >>= 1;
                exp += 1;
            }
            m = kept << shift;
        }
        // Re-extract the fraction and render exactly `prec` hex digits. The 52
        // fraction bits are 13 hex digits; a wider precision is padded with
        // zeros rather than slicing past the end of the string.
        let frac_bits = m & ((1u64 << 52) - 1);
        if prec == 0 {
            String::new()
        } else {
            let s = format!("{:013x}", frac_bits);
            let prec = prec as usize;
            if prec <= s.len() {
                s[..prec].to_string()
            } else {
                format!("{}{}", s, "0".repeat(prec - s.len()))
            }
        }
    };

    let hex_digits = if upper {
        hex_digits.to_uppercase()
    } else {
        hex_digits
    };

    let mut out = String::from(prefix);
    out.push('1');
    if !hex_digits.is_empty() {
        out.push('.');
        out.push_str(&hex_digits);
    }
    out.push(p_char);
    out.push(if exp < 0 { '-' } else { '+' });
    let e = exp.unsigned_abs();
    if e < 10 {
        out.push('0');
    }
    out.push_str(&e.to_string());
    out
}

#[cfg(test)]
#[allow(clippy::approx_constant)] // literals mirror Go's reference outputs
mod tests {
    use super::*;

    /// Values cross-checked against `strconv.FormatFloat(v, f, prec, 64)`.
    #[test]
    fn shortest_g_matches_go() {
        assert_eq!(format_float(0.0, 'g', -1), "0");
        assert_eq!(format_float(-0.0, 'g', -1), "-0");
        assert_eq!(format_float(1.0, 'g', -1), "1");
        assert_eq!(format_float(1.5, 'g', -1), "1.5");
        assert_eq!(format_float(0.1, 'g', -1), "0.1");
        assert_eq!(format_float(100000.0, 'g', -1), "100000");
        assert_eq!(format_float(1000000.0, 'g', -1), "1e+06");
        assert_eq!(format_float(1e20, 'g', -1), "1e+20");
        assert_eq!(format_float(1e-5, 'g', -1), "1e-05");
        assert_eq!(format_float(0.0001, 'g', -1), "0.0001");
        assert_eq!(format_float(-1.5e-7, 'g', -1), "-1.5e-07");
        assert_eq!(
            format_float(3.141592653589793, 'g', -1),
            "3.141592653589793"
        );
    }

    #[test]
    fn fixed_formats_match_go() {
        assert_eq!(format_float(1.0, 'f', 6), "1.000000");
        assert_eq!(format_float(1.5, 'f', 2), "1.50");
        assert_eq!(format_float(1234.5678, 'f', 2), "1234.57");
        assert_eq!(format_float(0.0, 'f', 2), "0.00");
        assert_eq!(format_float(1234.5678, 'e', 3), "1.235e+03");
        assert_eq!(format_float(1234.5678, 'E', 3), "1.235E+03");
        assert_eq!(format_float(0.0, 'e', 2), "0.00e+00");
        assert_eq!(format_float(1.0, 'f', -1), "1");
        assert_eq!(format_float(1e20, 'f', -1), "100000000000000000000");
    }

    #[test]
    fn special_values_match_go() {
        assert_eq!(format_float(f64::NAN, 'g', -1), "NaN");
        assert_eq!(format_float(f64::INFINITY, 'g', -1), "+Inf");
        assert_eq!(format_float(f64::NEG_INFINITY, 'g', -1), "-Inf");
    }

    /// `strconv.FormatFloat(v, 'e', -1, 64)` / `'f', -1` — the shortest forms,
    /// reachable through the public `strconv` surface but not through any
    /// `fmt` verb (which always supplies a default precision).
    ///
    /// ```text
    /// e -1 : 1.2345e+03   0e+00
    /// f -1 : 0            1.25
    /// ```
    #[test]
    fn shortest_e_and_f_match_go() {
        assert_eq!(format_float(1234.5, 'e', -1), "1.2345e+03");
        assert_eq!(format_float(0.0, 'e', -1), "0e+00");
        assert_eq!(format_float(-0.0, 'e', -1), "-0e+00");
        assert_eq!(format_float(0.0, 'f', -1), "0");
        assert_eq!(format_float(1.25, 'f', -1), "1.25");
        assert_eq!(format_float(1e-5, 'e', -1), "1e-05");
    }

    /// ```text
    /// b of SmallestNonzeroFloat64 -> 1p-1074
    /// x prec=3 of the same        -> 0x1.000p-1074
    /// ```
    #[test]
    fn subnormals_match_go() {
        let tiny = f64::from_bits(1);
        assert_eq!(format_float(tiny, 'b', -1), "1p-1074");
        assert_eq!(format_float(tiny, 'x', 3), "0x1.000p-1074");
        assert_eq!(format_float(tiny, 'x', -1), "0x1p-1074");
        assert_eq!(format_float(0.0, 'b', -1), "0p-1074");
    }

    #[test]
    fn hex_format_matches_go() {
        assert_eq!(format_float(1.0, 'x', -1), "0x1p+00");
        assert_eq!(format_float(-2.0, 'x', -1), "-0x1p+01");
        assert_eq!(format_float(0.0, 'x', -1), "0x0p+00");
    }
}
