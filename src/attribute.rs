//! SGR attributes.
//!
//! Direct port of the `Attribute` type and its constant blocks from `color.go`.
//!
//! In Go the type is declared as `type Attribute int` and the constants are
//! generated with `iota`. Here the type is a newtype over `i32` (Go's `int` is
//! 64-bit on all platforms `color` supports, but every value used by the
//! package — including 24-bit RGB components — fits comfortably in `i32`; the
//! newtype keeps the `Attribute(r)` conversions used by `RGB`/`BgRGB` valid).

use std::fmt;

/// Attribute defines a single SGR Code.
///
/// Port of Go's `type Attribute int`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Attribute(pub i32);

impl Attribute {
    /// Returns the underlying SGR code as an `i32`.
    ///
    /// Equivalent to Go's `int(attr)` conversion.
    #[inline]
    pub const fn code(self) -> i32 {
        self.0
    }
}

impl fmt::Display for Attribute {
    /// Formats the attribute as its decimal SGR code, matching Go's `%d` verb
    /// applied to an `Attribute` (which has no `String()` method, so `fmt`
    /// falls back to the underlying integer).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl fmt::Debug for Attribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl From<i32> for Attribute {
    #[inline]
    fn from(v: i32) -> Self {
        Attribute(v)
    }
}

impl From<Attribute> for i32 {
    #[inline]
    fn from(v: Attribute) -> Self {
        v.0
    }
}

// ---------------------------------------------------------------------------
// Base attributes
//
//	const (
//		Reset Attribute = iota
//		Bold
//		...
//	)
// ---------------------------------------------------------------------------

/// Resets all attributes.
pub const RESET: Attribute = Attribute(0);
/// Bold / increased intensity.
pub const BOLD: Attribute = Attribute(1);
/// Faint / decreased intensity.
pub const FAINT: Attribute = Attribute(2);
/// Italic.
pub const ITALIC: Attribute = Attribute(3);
/// Underline.
pub const UNDERLINE: Attribute = Attribute(4);
/// Slow blink.
pub const BLINK_SLOW: Attribute = Attribute(5);
/// Rapid blink.
pub const BLINK_RAPID: Attribute = Attribute(6);
/// Reverse video (swap foreground/background).
pub const REVERSE_VIDEO: Attribute = Attribute(7);
/// Concealed / hidden.
pub const CONCEALED: Attribute = Attribute(8);
/// Crossed out / strike-through.
pub const CROSSED_OUT: Attribute = Attribute(9);

// ---------------------------------------------------------------------------
// Reset attributes
//
//	const (
//		ResetBold Attribute = iota + 22
//		ResetItalic
//		ResetUnderline
//		ResetBlinking
//		_
//		ResetReversed
//		ResetConcealed
//		ResetCrossedOut
//	)
// ---------------------------------------------------------------------------

/// Turns off bold/faint.
pub const RESET_BOLD: Attribute = Attribute(22);
/// Turns off italic.
pub const RESET_ITALIC: Attribute = Attribute(23);
/// Turns off underline.
pub const RESET_UNDERLINE: Attribute = Attribute(24);
/// Turns off blinking.
pub const RESET_BLINKING: Attribute = Attribute(25);
// Attribute(26) is the blank `_` identifier in Go and is intentionally unnamed.
/// Turns off reverse video.
pub const RESET_REVERSED: Attribute = Attribute(27);
/// Turns off conceal.
pub const RESET_CONCEALED: Attribute = Attribute(28);
/// Turns off crossed out.
pub const RESET_CROSSED_OUT: Attribute = Attribute(29);

