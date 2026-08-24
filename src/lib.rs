//! Crate `color` is an ANSI color package to output colorized or SGR defined
//! output to the standard output. The API can be used in several way, pick one
//! that suits you.
//!
//! This is a line-by-line port of [github.com/fatih/color](https://github.com/fatih/color)
//! (v1.19.0). The Go API is preserved verbatim, adjusted only for Rust naming
//! conventions (`New` → [`Color::new`], `SprintFunc` → [`Color::sprint_func`],
//! `FgRed` → [`FG_RED`], …).
//!
//! Use simple and default helper functions with predefined foreground colors:
//!
//! ```no_run
//! use color::{cyan, blue, red, yellow, magenta, hi_green, hi_black, hi_white, vals, NO_ARGS};
//!
//! cyan("Prints text in cyan.", NO_ARGS);
//!
//! // a newline will be appended automatically
//! blue("Prints %s in blue.", &vals!["text"]);
//!
//! // More default foreground colors..
//! red("We have red", NO_ARGS);
//! yellow("Yellow color too!", NO_ARGS);
//! magenta("And many others ..", NO_ARGS);
//!
//! // Hi-intensity colors
//! hi_green("Bright green color.", NO_ARGS);
//! hi_black("Bright black means gray..", NO_ARGS);
//! hi_white("Shiny white color!", NO_ARGS);
//! ```
//!
//! However, there are times when custom color mixes are required. Below are some
//! examples to create custom color objects and use the print functions of each
//! separate color object.
//!
//! ```no_run
//! use color::{vals, Color, BOLD, BG_WHITE, FG_BLUE, FG_CYAN, FG_RED, UNDERLINE};
//! # use std::io::Write;
//! # let mut my_writer: Vec<u8> = Vec::new();
//!
//! // Create a new color object
//! let mut c = Color::new(&[FG_CYAN]);
//! c.add(&[UNDERLINE]);
//! c.println(&vals!["Prints cyan text with an underline."]).unwrap();
//!
//! // Or just add them to New()
//! let d = Color::new(&[FG_CYAN, BOLD]);
//! d.printf("This prints bold cyan %s\n", &vals!["too!."]).unwrap();
//!
//! // Mix up foreground and background colors, create new mixes!
//! let mut red = Color::new(&[FG_RED]);
//!
//! let bold_red = red.add(&[BOLD]).clone();
//! bold_red.println(&vals!["This will print text in bold red."]).unwrap();
//!
//! let white_background = red.add(&[BG_WHITE]).clone();
//! white_background.println(&vals!["Red text with White background."]).unwrap();
//!
//! // Use your own io::Write output
//! Color::new(&[FG_BLUE]).fprintln(&mut my_writer, &vals!["blue color!"]).unwrap();
//!
//! let blue = Color::new(&[FG_BLUE]);
//! blue.fprint(&mut my_writer, &vals!["This will print text in blue."]).unwrap();
//! ```
//!
//! You can create `print_xxx` functions to simplify even more:
//!
//! ```no_run
//! use color::{vals, Color, BOLD, FG_GREEN, FG_RED, NO_ARGS};
//!
//! // Create a custom print function for convenient
//! let red = Color::new(&[FG_RED]).printf_func();
//! red("warning", NO_ARGS);
//! red("error: %s", &vals!["file not found"]);
//!
//! // Mix up multiple attributes
//! let notice = Color::new(&[BOLD, FG_GREEN]).println_func();
//! notice(&vals!["don't forget this..."]);
//! ```
//!
//! You can also `fprint_xxx` functions to pass your own writer:
//!
//! ```no_run
//! use color::{vals, Color, BOLD, FG_BLUE, FG_GREEN};
//! # let mut my_writer: Vec<u8> = Vec::new();
//!
//! let blue = Color::new(&[FG_BLUE]).fprintf_func();
//! blue(&mut my_writer, "important notice: %s", &vals!["***"]);
//!
//! // Mix up with multiple attributes
//! let success = Color::new(&[BOLD, FG_GREEN]).fprintln_func();
//! success(&mut my_writer, &vals!["don't forget this..."]);
//! ```
//!
//! Or create `sprint_xxx` functions to mix strings with other non-colorized strings:
//!
//! ```no_run
//! use color::{gofmt, vals, Color, BG_GREEN, FG_RED, FG_WHITE, FG_YELLOW};
//!
//! let yellow = Color::new(&[FG_YELLOW]).sprint_func();
//! let red = Color::new(&[FG_RED]).sprint_func();
//!
//! print!("{}", gofmt::sprintf(
//!     "this is a %s and this is %s.\n",
//!     &vals![yellow(&vals!["warning"]), red(&vals!["error"])],
//! ));
//!
//! let info = Color::new(&[FG_WHITE, BG_GREEN]).sprint_func();
//! print!("{}", gofmt::sprintf("this %s rocks!\n", &vals![info(&vals!["package"])]));
//! ```
//!
//! Windows support is enabled by default. All print functions work as intended.
//! However, only for the `sprint_xxx` functions, users should write to
//! [`output`]:
//!
//! ```no_run
//! use color::{gofmt, output, vals, green_string, Color, BG_GREEN, FG_WHITE, NO_ARGS};
//! use std::io::Write;
//!
//! let mut out = output();
//! write!(out, "{}", gofmt::sprintf("Windows support: %s", &vals![green_string("PASS", NO_ARGS)])).unwrap();
//!
//! let info = Color::new(&[FG_WHITE, BG_GREEN]).sprint_func();
//! write!(out, "{}", gofmt::sprintf("this %s rocks!\n", &vals![info(&vals!["package"])])).unwrap();
//! ```
//!
//! Using with existing code is possible. Just use the [`set`] function to set the
//! standard output to the given parameters. That way a rewrite of an existing
//! code is not required.
//!
//! ```no_run
//! use color::{set, unset, BOLD, FG_MAGENTA, FG_YELLOW};
//!
//! // Use handy standard colors.
//! set(&[FG_YELLOW]);
//!
//! println!("Existing text will be now in Yellow");
//! println!("This one {}", "too");
//!
//! unset(); // don't forget to unset
//!
//! // You can mix up parameters
//! set(&[FG_MAGENTA, BOLD]);
//!
//! println!("All text will be now bold magenta.");
//! unset();
//! ```
//!
//! There might be a case where you want to disable color output (for example to
//! pipe the standard output of your app to somewhere else). `Color` has support to
//! disable colors both globally and for single color definition. For example
//! suppose you have a CLI app and a `--no-color` bool flag. You can easily disable
//! the color output with:
//!
//! ```no_run
//! use color::set_no_color;
//!
//! let flag_no_color = true;
//! if flag_no_color {
//!     set_no_color(true); // disables colorized output
//! }
//! ```
//!
//! You can also disable the color by setting the `NO_COLOR` environment variable
//! to any value.
//!
//! It also has support for single color definitions (local). You can
//! disable/enable color output on the fly:
//!
//! ```no_run
//! use color::{vals, Color, FG_CYAN};
//!
//! let mut c = Color::new(&[FG_CYAN]);
//! c.println(&vals!["Prints cyan text"]).unwrap();
//!
//! c.disable_color();
//! c.println(&vals!["This is printed without any color"]).unwrap();
//!
//! c.enable_color();
//! c.println(&vals!["This prints again cyan..."]).unwrap();
//! ```

