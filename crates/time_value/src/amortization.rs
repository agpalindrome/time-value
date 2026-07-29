//! Amortization schedules: the per-period principal / interest / balance
//! breakdown of a repaid loan.
//!
//! A [`Schedule`] is a lazy [`Iterator`] of [`Installment`]s — it holds only
//! scalars (no `Vec`, no allocation) and streams one period at a time, so it is
//! available in the default `no_std` build. Build one from an explicit level
//! payment with [`Schedule::with_payment`] (arithmetic-only), or from a term with
//! [`Schedule::for_term`], which sizes the payment via
//! [`annuity::payment`](crate::annuity::payment) and so needs `std`/`libm`.
//!
//! Each period splits the payment into the interest on the opening balance and
//! the principal that reduces it; the final installment clears whatever remains.

use core::marker::PhantomData;

use crate::money::combine;
use crate::{Currency, Money, Payment, Periodicity, Principal, Rate, TvmError};

/// Relative slack on the "this payment closes the loan" test, so the
/// floating-point residual of a *computed* level payment still lands the final
/// installment exactly on the intended period rather than leaving a vanishing
/// balance for one more.
const FINAL_INSTALLMENT_SLACK: f64 = 1e-9;

/// One period's entry in an amortization [`Schedule`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Installment {
    /// The 1-based period index.
    pub period: u32,
    /// The amount paid this period — the level payment, or the smaller final
    /// installment that clears the balance.
    pub payment: Money,
    /// The portion of `payment` covering interest on the opening balance
    /// (negative if the rate is negative).
    pub interest: Money,
    /// The portion of `payment` that reduces the balance.
    pub principal: Money,
    /// The balance remaining after this payment (zero on the final installment).
    pub balance: Money,
}

/// A lazy amortization schedule: an [`Iterator`] that repays a balance at a fixed
/// per-period `rate` with a level payment, yielding one [`Installment`] per period
/// until the balance is retired.
///
/// It holds only scalars, so iterating allocates nothing. See the
/// [module docs](self) for the two ways to build one.
#[derive(Debug, Clone)]
pub struct Schedule<P: Periodicity> {
    balance: f64,
    rate: f64,
    payment: f64,
    /// The currency every installment is denominated in — the shared currency of
    /// the `principal` and `payment` the schedule was built from (ADR-0034).
    currency: Currency,
    period: u32,
    marker: PhantomData<P>,
}

impl<P: Periodicity> Schedule<P> {
    /// A schedule repaying `principal` at `rate` with a given level `payment`,
    /// running until the balance is retired (the final installment clears the
    /// remainder). Arithmetic-only, so available in the default `no_std` build.
    ///
    /// A non-positive `principal` yields an empty schedule (nothing to repay).
    ///
    /// The two amounts are wrapped in [`Payment`] and [`Principal`] so they cannot
    /// be transposed — a swap that would otherwise compile and silently amortise
    /// the payment with the principal (ADR-0050).
    ///
    /// # Examples
    ///
    /// ```
    /// use time_value::{amortization::Schedule, Money, Monthly, Payment, Principal, Rate};
    ///
    /// // Repay 1000 at 10%/period, paying 500 each period.
    /// let mut schedule = Schedule::with_payment(
    ///     Rate::<Monthly>::new(0.10)?,
    ///     Payment(Money::agnostic(500.0)?),
    ///     Principal(Money::agnostic(1000.0)?),
    /// )?;
    ///
    /// let first = schedule.next().unwrap();
    /// assert_eq!(first.period, 1);
    /// assert!((first.interest.value() - 100.0).abs() < 1e-9); // 1000 × 10%
    /// assert!((first.principal.value() - 400.0).abs() < 1e-9); // 500 − 100
    /// assert!((first.balance.value() - 600.0).abs() < 1e-9);
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    ///
    /// Handing it the payment and the principal the other way round does not
    /// compile:
    ///
    /// ```compile_fail
    /// use time_value::{amortization::Schedule, Money, Monthly, Payment, Principal, Rate};
    ///
    /// let _ = Schedule::with_payment(
    ///     Rate::<Monthly>::new(0.10).unwrap(),
    ///     Principal(Money::agnostic(1000.0).unwrap()), // principal where the payment goes
    ///     Payment(Money::agnostic(500.0).unwrap()),
    /// );
    /// ```
    ///
    /// # Errors
    ///
    /// [`TvmError::Undefined`] if `payment` cannot amortise `principal` — it
    /// does not exceed the first period's interest, so the balance would never
    /// fall and no finite schedule exists (ADR-0027, ADR-0031).
    pub fn with_payment(
        rate: Rate<P>,
        payment: Payment,
        principal: Principal,
    ) -> Result<Self, TvmError> {
        let (payment, principal) = (payment.money(), principal.money());
        // A payment that does not exceed the first period's interest never
        // amortises a positive balance. (A non-positive balance is an empty
        // schedule, handled by `next`.)
        if principal.value() > 0.0 && payment.value() <= principal.value() * rate.value() {
            return Err(TvmError::Undefined);
        }
        Ok(Self {
            balance: principal.value(),
            rate: rate.value(),
            payment: payment.value(),
            currency: combine(principal.currency(), payment.currency())?,
            period: 0,
            marker: PhantomData,
        })
    }
}

