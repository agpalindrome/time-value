//! The crate's error type.

use core::fmt;

/// Which quantity a failure is about.
///
/// Carried by [`Error::NotFinite`], which several operations can raise. Without
/// it a caller learns only that *something* left the representable range, and
/// the remedies differ: a bad input is fixed at the call site, an overflowed
/// result is fixed by rescaling the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Quantity {
    /// A quantity of money.
    Amount,
    /// A simple interest rate.
    Rate,
    /// A span of elapsed periods.
    Periods,
    /// An accumulation factor.
    Factor,
    /// A comparison tolerance.
    Tolerance,
    /// The result of applying a factor to an amount.
    Product,
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            Self::Amount => "amount",
            Self::Rate => "rate",
            Self::Periods => "elapsed periods",
            Self::Factor => "accumulation factor",
            Self::Tolerance => "tolerance",
            Self::Product => "product",
        })
    }
}

/// Everything this crate can fail with.
///
/// The variants separate two kinds of failure: one where the inputs are
/// individually valid but jointly meaningless, and one where a value left what
/// the representation can hold. A caller may reasonably handle those
/// differently — the first is a modelling mistake, the second a limit.
///
/// Not `Copy`, deliberately: a variant carrying a source would forfeit it, and
/// [`Error::Unparsable`] already does carry one.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// The accumulation factor `1 + rt` is not strictly positive.
    ///
    /// A domain failure. Such a factor would flip the sign of whatever it
    /// multiplies, turning money held into money owed by nothing but elapsed
    /// time, so it is refused rather than applied.
    #[non_exhaustive]
    NonPositiveFactor {
        /// The offending factor.
        factor: f64,
    },

    /// The accumulation factor is too close to zero for its sign to be known.
    ///
    /// A domain failure, and the subtler of the two. `1 + rt` is a difference,
    /// so when `rt` is near `-1` almost every significant digit cancels and
    /// what survives is the rounding error in the inputs rather than the
    /// value the caller meant. The computed factor is then correct about
    /// the stored numbers and arbitrarily wrong about the model — and,
    /// being positive as often as not, would be accepted and applied in
    /// silence.
    ///
    /// Refusing here trades a narrow band of answers for the guarantee that an
    /// accepted factor means something. No factor this small describes a
    /// financial scenario: it is the ratio by which an amount would have to
    /// shrink for the arithmetic to have produced it.
    #[non_exhaustive]
    IndeterminateFactor {
        /// The computed factor, whose sign is not trustworthy.
        factor: f64,
    },

    /// A value is not finite — a NaN or an infinity.
    ///
    /// Either an input that was never a definite quantity, or arithmetic that
    /// left the range the representation can hold. The second is a
    /// representation failure rather than a modelling one: the same inputs in a
    /// wider representation would succeed.
    #[non_exhaustive]
    NotFinite {
        /// Which quantity was not finite.
        quantity: Quantity,
    },

    /// A non-zero amount shrank to zero.
    ///
    /// A representation failure at the bottom of the range rather than the top.
    /// The result is not merely imprecise — it has lost the amount entirely,
    /// and a liability that underflows stops comparing as one.
    #[non_exhaustive]
    Underflow,

    /// A comparison tolerance is negative.
    ///
    /// A negative tolerance costs the comparison its reflexivity: a value stops
    /// being close to itself.
    #[non_exhaustive]
    NegativeTolerance {
        /// The offending tolerance.
        tolerance: f64,
    },

    /// A span of elapsed periods is negative.
    ///
    /// A domain failure. Running the formula backwards does not discount — the
    /// inverse of simple accumulation is division, not evaluation at a negative
    /// argument — so a negative span is refused rather than answering a
    /// different question convincingly.
    #[non_exhaustive]
    NegativePeriods {
        /// The offending span.
        periods: f64,
    },

    /// Text that does not parse as the quantity it was read for.
    #[non_exhaustive]
    Unparsable {
        /// Why the text could not be read. Kept because it distinguishes empty
        /// text from malformed text, which this variant alone does not.
        source: core::num::ParseFloatError,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::NonPositiveFactor { factor } => {
                write!(
                    f,
                    "accumulation factor `1 + rt` is `{factor}`, not positive"
                )
            }
            Self::IndeterminateFactor { factor } => write!(
                f,
                "accumulation factor `1 + rt` is `{factor}`, too near zero for its sign to be known"
            ),
            Self::NotFinite { quantity } => write!(f, "{quantity} is not finite"),
            Self::Underflow => f.write_str("a non-zero amount shrank to zero"),
            Self::NegativeTolerance { tolerance } => {
                write!(f, "tolerance `{tolerance}` is negative")
            }
            Self::NegativePeriods { periods } => {
                write!(f, "elapsed periods `{periods}` is negative")
            }
            Self::Unparsable { .. } => f.write_str("text does not read as a number"),
        }
    }
}

impl core::error::Error for Error {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Unparsable { source } => Some(source),
            _ => None,
        }
    }
}

/// A [`Result`](core::result::Result) whose error is this crate's [`Error`].
pub type Result<T> = core::result::Result<T, Error>;
