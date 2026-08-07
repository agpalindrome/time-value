//! The arithmetic of simple interest, over `f64` directly.
//!
//! Private. What the typed layer adds is validation at construction, so an
//! invalid value cannot be built at all; this still has to check, because an
//! `f64` carries no invariant.

use crate::error::{Error, Quantity, Result};

/// How many rounding steps from zero a factor must stand before its sign is
/// believed.
///
/// `1 + rt` is a difference, so near `rt = -1` the leading digits cancel and
/// the residue is dominated by the error already present in `rate` and
/// `periods`. Two steps is enough to cover one rounding in each operand; the
/// exact constant matters less than that there is one.
const CANCELLATION_STEPS: f64 = 2.0;

/// The accumulation factor `1 + rt`.
///
/// Computed with a fused multiply-add, which is **load-bearing rather than
/// stylistic**. `periods.mul_add(rate, 1.0)` rounds once; `1.0 + periods *
/// rate` rounds twice, and the intermediate rounding can drive a product that
/// is only near `-1` to exactly `-1`, losing the sign of the residue entirely.
/// The two forms disagree about which inputs are valid, and the fused one
/// agrees with the exact sign of `1 + rt`.
///
/// # Errors
///
/// [`Error::NonPositiveFactor`] when the factor is not strictly positive, and
/// [`Error::IndeterminateFactor`] when it is too near zero for its sign to
/// carry meaning — both domain failures, where `rate` and `periods` are each
/// valid and jointly are not.
///
/// [`Error::NotFinite`] when the factor overflows, which is a representation
/// failure. Note an infinity of the *negative* sign is reported as a domain
/// failure instead: its sign is known, so it is a non-positive factor rather
/// than a value out of range, and no wider representation would rescue it.
pub(crate) fn accumulation_factor(rate: f64, periods: f64) -> Result<f64> {
    let product = periods * rate;
    let factor = periods.mul_add(rate, 1.0);

    // Sign before range. A factor of `-inf` is non-positive, which is knowable
    // from the operands alone and is not fixed by a wider representation, so
    // reporting it as an overflow would send a caller to the wrong remedy.
    if factor <= 0.0 {
        return Err(Error::NonPositiveFactor { factor });
    }
    if !factor.is_finite() {
        return Err(Error::NotFinite {
            quantity: Quantity::Factor,
        });
    }
    if factor <= CANCELLATION_STEPS * f64::EPSILON * product.abs() {
        return Err(Error::IndeterminateFactor { factor });
    }
    Ok(factor)
}

#[cfg(test)]
mod tests {
    use super::accumulation_factor;
    use crate::error::{Error, Quantity};

    /// Comparison to within a tolerance: absolute near zero, relative at scale,
    /// and never standing in for equality.
    fn close(a: f64, b: f64, absolute: f64, relative: f64) -> bool {
        (a - b).abs() <= absolute.max(relative * a.abs().max(b.abs()))
    }

    const ABS: f64 = 1e-12;
    const REL: f64 = 1e-12;

    #[test]
    fn accumulates_over_whole_periods() {
        let factor = accumulation_factor(0.05, 3.0).expect("0.05 over 3 periods should be valid");
        assert!(close(factor, 1.15, ABS, REL), "expected 1.15, got {factor}");
    }

    #[test]
    fn accumulates_over_a_fractional_period() {
        // Fractional spans are the ordinary case for simple interest, not an
        // edge one: a 91/360 money-market span is the shape of the domain.
        let factor = accumulation_factor(0.05, 91.0 / 360.0).expect("a partial period is valid");
        assert!(
            close(factor, 1.012_638_888_888_888_9, ABS, REL),
            "got {factor}"
        );
    }

    #[test]
    fn is_exactly_one_at_zero_periods() {
        // a(0) = 1 is a stated criterion, not an incidental result.
        let factor = accumulation_factor(0.05, 0.0).expect("zero periods is in the domain");
        assert!(
            (factor - 1.0).abs() < f64::EPSILON,
            "a(0) must be exactly 1, got {factor}"
        );
    }

    #[test]
    fn a_negative_rate_shrinks_rather_than_invalidates() {
        let factor = accumulation_factor(-0.02, 10.0).expect("-2% over 10 periods stays positive");
        assert!(close(factor, 0.8, ABS, REL), "got {factor}");
        assert!(factor > 0.0 && factor < 1.0);
    }