/// [`Schedule::for_term`] sizes the payment with [`annuity::payment`](crate::annuity::payment),
/// so it needs `powf` and is behind `std` / `libm` (ADR-0027).
#[cfg(any(feature = "std", feature = "libm"))]
impl<P: Periodicity> Schedule<P> {
    /// A schedule repaying `principal` at `rate` over `periods` periods, with the
    /// level payment computed by [`annuity::payment`](crate::annuity::payment). The
    /// final installment lands on period `periods`, clearing the balance.
    ///
    /// # Examples
    ///
    /// ```
    /// use time_value::{amortization::Schedule, Money, Monthly, Period, Rate};
    ///
    /// // A 1125.508 loan at 1%/month over a year: ~100/month, 12 installments.
    /// let schedule = Schedule::for_term(
    ///     Rate::<Monthly>::new(0.01)?,
    ///     Period::new(12.0)?,
    ///     Money::agnostic(1125.508)?,
    /// )?;
    /// assert_eq!(schedule.count(), 12);
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// As [`annuity::payment`](crate::annuity::payment) — [`TvmError::Undefined`]
    /// if `periods` is zero (nothing to amortise over).
    pub fn for_term(
        rate: Rate<P>,
        periods: crate::Period<P>,
        principal: Money,
    ) -> Result<Self, TvmError> {
        let payment = crate::annuity::payment(rate, periods, principal)?;
        Self::with_payment(rate, Payment(payment), Principal(principal))
    }
}

impl<P: Periodicity> Iterator for Schedule<P> {
    type Item = Installment;

