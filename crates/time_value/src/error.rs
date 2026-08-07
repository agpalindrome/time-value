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

    /// A value is not finite — a NaN or an infinity.
    ///
    /// Either an input that was never a definite quantity, or arithmetic that
    /// left the range the representation can hold. The second is a
    /// representation failure rather than a modelling one: the same inputs in a
    /// wider representation would succeed.
    NotFinite,

    /// A span of elapsed periods is negative.
    ///
    /// A domain failure. Running the formula backwards does not discount — the
    /// inverse of simple accumulation is division, not evaluation at a negative
    /// argument — so a negative span is refused rather than answering a
    /// different question convincingly.
    NegativePeriods {
        /// The offending span.
        periods: f64,
    },

    /// Text that does not parse as the quantity it was read for.
    Unparsable,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::NonPositiveFactor { factor } => {
                write!(f, "accumulation factor `1 + rt` is {factor}, not positive")
            }
            Self::NotFinite => f.write_str("value is not finite"),
            Self::NegativePeriods { periods } => {
                write!(f, "elapsed periods is {periods}, which is negative")
            }
            Self::Unparsable => f.write_str("text does not parse as a number"),
        }
    }
}

impl core::error::Error for Error {}

/// A [`Result`](core::result::Result) whose error is this crate's [`Error`].
pub type Result<T> = core::result::Result<T, Error>;