    #[test]
    fn rejects_a_factor_that_would_flip_the_sign() {
        let error = accumulation_factor(-0.5, 3.0).expect_err("-0.5 is not a valid factor");
        assert!(
            matches!(error, Error::NonPositiveFactor { .. }),
            "expected a domain failure, got {error:?}"
        );
    }

    #[test]
    fn rejects_a_factor_of_exactly_zero() {
        let error = accumulation_factor(-0.5, 2.0).expect_err("zero is not strictly positive");
        assert!(matches!(error, Error::NonPositiveFactor { factor } if factor == 0.0));
    }

    #[test]
    fn rejects_a_factor_whose_sign_is_only_rounding_error() {
        // -1/12 over 12 periods is exactly zero in real arithmetic. In binary
        // floating point the stored rate is not exactly -1/12, so `1 + rt`
        // survives as ~5.55e-17 — positive, meaningless, and previously
        // accepted, which turned a million into a fraction of a nanocent.
        for (rate, periods) in [
            (-1.0 / 12.0, 12.0),
            (-1.0 / 3.0, 3.0),
            (-1.0 / 7.0, 7.0),
            (-1.0 / 49.0, 49.0),
        ] {
            let error = accumulation_factor(rate, periods)
                .expect_err("a factor built from cancellation is refused");
            assert!(
                matches!(error, Error::IndeterminateFactor { .. }),
                "{rate} over {periods}: got {error:?}"
            );
        }
    }

    #[test]
    fn the_two_spellings_of_one_rate_now_agree() {
        // Previously `from_fraction`'s spelling was accepted and
        // `from_percent`'s rejected, one ulp apart, for the same intent.
        let as_fraction = accumulation_factor(-1.0 / 12.0, 12.0);
        let as_percent = accumulation_factor(-100.0 / 12.0 / 100.0, 12.0);
        assert!(as_fraction.is_err() && as_percent.is_err());
    }

    #[test]
    fn ordinary_small_factors_are_still_accepted() {
        // The guard must not swallow legitimately small factors. 1 + rt here is
        // 0.001, which is tiny but carries real digits.
        let factor = accumulation_factor(-0.999, 1.0).expect("0.001 is a real factor");
        assert!(close(factor, 0.001, ABS, REL), "got {factor}");
    }

    #[test]
    fn a_negative_overflow_is_a_domain_failure_not_a_range_one() {
        // The true factor is about -1e310. Its sign is knowable from the
        // operands, and no wider representation makes it positive, so calling
        // it a range failure would send a caller to the wrong remedy.
        let error = accumulation_factor(-1e300, 1e10).expect_err("-1e310 is not a factor");
        assert!(
            matches!(error, Error::NonPositiveFactor { .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn a_positive_overflow_is_a_range_failure() {
        let error = accumulation_factor(1e300, 1e10).expect_err("1e310 does not fit");
        assert!(
            matches!(
                error,
                Error::NotFinite {
                    quantity: Quantity::Factor
                }
            ),
            "got {error:?}"
        );
    }

    #[test]
    fn the_fused_form_is_load_bearing() {
        // Pins the choice of `mul_add` over `1.0 + periods * rate`. The unfused
        // form rounds the product of -1/7 and 7 to exactly -1.0 and reports a
        // factor of 0; the fused form keeps the residue. Swapping the
        // implementation without this test passes every other check.
        let (rate, periods): (f64, f64) = (-1.0 / 7.0, 7.0);
        let unfused = 1.0 + periods * rate;
        let fused = periods.mul_add(rate, 1.0);
        assert!(
            unfused == 0.0 && fused > 0.0,
            "the two forms should differ here: unfused {unfused}, fused {fused}"
        );
        // Both are refused, but for different reasons — and only the fused form
        // can tell "cancelled to nothing" from "genuinely zero".
        let error = accumulation_factor(rate, periods).expect_err("cancellation");
        assert!(
            matches!(error, Error::IndeterminateFactor { .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn rejects_non_finite_inputs() {
        for (rate, periods) in [(f64::NAN, 1.0), (f64::INFINITY, 1.0), (0.05, f64::NAN)] {
            accumulation_factor(rate, periods).expect_err("non-finite is not a factor");
        }
    }
}
