//! Amortization schedules: the per-period principal / interest / balance
//! breakdown of a repaid loan.
//!
//! A [`Schedule`] is a lazy [`Iterator`] of [`Installment`]s — it holds only
//! scalars (no `Vec`, no allocation) and streams one period at a time, so it is
//! available in the default `no_std` build. Build one from an explicit level
//! payment with [`Schedule::with_payment`] (arithmetic-only), or from a term with
//! [`Schedule::for_term`], which sizes the payment via
//! [`annuity::payment`][crate::annuity::payment] and so needs `std`/`libm`.
//!
//! Each period splits the payment into the interest on the opening balance and
//! the principal that reduces it; the final installment clears whatever remains.
// `for_term` and `annuity::payment` are gated, so in the default `no_std` build
// these links have no local target; define them as their docs.rs URLs instead of
// warning (ADR-0055).
#![cfg_attr(
    not(any(feature = "std", feature = "libm")),
    doc = "

[`Schedule::for_term`]: https://docs.rs/time_value/latest/time_value/amortization/struct.Schedule.html#method.for_term
[crate::annuity::payment]: https://docs.rs/time_value/latest/time_value/annuity/fn.payment.html
"
)]

use core::marker::PhantomData;

use crate::money::combine;
use crate::{Currency, Money, Payment, Periodicity, Principal, Rate, TvmError};

/// Relative slack on the "this payment closes the loan" test, so the
/// floating-point residual of a *computed* level payment still lands the final
/// installment exactly on the intended period rather than leaving a vanishing
/// balance for one more.
const FINAL_INSTALLMENT_SLACK: f64 = 1e-9;

/// One period's entry in an amortization [`Schedule`], read through its
/// accessors.
///
/// The fields are **not public** (ADR-0051): an `Installment` is *yielded* by an
/// iterator, so public fields would let downstream code construct it by literal
/// and match it exhaustively, freezing the field set for the life of the major
/// version. Reading goes through [`period`](Self::period),
/// [`payment`](Self::payment), [`interest`](Self::interest),
/// [`principal`](Self::principal) and [`balance`](Self::balance), which leaves
/// room to add a row later.
///
/// ```
/// use time_value::{amortization::Schedule, Money, Monthly, Payment, Principal, Rate};
///
/// let first = Schedule::with_payment(
///     Rate::<Monthly>::new(0.10)?,
///     Payment(Money::agnostic(500.0)?),
///     Principal(Money::agnostic(1000.0)?),
/// )?
/// .next()
/// .unwrap();
///
/// assert_eq!(first.period(), 1);
/// assert_eq!(first.interest().value() + first.principal().value(), first.payment().value());
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// Building one by struct literal does not compile:
///
/// ```compile_fail
/// use time_value::{Installment, Money};
///
/// let _ = Installment {
///     period: 1,
///     payment: Money::agnostic(500.0).unwrap(),
///     interest: Money::agnostic(100.0).unwrap(),
///     principal: Money::agnostic(400.0).unwrap(),
///     balance: Money::agnostic(600.0).unwrap(),
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Installment {
    // `pub(crate)`, not module-private: `serde_impls` builds one field-by-field
    // from the wire form. A `pub(crate) fn new` would take four positional
    // `Money`s — the transposition ADR-0050 exists to prevent — so the two
    // in-crate construction sites name their fields instead (ADR-0051).
    pub(crate) period: u32,
    pub(crate) payment: Money,
    pub(crate) interest: Money,
    pub(crate) principal: Money,
    pub(crate) balance: Money,
}

impl Installment {
    /// The 1-based period index.
    #[must_use]
    pub const fn period(self) -> u32 {
        self.period
    }

    /// The amount paid this period — the level payment, or the smaller final
    /// installment that clears the balance.
    #[must_use]
    pub const fn payment(self) -> Money {
        self.payment
    }

    /// The portion of the payment covering interest on the opening balance
    /// (negative if the rate is negative).
    #[must_use]
    pub const fn interest(self) -> Money {
        self.interest
    }

    /// The portion of the payment that reduces the balance.
    #[must_use]
    pub const fn principal(self) -> Money {
        self.principal
    }

    /// The balance remaining after this payment (zero on the final installment).
    #[must_use]
    pub const fn balance(self) -> Money {
        self.balance
    }
}

