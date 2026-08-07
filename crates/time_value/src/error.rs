//! The crate's error type.

use core::fmt;

/// Everything this crate can fail with.
///
/// The variants distinguish the two kinds of failure the
/// [knowledge bundle](https://github.com/ojhermann-org/time-value/tree/main/knowledge)
/// separates: one where the inputs are individually valid but jointly
/// meaningless, and one where the arithmetic left what the representation can
/// hold. A caller may reasonably handle those differently — the first is a
/// modelling mistake, the second is a limit.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// The accumulation factor `1 + rt` is not strictly positive.
    ///
    /// A domain failure. Such a factor would flip the sign of whatever it
    /// multiplies, turning money held into money owed by nothing but elapsed
    /// time, so it is refused rather than applied.
    NonPositiveFactor {
        /// The offending factor, carried so a caller can see how far past the
        /// boundary the inputs went.
        factor: f64,
    },

    /// A computed value left the range the representation can hold.
    ///
    /// A representation failure, not a modelling one: the same inputs in a
    /// wider representation would succeed.
    NotFinite,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::NonPositiveFactor { factor } => {
                write!(f, "accumulation factor `1 + rt` is {factor}, not positive")
            }
            Self::NotFinite => f.write_str("result is not finite"),
        }
    }
}

impl core::error::Error for Error {}

/// A [`Result`](core::result::Result) whose error is this crate's [`Error`].
pub type Result<T> = core::result::Result<T, Error>;
