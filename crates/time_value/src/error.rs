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

/// What would fix a failure.
///
/// The two classes [failures are classified by remedy] names, and the reason an
/// error is worth reading at all: they prescribe opposite actions, so telling
/// them apart is what lets a caller act rather than guess.
///
/// **Exhaustive, deliberately.** The principle's test is a question with two
/// answers — would a wider representation change the answer? — so a caller
/// matching both arms has covered the domain, and warning them otherwise would
/// be false. Contrast [`Error`] and [`Quantity`], which are `#[non_exhaustive]`
/// because both grow as operations arrive.
///
/// [failures are classified by remedy]: ../../../../knowledge/principles/failures-are-classified-by-remedy.md
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The inputs were each valid and jointly meaningless, or one was never a
    /// quantity at all. **Change the model.** No wider representation helps.
    Domain,
    /// The arithmetic left what the numbers can hold. **Rescale, or carry more
    /// precision.** The model was fine.
    Representation,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            Self::Domain => "domain",
            Self::Representation => "representation",
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

    /// A ratio was asked for with a zero divisor.
    ///
    /// A domain failure. The ratio of an amount to nothing is undefined rather
    /// than large, so no wider representation answers it.
    #[non_exhaustive]
    ZeroDivisor,

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

impl Error {
    /// Which class of failure this is, and so what would fix it.
    ///
    /// **This lives here rather than in each caller**, and not only to avoid
    /// repeating it. [`Error`] is `#[non_exhaustive]`, so a downstream match
    /// needs a wildcard arm and a variant added later lands silently in
    /// whichever bucket that arm picked. Inside the defining crate the
    /// match below is exhaustive, so a new variant fails to compile until
    /// somebody classifies it — which is the only arrangement where
    /// [failures are classified by remedy]'s claim stays true by
    /// construction rather than by attention.
    ///
    /// **[`Error::NotFinite`] is the one variant that needs its field
    /// consulted**, and the principle predicted exactly that: "an amount
    /// that was never a number and a product that outgrew the range are
    /// different problems with different fixes, and one message served
    /// both." Its answer was that a shared variant must carry enough to say
    /// which case it is, and [`Quantity`] is what this library carried. It
    /// turns out to be sufficient: the quantities a caller supplies are
    /// non-values on arrival, and the two computed ones are values that
    /// outgrew the range.
    ///
    /// The fragility that leaves is worth naming. `Quantity::Amount` classifies
    /// as domain because it can only arise from [`crate::Amount::new`]
    /// rejecting a magnitude somebody passed in. An operation that computed
    /// a magnitude and handed it to that constructor **without** checking
    /// the range first would report a representation failure wearing a
    /// domain label. Today none does — `apply` and `ratio_to` both check
    /// and report `Quantity::Product` — and the tests below pin both
    /// directions.
    ///
    /// [failures are classified by remedy]: ../../../../knowledge/principles/failures-are-classified-by-remedy.md
    #[must_use]
    pub fn kind(&self) -> Kind {
        match *self {
            // Each is unfixable by a wider representation: a sign is a sign, a
            // cancelled difference stays cancelled, a ratio of nothing has no
            // answer, and text that is not a number never becomes one.
            Self::NonPositiveFactor { .. }
            | Self::IndeterminateFactor { .. }
            | Self::NegativeTolerance { .. }
            | Self::ZeroDivisor
            | Self::NegativePeriods { .. }
            | Self::Unparsable { .. } => Kind::Domain,

            // The range, at the top and at the bottom.
            Self::Underflow => Kind::Representation,

            Self::NotFinite { quantity } => match quantity {
                Quantity::Amount | Quantity::Rate | Quantity::Periods | Quantity::Tolerance => {
                    Kind::Domain
                }
                Quantity::Factor | Quantity::Product => Kind::Representation,
            },
        }
    }
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
            Self::ZeroDivisor => f.write_str("ratio has a zero divisor"),
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

#[cfg(test)]
mod tests {
    use super::{Error, Kind, Quantity};

    /// The principle's own test, applied to every variant: would a wider
    /// representation change the answer? Constructed directly rather than
    /// raised by an operation, because what is being pinned is the
    /// classification, and a variant no current operation raises still has
    /// to be classified.
    #[test]
    fn every_variant_is_classified_by_what_would_fix_it() {
        let cases = [
            (Error::NonPositiveFactor { factor: -0.5 }, Kind::Domain),
            (Error::IndeterminateFactor { factor: 1e-17 }, Kind::Domain),
            (Error::NegativeTolerance { tolerance: -1.0 }, Kind::Domain),
            (Error::ZeroDivisor, Kind::Domain),
            (Error::NegativePeriods { periods: -1.0 }, Kind::Domain),
            (Error::Underflow, Kind::Representation),
        ];
        for (error, expected) in cases {
            assert_eq!(error.kind(), expected, "{error:?}");
        }
    }

    /// The variant the principle warned about. One name, two remedies, told
    /// apart only by the field it was made to carry.
    #[test]
    fn not_finite_is_classified_by_the_quantity_it_names() {
        for supplied in [
            Quantity::Amount,
            Quantity::Rate,
            Quantity::Periods,
            Quantity::Tolerance,
        ] {
            let error = Error::NotFinite { quantity: supplied };
            assert_eq!(
                error.kind(),
                Kind::Domain,
                "{supplied} is a value a caller hands in, and a non-value is not \
                 rescued by more bits"
            );
        }
        for computed in [Quantity::Factor, Quantity::Product] {
            let error = Error::NotFinite { quantity: computed };
            assert_eq!(
                error.kind(),
                Kind::Representation,
                "{computed} left a range a wider one would hold"
            );
        }
    }

    #[test]
    fn a_kind_renders_as_the_word_a_message_would_use() {
        assert_eq!(Kind::Domain.to_string(), "domain");
        assert_eq!(Kind::Representation.to_string(), "representation");
    }
}
