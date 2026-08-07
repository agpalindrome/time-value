//! Future value under simple interest, over `f64` directly.
//!
//! This is the first of two steps. The behaviour is settled here, in tests,
//! before any type exists to enforce it — so the types that follow are
//! answering a question already asked rather than defining it.
//!
//! The functions here are the arithmetic. What the typed layer adds is
//! validation at construction, so an invalid value cannot be built at all;
//! these still have to check, because an `f64` carries no invariant.

use crate::error::{Error, Result};

/// The accumulation factor `1 + rt`.
///
/// # Errors
///
/// [`Error::NonPositiveFactor`] when `1 + rt` is not strictly positive — the
/// domain failure, where `rate` and `periods` are each valid and jointly
/// meaningless. [`Error::NotFinite`] when the factor leaves the representable
/// range, which is a different problem with a different remedy.
pub fn accumulation_factor(rate: f64, periods: f64) -> Result<f64> {
    let factor = periods.mul_add(rate, 1.0);
    if !factor.is_finite() {
        return Err(Error::NotFinite);
    }
    if factor <= 0.0 {
        return Err(Error::NonPositiveFactor { factor });
    }
    Ok(factor)
}

/// `FV = PV(1 + rt)`.
///
/// # Errors
///
/// Whatever [`accumulation_factor`] reports, or [`Error::NotFinite`] when the
/// product itself leaves the representable range — the factor can be perfectly
/// valid and the result still too large to hold.
pub fn future_value(present_value: f64, rate: f64, periods: f64) -> Result<f64> {
    let factor = accumulation_factor(rate, periods)?;
    let future = present_value * factor;
    if future.is_finite() {
        Ok(future)
    } else {
        Err(Error::NotFinite)
    }
}

#[cfg(test)]
mod tests {
    use super::{accumulation_factor, future_value};
    use crate::error::Error;

    /// Comparison to within a tolerance, as the concept requires: absolute near
    /// zero, relative at scale, and never standing in for equality.
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
    fn a_zero_rate_leaves_the_amount_alone() {
        let future = future_value(100.0, 0.0, 7.0).expect("a zero rate is in the domain");
        assert!(close(future, 100.0, ABS, REL), "got {future}");
    }

    #[test]
    fn a_negative_rate_shrinks_the_amount() {
        // Negative rates are real; the factor lies between 0 and 1 and the
        // amount shrinks. That is not the same as the factor being invalid.
        let factor = accumulation_factor(-0.02, 10.0).expect("-2% over 10 periods stays positive");
        assert!(close(factor, 0.8, ABS, REL), "got {factor}");
        assert!(factor > 0.0 && factor < 1.0);
    }

    #[test]
    fn rejects_a_factor_that_would_flip_the_sign() {
        // -50% over three periods gives 1 + rt = -0.5. The formula would happily
        // return a number; it is meaningless, so it is refused — and refused as
        // a domain failure, which is not the same as running out of range.
        let error = accumulation_factor(-0.5, 3.0).expect_err("-0.5 is not a valid factor");
        assert!(
            matches!(error, Error::NonPositiveFactor { .. }),
            "expected a domain failure, got {error:?}"
        );
    }

    #[test]
    fn rejects_a_factor_of_exactly_zero() {
        // The criterion is strictly positive, so the boundary is excluded: an
        // amount annihilated by elapsed time is not an accumulation.
        let error = accumulation_factor(-0.5, 2.0).expect_err("zero is not strictly positive");
        assert!(matches!(error, Error::NonPositiveFactor { factor } if factor == 0.0));
    }

    #[test]
    fn a_negative_present_value_stays_negative() {
        // FV inherits its sign from PV. A liability accumulates into a larger
        // liability; a positive factor never turns one into an asset.
        let future = future_value(-100.0, 0.05, 3.0).expect("a liability is a valid amount");
        assert!(close(future, -115.0, ABS, REL), "got {future}");
    }

    #[test]
    fn a_zero_present_value_is_the_fixed_point() {
        let future = future_value(0.0, 0.05, 3.0).expect("zero is a valid amount");
        assert!(future.abs() < f64::EPSILON, "got {future}");
    }

    #[test]
    fn rejects_non_finite_inputs() {
        for (rate, periods) in [(f64::NAN, 1.0), (f64::INFINITY, 1.0), (0.05, f64::NAN)] {
            let error = accumulation_factor(rate, periods).expect_err("non-finite is not a factor");
            assert!(matches!(error, Error::NotFinite), "got {error:?}");
        }
        assert!(matches!(
            future_value(f64::NAN, 0.05, 1.0),
            Err(Error::NotFinite)
        ));
    }

    #[test]
    fn tells_the_two_failures_apart() {
        // The whole reason the operation is split. Both inputs finite and the
        // factor valid; only the product leaves the range — so this is a
        // representation failure, reported differently from the domain one.
        accumulation_factor(1.0, 1.0).expect("1 + 1*1 = 2 is a valid factor");
        let error = future_value(f64::MAX, 1.0, 1.0).expect_err("2 * f64::MAX overflows");
        assert!(matches!(error, Error::NotFinite), "got {error:?}");
    }

    #[test]
    fn recovers_the_factor_by_dividing() {
        // FV/PV is the accumulation factor arrived at from the other direction,
        // which is why the factor is an object rather than an expression.
        let (present, rate, periods) = (250.0, 0.04, 2.5);
        let future = future_value(present, rate, periods).expect("valid");
        let recovered = future / present;
        let direct = accumulation_factor(rate, periods).expect("valid");
        assert!(
            close(recovered, direct, ABS, REL),
            "{recovered} vs {direct}"
        );
    }
}