/// A lazy amortization schedule: an [`Iterator`] that repays a balance at a fixed
/// per-period `rate` with a level payment, yielding one [`Installment`] per period
/// until the balance is retired.
///
/// It holds only scalars, so iterating allocates nothing. See the
/// [module docs](self) for the two ways to build one.
///
/// # It always terminates
///
/// The iterator ends when the balance reaches zero — and also, unconditionally,
/// as soon as a period **fails to reduce the balance**. That second guard is what
/// makes "until the balance is retired" a promise rather than a hope: once the
/// per-period principal reduction falls below the ULP of the balance,
/// `balance − principal == balance` in `f64` and the loan stops moving, so
/// without it the iterator would yield forever (ADR-0054). Two shapes hit it —
/// a payment that only just exceeds the first period's interest, which
/// [`with_payment`](Self::with_payment) rejects up front as
/// [`PaymentDoesNotAmortize`](TvmError::PaymentDoesNotAmortize); and a schedule
/// that converges to a positive balance *asymptotically* (a negative rate), which
/// runs healthily for a while and stalls only later, so it can only be caught
/// here.
///
/// The `u32` period counter is likewise checked: a schedule that somehow reached
/// period `2³² − 1` ends there rather than wrapping.
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
    /// assert_eq!(first.period(), 1);
    /// assert!((first.interest().value() - 100.0).abs() < 1e-9); // 1000 × 10%
    /// assert!((first.principal().value() - 400.0).abs() < 1e-9); // 500 − 100
    /// assert!((first.balance().value() - 600.0).abs() < 1e-9);
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
    /// - [`TvmError::PaymentDoesNotAmortize`] if `payment` cannot amortise
    ///   `principal` — the first period would not reduce the balance, so no finite
    ///   schedule exists (ADR-0027, ADR-0031, ADR-0052, ADR-0054). That covers the
    ///   arithmetic case [`annuity::periods`][crate::annuity::periods] rejects, a
    ///   payment not exceeding the first period's interest (`PMT ≤ PV·r`), *and*
    ///   the floating-point one it does not: a payment that exceeds the interest by
    ///   so little that the reduction is smaller than the ULP of the balance, so
    ///   `balance − principal == balance` and the balance never falls. The check is
    ///   literally the iterator's own step, run once — the constructor and
    ///   [`next`](Iterator::next) cannot disagree about what "amortises" means.
    /// - [`TvmError::CurrencyMismatch`] if `payment` and `principal` are in
    ///   distinct non-`Xxx` currencies, so the schedule has no single
    ///   denomination (ADR-0034).
    #[cfg_attr(
        not(any(feature = "std", feature = "libm")),
        doc = "

[crate::annuity::periods]: https://docs.rs/time_value/latest/time_value/annuity/fn.periods.html
"
    )]
    pub fn with_payment(
        rate: Rate<P>,
        payment: Payment,
        principal: Principal,
    ) -> Result<Self, TvmError> {
        let (payment, principal) = (payment.money(), principal.money());
        let schedule = Self {
            balance: principal.value(),
            rate: rate.value(),
            payment: payment.value(),
            currency: combine(principal.currency(), payment.currency())?,
            period: 0,
            marker: PhantomData,
        };
        // Reject an unamortizable loan loudly here rather than handing back an
        // empty schedule (ADR-0054). A non-positive balance is a different thing —
        // there is nothing to repay, which is an empty schedule, not an error.
        if schedule.balance > 0.0 && schedule.step().is_none() {
            return Err(TvmError::PaymentDoesNotAmortize);
        }
        Ok(schedule)
    }

    /// This period's split of the payment, and the balance it leaves — or `None`
    /// if the period does not **strictly** reduce the balance.
    ///
    /// The single definition of "a period that amortises", shared by
    /// [`with_payment`](Self::with_payment)'s up-front check and
    /// [`next`](Iterator::next)'s per-period one. A non-reducing period is fatal
    /// rather than merely unproductive: nothing about the schedule's state changes
    /// across it, so every period after it is identical and the iterator would
    /// never end. `None` also absorbs a `NaN` balance, which no comparison would
    /// otherwise catch.
    fn step(&self) -> Option<Step> {
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
        if balance.is_nan() || balance >= self.balance {
            return None;
        }
        Some(Step {
            payment,
            interest,
            principal,
            balance,
        })
    }
}