/// Port of Go's `mapResetAttributes` lookup table.
///
/// Returns the specific "reset" attribute for the given attribute, or `None`
/// when the attribute has no dedicated reset code (in which case callers fall
/// back to the generic [`RESET`]).
///
/// Implemented as a `match` rather than a `HashMap` so that it is a pure,
/// allocation-free, thread-safe lookup — the Go map is never mutated after
/// initialization, so the observable behavior is identical.
pub(crate) fn map_reset_attributes(attr: Attribute) -> Option<Attribute> {
    match attr {
        BOLD => Some(RESET_BOLD),
        FAINT => Some(RESET_BOLD),
        ITALIC => Some(RESET_ITALIC),
        UNDERLINE => Some(RESET_UNDERLINE),
        BLINK_SLOW => Some(RESET_BLINKING),
        BLINK_RAPID => Some(RESET_BLINKING),
        REVERSE_VIDEO => Some(RESET_REVERSED),
        CONCEALED => Some(RESET_CONCEALED),
        CROSSED_OUT => Some(RESET_CROSSED_OUT),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Foreground text colors
//
//	const (
//		FgBlack Attribute = iota + 30
//		...
//		foreground // used internally for 256 and 24-bit coloring
//	)
// ---------------------------------------------------------------------------

/// Black foreground.
pub const FG_BLACK: Attribute = Attribute(30);
/// Red foreground.
pub const FG_RED: Attribute = Attribute(31);
/// Green foreground.
pub const FG_GREEN: Attribute = Attribute(32);
/// Yellow foreground.
pub const FG_YELLOW: Attribute = Attribute(33);
/// Blue foreground.
pub const FG_BLUE: Attribute = Attribute(34);
/// Magenta foreground.
pub const FG_MAGENTA: Attribute = Attribute(35);
/// Cyan foreground.
pub const FG_CYAN: Attribute = Attribute(36);
/// White foreground.
pub const FG_WHITE: Attribute = Attribute(37);

/// Used internally for 256 and 24-bit coloring (Go's unexported `foreground`).
pub(crate) const FOREGROUND: Attribute = Attribute(38);

// ---------------------------------------------------------------------------
// Foreground Hi-Intensity text colors
// ---------------------------------------------------------------------------

/// Hi-intensity black (gray) foreground.
pub const FG_HI_BLACK: Attribute = Attribute(90);
/// Hi-intensity red foreground.
pub const FG_HI_RED: Attribute = Attribute(91);
/// Hi-intensity green foreground.
pub const FG_HI_GREEN: Attribute = Attribute(92);
/// Hi-intensity yellow foreground.
pub const FG_HI_YELLOW: Attribute = Attribute(93);
/// Hi-intensity blue foreground.
pub const FG_HI_BLUE: Attribute = Attribute(94);
/// Hi-intensity magenta foreground.
pub const FG_HI_MAGENTA: Attribute = Attribute(95);
/// Hi-intensity cyan foreground.
pub const FG_HI_CYAN: Attribute = Attribute(96);
/// Hi-intensity white foreground.
pub const FG_HI_WHITE: Attribute = Attribute(97);

// ---------------------------------------------------------------------------
// Background text colors
// ---------------------------------------------------------------------------

/// Black background.
pub const BG_BLACK: Attribute = Attribute(40);
/// Red background.
pub const BG_RED: Attribute = Attribute(41);
/// Green background.
pub const BG_GREEN: Attribute = Attribute(42);
/// Yellow background.
pub const BG_YELLOW: Attribute = Attribute(43);
/// Blue background.
pub const BG_BLUE: Attribute = Attribute(44);
/// Magenta background.
pub const BG_MAGENTA: Attribute = Attribute(45);
/// Cyan background.
pub const BG_CYAN: Attribute = Attribute(46);
/// White background.
pub const BG_WHITE: Attribute = Attribute(47);

/// Used internally for 256 and 24-bit coloring (Go's unexported `background`).
pub(crate) const BACKGROUND: Attribute = Attribute(48);

// ---------------------------------------------------------------------------
// Background Hi-Intensity text colors
// ---------------------------------------------------------------------------

/// Hi-intensity black (gray) background.
pub const BG_HI_BLACK: Attribute = Attribute(100);
/// Hi-intensity red background.
pub const BG_HI_RED: Attribute = Attribute(101);
/// Hi-intensity green background.
pub const BG_HI_GREEN: Attribute = Attribute(102);
/// Hi-intensity yellow background.
pub const BG_HI_YELLOW: Attribute = Attribute(103);
/// Hi-intensity blue background.
pub const BG_HI_BLUE: Attribute = Attribute(104);
/// Hi-intensity magenta background.
pub const BG_HI_MAGENTA: Attribute = Attribute(105);
/// Hi-intensity cyan background.
pub const BG_HI_CYAN: Attribute = Attribute(106);
/// Hi-intensity white background.
pub const BG_HI_WHITE: Attribute = Attribute(107);

#[cfg(test)]
mod tests {
    use super::*;

    /// `Attribute` has no `String()` method in Go, so `fmt` prints the
    /// underlying integer for both `%d` and `%v`.
    #[test]
    fn attribute_formats_as_its_sgr_code() {
        assert_eq!(FG_RED.to_string(), "31");
        assert_eq!(format!("{}", BG_HI_WHITE), "107");
        assert_eq!(format!("{:?}", RESET), "0");
        assert_eq!(format!("{:>5}", FG_RED), "   31");
    }

    #[test]
    fn attribute_converts_to_and_from_i32() {
        assert_eq!(Attribute::from(31), FG_RED);
        assert_eq!(i32::from(FG_RED), 31);
        assert_eq!(FG_RED.code(), 31);
        assert_eq!(Attribute::default(), RESET);
    }

    /// Every attribute that Go maps to a dedicated reset code, plus a sample
    /// that falls back to the generic reset.
    #[test]
    fn reset_table_matches_go() {
        for (attr, want) in [
            (BOLD, RESET_BOLD),
            (FAINT, RESET_BOLD),
            (ITALIC, RESET_ITALIC),
            (UNDERLINE, RESET_UNDERLINE),
            (BLINK_SLOW, RESET_BLINKING),
            (BLINK_RAPID, RESET_BLINKING),
            (REVERSE_VIDEO, RESET_REVERSED),
            (CONCEALED, RESET_CONCEALED),
            (CROSSED_OUT, RESET_CROSSED_OUT),
        ] {
            assert_eq!(map_reset_attributes(attr), Some(want), "{attr} reset");
        }
        for attr in [RESET, FG_RED, BG_BLUE, FG_HI_CYAN, FOREGROUND, BACKGROUND] {
            assert_eq!(map_reset_attributes(attr), None, "{attr} has no reset code");
        }
    }
}