#![deny(missing_docs)]

mod attribute;
mod color;
/// A port of the subset of Go's `fmt` package used by this crate.
pub mod gofmt;
mod value;
mod writer;

pub use attribute::{
    Attribute, BG_BLACK, BG_BLUE, BG_CYAN, BG_GREEN, BG_HI_BLACK, BG_HI_BLUE, BG_HI_CYAN,
    BG_HI_GREEN, BG_HI_MAGENTA, BG_HI_RED, BG_HI_WHITE, BG_HI_YELLOW, BG_MAGENTA, BG_RED, BG_WHITE,
    BG_YELLOW, BLINK_RAPID, BLINK_SLOW, BOLD, CONCEALED, CROSSED_OUT, FAINT, FG_BLACK, FG_BLUE,
    FG_CYAN, FG_GREEN, FG_HI_BLACK, FG_HI_BLUE, FG_HI_CYAN, FG_HI_GREEN, FG_HI_MAGENTA, FG_HI_RED,
    FG_HI_WHITE, FG_HI_YELLOW, FG_MAGENTA, FG_RED, FG_WHITE, FG_YELLOW, ITALIC, RESET,
    RESET_BLINKING, RESET_BOLD, RESET_CONCEALED, RESET_CROSSED_OUT, RESET_ITALIC, RESET_REVERSED,
    RESET_UNDERLINE, REVERSE_VIDEO, UNDERLINE,
};

pub use color::{
    bg_rgb, black, black_string, blue, blue_string, cyan, cyan_string, equals, error, green,
    green_string, hi_black, hi_black_string, hi_blue, hi_blue_string, hi_cyan, hi_cyan_string,
    hi_green, hi_green_string, hi_magenta, hi_magenta_string, hi_red, hi_red_string, hi_white,
    hi_white_string, hi_yellow, hi_yellow_string, magenta, magenta_string, new, no_color, output,
    red, red_string, rgb, set, set_error, set_no_color, set_output, unset, white, white_string,
    yellow, yellow_string, Color, Result, WriteError,
};

pub use value::{Value, NO_ARGS};

pub use writer::{
    new_colorable_stderr, new_colorable_stdout, set_stderr_available, set_stdout_available,
    std_err, std_out, stderr_available, stdout_available, stdout_is_terminal, SharedBuffer, Writer,
};

/// Internal helpers exposed for the migrated test-suite, mirroring the
/// unexported Go identifiers that `color_test.go` exercises directly.
#[doc(hidden)]
pub mod internal {
    /// Port of the unexported `noColorIsSet()`.
    pub fn no_color_is_set() -> bool {
        crate::color::no_color_is_set()
    }
}