    fn next(&mut self) -> Option<Self::Item> {
        if self.balance <= 0.0 {
            return None;
        }
        let interest = self.balance * self.rate;
        // `due` is what it takes to close the loan this period. The final
        // installment lands once the level payment covers it — within a hair, to
        // absorb the floating-point residual of a computed level payment.
        let due = self.balance + interest;
        let closes = due <= self.payment * (1.0 + FINAL_INSTALLMENT_SLACK);
        let (payment, principal, balance) = if closes {
            (due, self.balance, 0.0)
        } else {
            let principal = self.payment - interest;
            (self.payment, principal, self.balance - principal)
        };
        self.period += 1;
        self.balance = balance;
        Some(Installment {
            period: self.period,
            payment: Money::from_operation(payment, self.currency).ok()?,
            interest: Money::from_operation(interest, self.currency).ok()?,
            principal: Money::from_operation(principal, self.currency).ok()?,
            balance: Money::from_operation(balance, self.currency).ok()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Schedule;
    use crate::{Money, Monthly, Payment, Principal, Rate, TvmError};

    fn approx(a: f64, b: f64, tolerance: f64) -> bool {
        let d = a - b;
        d < tolerance && d > -tolerance
    }

    fn rate(r: f64) -> Rate<Monthly> {
        Rate::<Monthly>::new(r).unwrap()
    }

    #[test]
    fn splits_each_payment_into_interest_and_principal() {
        // 1000 at 10%, paying 500: 400/600, then 440/160, then a 176 stub.
        let mut schedule = Schedule::with_payment(
            rate(0.10),
            Payment(Money::agnostic(500.0).unwrap()),
            Principal(Money::agnostic(1000.0).unwrap()),
        )
        .unwrap();

        let first = schedule.next().unwrap();
        assert_eq!(first.period, 1);
        assert!(approx(first.interest.value(), 100.0, 1e-9));
        assert!(approx(first.principal.value(), 400.0, 1e-9));
        assert!(approx(first.balance.value(), 600.0, 1e-9));

        let second = schedule.next().unwrap();
        assert_eq!(second.period, 2);
        assert!(approx(second.interest.value(), 60.0, 1e-9));
        assert!(approx(second.principal.value(), 440.0, 1e-9));
        assert!(approx(second.balance.value(), 160.0, 1e-9));

        let last = schedule.next().unwrap();
        assert_eq!(last.period, 3);
        assert!(approx(last.interest.value(), 16.0, 1e-9));
        assert!(approx(last.payment.value(), 176.0, 1e-9)); // the stub clears it
        assert!(approx(last.balance.value(), 0.0, 1e-9));

        assert!(schedule.next().is_none());
    }

    #[test]
    fn interest_plus_principal_equals_each_payment() {
        let schedule = Schedule::with_payment(
            rate(0.05),
            Payment(Money::agnostic(300.0).unwrap()),
            Principal(Money::agnostic(2000.0).unwrap()),
        )
        .unwrap();
        for installment in schedule {
            assert!(approx(
                installment.interest.value() + installment.principal.value(),
                installment.payment.value(),
                1e-9,
            ));
        }
    }

    #[test]
    fn a_payment_below_the_interest_never_amortises() {
        // 10% on 1000 is 100/period; a 100 (or less) payment never reduces it.
        assert_eq!(
            Schedule::with_payment(
                rate(0.10),
                Payment(Money::agnostic(100.0).unwrap()),
                Principal(Money::agnostic(1000.0).unwrap())
            )
            .map(Schedule::count),
            Err(TvmError::Undefined),
        );
    }

    #[test]
    fn a_non_positive_principal_is_an_empty_schedule() {
        let mut schedule = Schedule::with_payment(
            rate(0.10),
            Payment(Money::agnostic(100.0).unwrap()),
            Principal(Money::ZERO),
        )
        .unwrap();
        assert!(schedule.next().is_none());
    }

    #[cfg(any(feature = "std", feature = "libm"))]
    mod for_term {
        use super::{approx, rate};
        use crate::{amortization::Schedule, Money, Period};

        #[test]
        fn runs_exactly_the_term_and_clears_the_balance() {
            let principal = Money::agnostic(1125.508).unwrap();
            let schedule =
                Schedule::for_term(rate(0.01), Period::new(12.0).unwrap(), principal).unwrap();

            let mut count = 0u32;
            let mut principal_repaid = 0.0;
            let mut final_balance = f64::NAN;
            for installment in schedule {
                count += 1;
                principal_repaid += installment.principal.value();
                final_balance = installment.balance.value();
            }
            assert_eq!(count, 12);
            // The principal portions sum back to the original balance...
            assert!(approx(principal_repaid, principal.value(), 1e-6));
            // ...and the loan is fully repaid at the end.
            assert!(approx(final_balance, 0.0, 1e-6));
        }

        #[test]
        fn zero_term_is_degenerate() {
            assert_eq!(
                Schedule::for_term(rate(0.01), Period::ZERO, Money::agnostic(1000.0).unwrap())
                    .map(Schedule::count),
                Err(crate::TvmError::Undefined),
            );
        }
    }
}