/// One period's arithmetic, before it is denominated: the split of the payment and
/// the balance left behind. The `f64` companion to [`Installment`], which
/// [`Schedule::step`] produces and [`Schedule::next`](Iterator::next) turns into
/// [`Money`] — named fields rather than a tuple so the four magnitudes cannot be
/// transposed on the way through (ADR-0050).
struct Step {
    payment: f64,
    interest: f64,
    principal: f64,
    balance: f64,
}

/// [`Schedule::for_term`] sizes the payment with [`annuity::payment`](crate::annuity::payment),
/// so it needs `powf` and is behind `std` / `libm` (ADR-0027).
#[cfg(any(feature = "std", feature = "libm"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "libm"))))]
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
    /// As [`annuity::payment`](crate::annuity::payment) — [`TvmError::ZeroPeriods`]
    /// if `periods` is zero (nothing to amortise over) — then as
    /// [`with_payment`](Self::with_payment) for the sized payment, which can add
    /// [`TvmError::CurrencyMismatch`] (ADR-0052) and
    /// [`TvmError::PaymentDoesNotAmortize`] (ADR-0054). The latter is reachable
    /// here on extreme rate/term pairs: at a high enough rate over a long enough
    /// term the level payment converges on the first period's interest, and once
    /// the difference is below the ULP of the principal (`0.2` over `200` periods
    /// is already there) no `f64` schedule retires the balance.
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
        // Both `?`s end the schedule: a period that cannot reduce the balance
        // would repeat forever, and a period index past `u32::MAX` cannot be
        // reported (ADR-0054). Neither can happen to a healthy loan.
        let step = self.step()?;
        let period = self.period.checked_add(1)?;
        self.period = period;
        self.balance = step.balance;
        Some(Installment {
            period,
            payment: Money::from_operation(step.payment, self.currency).ok()?,
            interest: Money::from_operation(step.interest, self.currency).ok()?,
            principal: Money::from_operation(step.principal, self.currency).ok()?,
            balance: Money::from_operation(step.balance, self.currency).ok()?,
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
        assert_eq!(first.period(), 1);
        assert!(approx(first.interest().value(), 100.0, 1e-9));
        assert!(approx(first.principal().value(), 400.0, 1e-9));
        assert!(approx(first.balance().value(), 600.0, 1e-9));

        let second = schedule.next().unwrap();
        assert_eq!(second.period(), 2);
        assert!(approx(second.interest().value(), 60.0, 1e-9));
        assert!(approx(second.principal().value(), 440.0, 1e-9));
        assert!(approx(second.balance().value(), 160.0, 1e-9));

        let last = schedule.next().unwrap();
        assert_eq!(last.period(), 3);
        assert!(approx(last.interest().value(), 16.0, 1e-9));
        assert!(approx(last.payment().value(), 176.0, 1e-9)); // the stub clears it
        assert!(approx(last.balance().value(), 0.0, 1e-9));

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
                installment.interest().value() + installment.principal().value(),
                installment.payment().value(),
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
            Err(TvmError::PaymentDoesNotAmortize),
        );
    }

    /// `with_payment` folds the two amounts' currencies together, so its
    /// `# Errors` names `CurrencyMismatch`; pin it (ADR-0045 rule 2, ADR-0052).
    #[test]
    fn a_payment_and_principal_in_distinct_currencies_are_a_mismatch() {
        use crate::Currency;
        assert_eq!(
            Schedule::with_payment(
                rate(0.10),
                Payment(Money::new(500.0, Currency::Eur).unwrap()),
                Principal(Money::new(1000.0, Currency::Usd).unwrap()),
            )
            .map(Schedule::count),
            // `combine(principal, payment)`, in that order.
            Err(TvmError::CurrencyMismatch {
                left: Currency::Usd,
                right: Currency::Eur,
            }),
        );
    }

    /// The other side of `with_payment`'s fold: ADR-0034's identity rule, so an
    /// agnostic payment against a denominated principal (or the reverse) yields a
    /// schedule denominated in the one currency named — and every row of it is,
    /// since `next` stamps the folded currency on all four amounts. Exhaustive over
    /// the closed currency set; the `Xxx` iteration is the all-agnostic case.
    #[test]
    fn a_schedule_adopts_the_one_currency_its_inputs_name() {
        use crate::Currency;
        for &currency in Currency::ALL {
            for (payment, principal) in [
                // Agnostic payment, denominated principal — and the reverse.
                (
                    Money::agnostic(500.0).unwrap(),
                    Money::new(1000.0, currency).unwrap(),
                ),
                (
                    Money::new(500.0, currency).unwrap(),
                    Money::agnostic(1000.0).unwrap(),
                ),
                // Both denominated the same way: equal currencies pass through.
                (
                    Money::new(500.0, currency).unwrap(),
                    Money::new(1000.0, currency).unwrap(),
                ),
            ] {
                let schedule =
                    Schedule::with_payment(rate(0.10), Payment(payment), Principal(principal))
                        .unwrap();
                let mut rows = 0;
                for installment in schedule {
                    rows += 1;
                    for amount in [
                        installment.payment(),
                        installment.interest(),
                        installment.principal(),
                        installment.balance(),
                    ] {
                        assert_eq!(
                            amount.currency(),
                            currency,
                            "a {} schedule yielded a row denominated in {}",
                            currency.code(),
                            amount.currency().code(),
                        );
                    }
                }
                assert_eq!(rows, 3, "the 1000-at-10%-paying-500 schedule is 3 periods");
            }
        }
    }

    /// The defect ADR-0054 fixes: each of these constructed `Ok` and then iterated
    /// forever, because the principal reduction was below the ULP of the balance
    /// so `balance - principal == balance`. They stall in the *first* period, so
    /// the constructor now rejects them outright.
    #[test]
    fn a_payment_too_small_to_move_the_balance_does_not_amortise() {
        let unamortizable = |r: f64, pmt: f64, principal: f64| {
            Schedule::with_payment(
                rate(r),
                Payment(Money::agnostic(pmt).unwrap()),
                Principal(Money::agnostic(principal).unwrap()),
            )
            .map(Schedule::count)
        };

        // Strictly more than the 1000.0 of first-period interest, but 20_000 has a
        // ULP of ~3.6e-12, so the 1e-12 reduction rounds away entirely.
        assert_eq!(
            unamortizable(0.05, 1_000.000_000_000_001, 20_000.0),
            Err(TvmError::PaymentDoesNotAmortize),
        );
        // The same at a zero rate: 1e9 has a ULP of ~1.2e-7, so a 1e-8 payment
        // reduces nothing at all.
        assert_eq!(
            unamortizable(0.0, 1e-8, 1e9),
            Err(TvmError::PaymentDoesNotAmortize),
        );
    }

    /// A negative rate can make the balance converge to a positive limit
    /// *asymptotically*: every period genuinely reduces it, so no up-front check
    /// can reject the loan, and it is the iterator's own per-period guard that has
    /// to stop it once the reductions fall below the ULP (ADR-0054). This is the
    /// case that proves the constructor check alone is not enough.
    #[test]
    fn an_asymptotic_schedule_terminates_without_retiring_the_balance() {
        // -10% per period on 1000 accrues -100 of interest, and paying -50 leaves
        // +50 of principal: the balance falls 1000 → 950 → 905 → … → 500, where
        // the interest exactly matches the payment and it can fall no further.
        let schedule = Schedule::with_payment(
            rate(-0.10),
            Payment(Money::agnostic(-50.0).unwrap()),
            Principal(Money::agnostic(1000.0).unwrap()),
        )
        .unwrap();

        let installments: usize = schedule.clone().count();
        assert!(
            installments > 0 && installments < 10_000,
            "expected a long-but-finite schedule, got {installments}",
        );
        // It stops short of zero, at the limit it converges to — the balance is
        // never retired, so the schedule simply ends.
        let last = schedule.last().unwrap();
        assert!(approx(last.balance().value(), 500.0, 1e-6));
    }

    /// Every schedule is finite. `take(n).count()` is bounded by `n` whatever the
    /// inputs, so this asserts the stronger thing: the schedule ends *on its own*,
    /// below a bound no legitimate loan approaches (ADR-0045 rule 2 — the class,
    /// not the instance).
    #[test]
    fn every_schedule_terminates() {
        // (rate, payment, principal) triples spanning the pathological shapes:
        // barely-amortising, zero-rate, negative-rate asymptotic, and healthy.
        let cases = [
            (0.05, 1_000.000_000_000_001, 20_000.0),
            (0.0, 1e-8, 1e9),
            (-0.10, -50.0, 1000.0),
            (-0.5, -1.0, 1e6),
            (0.10, 500.0, 1000.0),
            (0.05, 300.0, 2000.0),
            (1e-12, 1.0, 1e6),
            (0.0, 1.0, 1e6),
            (-1e-9, 1.0, 1e5),
        ];
        for (r, payment, principal) in cases {
            let Ok(schedule) = Schedule::with_payment(
                rate(r),
                Payment(Money::agnostic(payment).unwrap()),
                Principal(Money::agnostic(principal).unwrap()),
            ) else {
                continue; // rejected up front, which is also termination
            };
            let taken = schedule.take(2_000_000).count();
            assert!(
                taken < 2_000_000,
                "rate {r}, payment {payment}, principal {principal} did not terminate",
            );
        }
    }

    /// A stalling schedule yields nothing further — it does not emit a repeated
    /// installment whose balance never moves.
    #[test]
    fn a_stalled_period_ends_the_schedule_rather_than_repeating() {
        let mut schedule = Schedule::with_payment(
            rate(-0.10),
            Payment(Money::agnostic(-50.0).unwrap()),
            Principal(Money::agnostic(1000.0).unwrap()),
        )
        .unwrap();
        let mut previous = f64::INFINITY;
        for installment in schedule.by_ref() {
            assert!(
                installment.balance().value() < previous,
                "period {} did not reduce the balance",
                installment.period(),
            );
            previous = installment.balance().value();
        }
        // Exhausted iterators stay exhausted.
        assert!(schedule.next().is_none());
    }

    /// `next` used to do `self.period += 1`, which panics in a debug build
    /// ("attempt to add with overflow") once the counter wraps at `2³²` — reachable
    /// only because the iterator could run forever, but pinned independently so the
    /// counter cannot panic even if it is somehow driven that far (ADR-0054). The
    /// schedule is built by struct literal because no public constructor can put a
    /// schedule in this state.
    #[test]
    fn the_period_counter_cannot_overflow() {
        use core::marker::PhantomData;
        let mut schedule = Schedule::<Monthly> {
            balance: 1000.0,
            rate: 0.10,
            payment: 500.0,
            currency: crate::Currency::Xxx,
            period: u32::MAX,
            marker: PhantomData,
        };
        // The period would be 2^32, which `u32` cannot hold: the schedule ends
        // instead of wrapping to 0 (or panicking).
        assert!(schedule.next().is_none());
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
                principal_repaid += installment.principal().value();
                final_balance = installment.balance().value();
            }
            assert_eq!(count, 12);
            // The principal portions sum back to the original balance...
            assert!(approx(principal_repaid, principal.value(), 1e-6));
            // ...and the loan is fully repaid at the end.
            assert!(approx(final_balance, 0.0, 1e-6));
        }

        /// `for_term` sizes the payment itself, so it can size one that cannot
        /// amortise: at 20% over 200 periods the level payment exceeds the first
        /// period's interest by ~3.6e-12 while the ULP of the `100_000` balance is
        /// ~1.5e-11, so no period ever moves it. This used to iterate forever —
        /// the CLI died allocating 2.6 GB collecting it (ADR-0054).
        #[test]
        fn a_term_whose_payment_cannot_move_the_balance_is_rejected() {
            assert_eq!(
                Schedule::for_term(
                    rate(0.2),
                    Period::new(200.0).unwrap(),
                    Money::agnostic(100_000.0).unwrap(),
                )
                .map(Schedule::count),
                Err(crate::TvmError::PaymentDoesNotAmortize),
            );
        }

        #[test]
        fn zero_term_is_degenerate() {
            assert_eq!(
                Schedule::for_term(rate(0.01), Period::ZERO, Money::agnostic(1000.0).unwrap())
                    .map(Schedule::count),
                Err(crate::TvmError::ZeroPeriods),
            );
        }
    }
}
