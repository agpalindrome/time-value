//! How far apart two amounts may be and still count as close.

use crate::error::{Error, Quantity, Result};

/// A tolerance for approximate comparison.
///
/// Both halves are needed and neither is defaulted. The **absolute** term
/// carries the comparison near zero, where a relative tolerance always rejects
/// — the relative error against zero is 1 whatever the magnitudes. The
/// **relative** term carries it at scale, where an absolute tolerance falls
/// below a single representable step and silently becomes exact equality.
///
/// It exists as a type so the two can never be transposed. They are both
/// `f64`, they are not interchangeable, and a swap changes the answer without
/// failing — the same argument that gives a
/// [rate](crate::SimpleInterestRate) two named constructors rather than one.
///
/// # Examples
///
/// ```
/// use time_value::Tolerance;
///
/// // Each call names the value it takes, so there is no order to get wrong.
/// let both = Tolerance::absolute(1e-12)?.and_relative(1e-9)?;
/// let near_zero_only = Tolerance::absolute(1e-12)?;
/// let at_scale_only = Tolerance::relative(1e-9)?;
///
/// // A tolerance is validated like any other quantity here.
/// assert!(Tolerance::absolute(f64::NAN).is_err());
/// assert!(Tolerance::relative(-1.0).is_err());
/// # Ok::<(), time_value::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    absolute: f64,
    relative: f64,
}

/// Tolerances are never NaN.
impl Eq for Tolerance {}

fn checked(value: f64) -> Result<f64> {
    if !value.is_finite() {
        return Err(Error::NotFinite {
            quantity: Quantity::Tolerance,
        });
    }
    if value < 0.0 {
        return Err(Error::NegativeTolerance { tolerance: value });
    }
    Ok(value)
}

impl Tolerance {
    /// A tolerance measured in absolute difference, with no relative term.
    ///
    /// # Errors
    ///
    /// [`Error::NotFinite`] if `value` is a NaN or an infinity — a NaN would be
    /// silently discarded by the comparison and answer under a tolerance the
    /// caller never gave. [`Error::NegativeTolerance`] if it is below zero,
    /// which costs the comparison its reflexivity.
    pub fn absolute(value: f64) -> Result<Self> {
        Ok(Self {
            absolute: checked(value)?,
            relative: 0.0,
        })
    }

    /// A tolerance measured relative to the larger magnitude, with no absolute
    /// term.
    ///
    /// # Errors
    ///
    /// As [`Tolerance::absolute`].
    pub fn relative(value: f64) -> Result<Self> {
        Ok(Self {
            absolute: 0.0,
            relative: checked(value)?,
        })
    }

    /// Adds a relative term.
    ///
    /// # Errors
    ///
    /// As [`Tolerance::absolute`].
    pub fn and_relative(self, value: f64) -> Result<Self> {
        Ok(Self {
            relative: checked(value)?,
            ..self
        })
    }

    /// Adds an absolute term.
    ///
    /// # Errors
    ///
    /// As [`Tolerance::absolute`].
    pub fn and_absolute(self, value: f64) -> Result<Self> {
        Ok(Self {
            absolute: checked(value)?,
            ..self
        })
    }

    /// Whether a difference is within this tolerance, given the scale the
    /// relative term is measured against.
    pub(crate) fn permits(self, difference: f64, scale: f64) -> bool {
        // A difference that overflows means the two values are at least the
        // width of the representable range apart. Without this guard both sides
        // of the comparison can reach infinity and `inf <= inf` reports them as
        // close.
        if !difference.is_finite() {
            return false;
        }
        difference <= self.absolute.max(self.relative * scale)
    }
}

#[cfg(test)]
mod tests {
    use super::Tolerance;
    use crate::error::{Error, Quantity};

    #[test]
    fn refuses_a_non_finite_tolerance() {
        // A NaN tolerance would be swallowed by `f64::max`, which returns the
        // non-NaN operand — so the comparison would answer confidently under a
        // tolerance the caller never supplied.
        for value in [f64::NAN, f64::INFINITY] {
            assert!(matches!(
                Tolerance::absolute(value),
                Err(Error::NotFinite {
                    quantity: Quantity::Tolerance
                })
            ));
            Tolerance::relative(value).expect_err("non-finite is refused");
        }
    }

    #[test]
    fn refuses_a_negative_tolerance() {
        // A negative tolerance costs reflexivity: a value stops being close to
        // itself.
        let error = Tolerance::absolute(-1e-9).expect_err("negative is refused");
        assert!(
            matches!(error, Error::NegativeTolerance { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn refuses_a_bad_term_added_later() {
        let base = Tolerance::absolute(1e-12).expect("valid");
        base.and_relative(f64::NAN).expect_err("NaN is refused");
        base.and_relative(-1.0).expect_err("negative is refused");
    }

    #[test]
    fn a_difference_that_overflows_is_never_close() {
        let tolerance = Tolerance::relative(1.5).expect("valid");
        assert!(!tolerance.permits(f64::INFINITY, f64::MAX));
    }

    #[test]
    fn zero_is_a_legitimate_tolerance() {
        // It means exact, which is a coherent thing to ask for.
        let tolerance = Tolerance::absolute(0.0).expect("zero is valid");
        assert!(tolerance.permits(0.0, 1.0));
        assert!(!tolerance.permits(1e-300, 1.0));
    }
}
