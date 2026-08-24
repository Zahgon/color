//! A model of Go's `interface{}` (`any`) for the variadic print arguments.
//!
//! Go's print API is `func(a ...interface{})`. Rust has no equivalent
//! heterogeneous variadic, so the arguments are modelled with an enum that
//! carries both the value *and* the dynamic Go type information that `fmt`
//! needs:
//!
//!   * `reflect.TypeOf(arg).Kind() == reflect.String` — used by `fmt.Sprint`
//!     to decide whether to insert a separating space.
//!   * `reflect.TypeOf(arg).String()` — used to render `%!d(string=foo)` and
//!     `%!(EXTRA int=3)` diagnostics.
//!
//! Keeping the type tag explicit is what allows the formatting engine in
//! [`crate::gofmt`] to reproduce Go's output byte for byte.

use std::fmt;

/// A dynamically typed value, the Rust stand-in for Go's `interface{}`.
///
/// Use the [`From`] impls or the [`vals!`](crate::vals) macro to build values:
///
/// ```
/// use color::{vals, Value};
///
/// let args = vals!["hello", 123, true];
/// assert_eq!(args[0], Value::Str("hello".to_string()));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// An untyped `nil` interface value.
    Nil,
    /// Go `bool`.
    Bool(bool),
    /// Go signed integer (`int`, `int8` … `int64`).
    Int(i64),
    /// Go unsigned integer (`uint`, `uint16` … `uint64`).
    Uint(u64),
    /// Go `uint8` (a.k.a. `byte`).
    ///
    /// Distinct from [`Value::Uint`] because `fmt` reports the type name in
    /// `%T` and in `%!verb(type=value)` diagnostics, and the elements of a
    /// `[]byte` are `uint8`.
    Uint8(u8),
    /// Go `float64`.
    Float(f64),
    /// Go `string`.
    Str(String),
    /// Go `rune` (`int32` alias) rendered as a character by `%c`/`%q`.
    Rune(char),
    /// Go `[]byte`.
    Bytes(Vec<u8>),
    /// Go `[]interface{}`.
    Slice(Vec<Value>),
    /// Go `error`. Formats via its `Error()` string for `%v`/`%s`.
    Error(String),
}

impl Value {
    /// The Go type name, as produced by `reflect.TypeOf(v).String()`.
    ///
    /// Used verbatim inside `%!verb(type=value)` and `%!(EXTRA type=value)`
    /// diagnostics.
    pub fn go_type_name(&self) -> &'static str {
        match self {
            Value::Nil => "<nil>",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Uint(_) => "uint",
            Value::Uint8(_) => "uint8",
            Value::Float(_) => "float64",
            Value::Str(_) => "string",
            Value::Rune(_) => "int32",
            Value::Bytes(_) => "[]uint8",
            Value::Slice(_) => "[]interface {}",
            Value::Error(_) => "*errors.errorString",
        }
    }

    /// Reports whether the dynamic type's `reflect.Kind` is `reflect.String`.
    ///
    /// `fmt.Sprint` adds a space between operands "when neither is a string",
    /// and that decision is made on exactly this predicate:
    ///
    /// ```go
    /// isString := arg != nil && reflect.TypeOf(arg).Kind() == reflect.String
    /// ```
    ///
    /// Note that a `[]byte` or an `error` is *not* a string kind, matching Go.
    pub fn is_string_kind(&self) -> bool {
        matches!(self, Value::Str(_))
    }

    /// Reports whether this is the untyped `nil` interface.
    pub fn is_nil(&self) -> bool {
        matches!(self, Value::Nil)
    }
}

impl fmt::Display for Value {
    /// Renders the value the way Go's `%v` verb would.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&crate::gofmt::sprint(std::slice::from_ref(self)))
    }
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Str(v.to_string())
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Str(v)
    }
}

impl From<&String> for Value {
    fn from(v: &String) -> Self {
        Value::Str(v.clone())
    }
}

impl From<std::borrow::Cow<'_, str>> for Value {
    fn from(v: std::borrow::Cow<'_, str>) -> Self {
        Value::Str(v.into_owned())
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<char> for Value {
    fn from(v: char) -> Self {
        Value::Rune(v)
    }
}

macro_rules! impl_from_int {
    ($($t:ty),*) => {
        $(impl From<$t> for Value {
            fn from(v: $t) -> Self { Value::Int(v as i64) }
        })*
    };
}
impl_from_int!(i8, i16, i32, i64, isize);

macro_rules! impl_from_uint {
    ($($t:ty),*) => {
        $(impl From<$t> for Value {
            fn from(v: $t) -> Self { Value::Uint(v as u64) }
        })*
    };
}
impl_from_uint!(u16, u32, u64, usize);

impl From<u8> for Value {
    /// Maps to Go's `byte`/`uint8`, which `fmt` names `uint8`.
    fn from(v: u8) -> Self {
        Value::Uint8(v)
    }
}

impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Value::Float(v as f64)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}

impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Value::Bytes(v)
    }
}

impl From<&[u8]> for Value {
    fn from(v: &[u8]) -> Self {
        Value::Bytes(v.to_vec())
    }
}

impl From<Vec<Value>> for Value {
    fn from(v: Vec<Value>) -> Self {
        Value::Slice(v)
    }
}

impl From<&Value> for Value {
    fn from(v: &Value) -> Self {
        v.clone()
    }
}

impl<T> From<Option<T>> for Value
where
    T: Into<Value>,
{
    /// `None` maps to Go's `nil` interface; `Some(x)` maps to `x`.
    fn from(v: Option<T>) -> Self {
        match v {
            Some(v) => v.into(),
            None => Value::Nil,
        }
    }
}

/// Builds a `Vec<Value>` — the Rust equivalent of a Go `...interface{}`
/// argument list.
///
/// ```
/// use color::{vals, Color, FG_RED};
///
/// let mut c = Color::new(&[FG_RED]);
/// // Colorization is disabled by default when stdout is not a terminal, so
/// // force it on to get a deterministic result.
/// c.enable_color();
/// assert_eq!(c.sprint(&vals!["foo", "bar"]), "\u{1b}[31mfoobar\u{1b}[0m");
/// ```
#[macro_export]
macro_rules! vals {
    () => { ::std::vec::Vec::<$crate::Value>::new() };
    ($($x:expr),+ $(,)?) => {
        ::std::vec![$($crate::Value::from($x)),+]
    };
}

/// The empty argument list, a convenience for the very common
/// `f(format)` / `f(format, ...)` split in the helper functions.
pub const NO_ARGS: &[Value] = &[];
