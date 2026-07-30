//! Property-based tests over the public numeric API.
//!
//! These assert the *laws* the operations obey for whole families of inputs,
//! rather than a handful of worked examples: present and future value invert
//! each other, net present value is monotone in the discount rate and collapses
//! to the plain sum at a zero rate, the internal rate of return zeroes the net
//! present value, the annuity payment inverts the annuity present value (for both
//! ordinary and annuity-due), an amortization schedule conserves the principal it
//! repays, currency rounding is idempotent, the dated XNPV agrees with the
//! periodic NPV on whole-year offsets, `Money`'s arithmetic obeys the usual
//! algebraic laws — `try_sum` is the `try_add` fold, `abs` and `signum` decompose an
//! amount, `try_min`/`try_max` bracket their arguments — the finite scalars'
//! ordering is the ordering of the value they
//! wrap, and an owned cashflow series survives a `serde` round trip unchanged.
//!
//! `proptest` is a dev-dependency only, so it never reaches the published
//! crate's dependency tree (the zero-dependency promise is about distribution,
//! not test tooling — `docs/adr/0009-no_std-and-optional-libm.md`). The `std`/
//! `libm`-gated operations (single sum, annuity, `Schedule::for_term`,
//! `Money::round_to_currency`, `DatedCashflows`) are tested only when a
//! transcendental-math feature is on.

use proptest::prelude::*;
use time_value::{amortization::Schedule, Cashflows, Money, Monthly, Payment, Principal, Rate};

/// Absolute closeness check, mirroring the crate's own `no_std`-safe tolerance
/// helper (`f64::abs` is not in `core`).
fn close(a: f64, b: f64, tolerance: f64) -> bool {
    let d = a - b;
    d < tolerance && d > -tolerance
}

/// A schedule that is guaranteed to amortise: the level payment is the first
/// period's interest (`principal × rate`) plus a strictly positive `slice` of the
/// principal, so it always clears that period's interest and the balance must
/// fall. This keeps the generators inside [`Schedule::with_payment`]'s defined
/// domain — a payment at or below the first period's interest is
/// [`TvmError::PaymentDoesNotAmortize`](time_value::TvmError::PaymentDoesNotAmortize),
/// not a schedule.
///
/// The slice is bounded below (5% of the principal per period), which also bounds
/// the schedule's length: the term runs to about `−ln(slice / (rate + slice)) /
/// ln(1 + rate)` periods, a couple of dozen at the extremes of the ranges used
/// here, so every case terminates quickly.
fn amortizing_schedule(rate: f64, principal: f64, slice: f64) -> Schedule<Monthly> {
    Schedule::with_payment(
        Rate::<Monthly>::new(rate).unwrap(),
        Payment(Money::agnostic(principal * (rate + slice)).unwrap()),
        Principal(Money::agnostic(principal).unwrap()),
    )
    .unwrap()
}

proptest! {
    /// At a zero discount rate nothing is discounted, so the net present value is
    /// exactly the arithmetic sum of the cashflows.
    #[test]
    fn npv_at_zero_rate_is_the_plain_sum(
        amounts in prop::collection::vec(-1e6f64..1e6, 1..=16),
    ) {
        let flows: Vec<Money> = amounts.iter().map(|&a| Money::agnostic(a).unwrap()).collect();
        let series = Cashflows::<Monthly>::new(&flows);
        let sum: f64 = amounts.iter().sum();
        let npv = series
            .net_present_value(Rate::<Monthly>::new(0.0).unwrap())
            .unwrap()
            .value();
        // Up to 16 addends, each |·| ≤ 1e6, so accumulated rounding stays well
        // under this tolerance.
        prop_assert!(close(npv, sum, 1e-6));
    }

    /// With every cashflow positive (and at least one discounted), raising the
    /// discount rate can only lower the net present value.
    #[test]
    fn npv_does_not_increase_with_the_rate(
        amounts in prop::collection::vec(1.0f64..1e5, 2..=16),
        low in 0.0f64..0.5,
        bump in 1e-3f64..0.5,
    ) {
        let flows: Vec<Money> = amounts.iter().map(|&a| Money::agnostic(a).unwrap()).collect();
        let series = Cashflows::<Monthly>::new(&flows);
        let npv_low = series
            .net_present_value(Rate::<Monthly>::new(low).unwrap())
            .unwrap()
            .value();
        let npv_high = series
            .net_present_value(Rate::<Monthly>::new(low + bump).unwrap())
            .unwrap()
            .value();
        // Each discounted term shrinks as the rate rises; the undiscounted t=0
        // term is unchanged. A tiny epsilon absorbs rounding.
        prop_assert!(npv_high <= npv_low + 1e-6);
    }

    /// A conventional series — an outflow now, then inflows that more than repay
    /// it — has an internal rate of return, and discounting at it zeroes the NPV.
    #[test]
    fn irr_zeroes_the_npv(
        inflows in prop::collection::vec(1.0f64..1e3, 1..=10),
        fraction in 0.05f64..0.95,
    ) {
        // Outflow strictly below the total inflow: NPV > 0 at r = 0 and tends to
        // the (negative) initial outflow as r → ∞, so a root is guaranteed.
        let total: f64 = inflows.iter().sum();
        let outflow = total * fraction;
        let mut flows = vec![Money::agnostic(-outflow).unwrap()];
        flows.extend(inflows.iter().map(|&a| Money::agnostic(a).unwrap()));
        let series = Cashflows::<Monthly>::new(&flows);

        let irr = series.internal_rate_of_return().unwrap();
        // The solver converges to a magnitude-relative tolerance (ADR-0021), so
        // the residual NPV is bounded relative to the cashflow scale, not by a
        // fixed absolute epsilon.
        prop_assert!(close(
            series.net_present_value(irr).unwrap().value(),
            0.0,
            1e-6 * total
        ));
    }

    /// Negation is an involution: flipping a cashflow's sign twice is a no-op.
    /// Exact, not approximate — IEEE negation only toggles the sign bit.
    #[test]
    fn negating_money_twice_is_the_identity(amount in -1e12f64..1e12) {
        let money = Money::agnostic(amount).unwrap();
        prop_assert_eq!(-(-money), money);
    }

    /// Subtraction is addition of the negation (ADR-0023). Bounded well inside
    /// `f64` range, so neither form can overflow and both must be `Ok`.
    #[test]
    fn subtracting_money_is_adding_its_negation(
        a in -1e12f64..1e12,
        b in -1e12f64..1e12,
    ) {
        let (a, b) = (Money::agnostic(a).unwrap(), Money::agnostic(b).unwrap());
        prop_assert_eq!(a.try_sub(b).unwrap(), a.try_add(-b).unwrap());
    }

    /// Scaling then unscaling by the same non-tiny factor recovers the amount.
    #[test]
    fn scaling_money_then_dividing_recovers_it(
        amount in -1e9f64..1e9,
        factor in 0.01f64..100.0,
    ) {
        let money = Money::agnostic(amount).unwrap();
        let recovered = money.try_mul(factor).unwrap().try_div(factor).unwrap();
        prop_assert!(close(recovered.value(), amount, 1e-6 + 1e-12 * amount.abs()));
    }

    /// `try_sum` *is* the left-to-right `try_add` fold from `Money::ZERO` (ADR-0061),
    /// over whole families of series rather than the worked example the unit tests
    /// pin — including the empty one, which must give `Money::ZERO`. Amounts are
    /// bounded well inside `f64` range, so no partial sum can overflow and every
    /// call must be `Ok`.
    #[test]
    fn try_sum_is_the_try_add_fold(
        amounts in prop::collection::vec(-1e9f64..1e9, 0..=32),
    ) {
        let flows: Vec<Money> = amounts.iter().map(|&a| Money::agnostic(a).unwrap()).collect();
        let mut folded = Money::ZERO;
        for &flow in &flows {
            folded = folded.try_add(flow).unwrap();
        }
        prop_assert_eq!(Money::try_sum(flows.iter().copied()).unwrap(), folded);
        // Totalling the negated series negates the total: the fold is linear.
        prop_assert_eq!(
            Money::try_sum(flows.iter().map(|&m| -m)).unwrap(),
            -folded
        );
    }

    /// `abs` and `signum` decompose an amount into a magnitude and a direction that
    /// multiply back to it *exactly* — both are sign operations, so no arithmetic is
    /// lost (ADR-0061). `abs` is idempotent and never negative, and `signum` agrees
    /// with the raw value's direction.
    #[test]
    fn abs_and_signum_decompose_the_amount(amount in -1e12f64..1e12) {
        let money = Money::agnostic(amount).unwrap();

        prop_assert_eq!(money.abs(), Money::agnostic(amount.abs()).unwrap());
        prop_assert_eq!(money.abs().abs(), money.abs());
        prop_assert!(money.abs().value() >= 0.0);
        prop_assert_eq!(money.abs(), (-money).abs());
        // |x| · sgn(x) = x, for the zero case too (0 · 0 = 0).
        prop_assert_eq!(money.abs().try_mul(money.signum()).unwrap(), money);
        // Spelled as inequalities: the crate denies `clippy::float_cmp`.
        prop_assert_eq!(money.signum() > 0.0, amount > 0.0);
        prop_assert_eq!(money.signum() < 0.0, amount < 0.0);
    }

    /// `try_min`/`try_max` select one of their two arguments, bracket both, are
    /// commutative, and between them account for the pair — the properties an
    /// infallible `min`/`max` would have, holding wherever the partial ordering is
    /// defined (ADR-0059, ADR-0061).
    #[test]
    fn try_min_and_try_max_bracket_their_arguments(
        a in -1e12f64..1e12,
        b in -1e12f64..1e12,
    ) {
        let (x, y) = (Money::agnostic(a).unwrap(), Money::agnostic(b).unwrap());
        let lo = x.try_min(y).unwrap();
        let hi = x.try_max(y).unwrap();

        // The answer is one of the arguments, not a computed value …
        prop_assert!(lo == x || lo == y);
        prop_assert!(hi == x || hi == y);
        // … it brackets them both …
        prop_assert!(lo <= x && lo <= y);
        prop_assert!(hi >= x && hi >= y);
        // … the two together are the pair, in some order …
        prop_assert_eq!(lo.try_add(hi).unwrap(), x.try_add(y).unwrap());
        // … and neither depends on the argument order.
        prop_assert_eq!(lo, y.try_min(x).unwrap());
        prop_assert_eq!(hi, y.try_max(x).unwrap());
    }

    /// Every installment accounts for its whole payment: the interest on the
    /// opening balance plus the principal repaid *is* the amount paid (ADR-0027).
    /// This generalises the `interest_plus_principal_equals_each_payment` point
    /// test, and it holds for the smaller final installment too — there the
    /// payment is whatever closes the loan.
    ///
    /// The split is a rearrangement (`principal = payment − interest`), so the sum
    /// recovers the payment only up to rounding; the tolerance is relative to the
    /// principal, which bounds every quantity in the schedule.
    #[test]
    fn every_installment_splits_its_payment_into_interest_and_principal(
        rate in 0.0f64..0.2,
        principal in 100.0f64..1e6,
        slice in 0.05f64..1.0,
    ) {
        for installment in amortizing_schedule(rate, principal, slice) {
            prop_assert!(close(
                installment.interest().value() + installment.principal().value(),
                installment.payment().value(),
                1e-9 * principal,
            ));
        }
    }

    /// Interest accrues on the balance *outstanding at the start of the period* —
    /// the previous installment's closing balance, or the original principal for
    /// the first — and the principal repaid is exactly what the balance falls by.
    ///
    /// Both are asserted exactly rather than approximately: the schedule evaluates
    /// these very expressions (`balance × rate`, then `balance − principal`), so
    /// recomputing them here reproduces the same `f64` bit-for-bit. The closing
    /// installment satisfies the second law too, by repaying the whole remaining
    /// balance and landing on zero.
    ///
    /// The comparison is between `Money` values rather than bare `f64`s — the
    /// crate denies `clippy::float_cmp`, and equality on the public type is the
    /// idiom the rest of this file uses where a law really is exact.
    #[test]
    fn interest_accrues_on_the_opening_balance(
        rate in 0.0f64..0.2,
        principal in 100.0f64..1e6,
        slice in 0.05f64..1.0,
    ) {
        let mut opening = Money::agnostic(principal).unwrap();
        for installment in amortizing_schedule(rate, principal, slice) {
            prop_assert_eq!(
                installment.interest(),
                Money::agnostic(opening.value() * rate).unwrap()
            );
            prop_assert_eq!(
                installment.balance(),
                opening.try_sub(installment.principal()).unwrap()
            );
            opening = installment.balance();
        }
    }

    /// The schedule conserves principal: the principal portions of every
    /// installment sum back to the amount borrowed, no more and no less, and the
    /// loan ends exactly repaid. The residual is an accumulation of at most a few
    /// dozen roundings, so the tolerance scales with the principal.
    ///
    /// The final balance is exactly zero, not merely close to it: the branch that
    /// closes the loan assigns `0.0` rather than subtracting down to it.
    #[test]
    fn the_principal_portions_repay_exactly_the_principal(
        rate in 0.0f64..0.2,
        principal in 100.0f64..1e6,
        slice in 0.05f64..1.0,
    ) {
        let mut repaid = 0.0;
        let mut final_balance = None;
        for installment in amortizing_schedule(rate, principal, slice) {
            repaid += installment.principal().value();
            final_balance = Some(installment.balance());
        }
        // `Some` also witnesses that a positive principal owes at least one
        // installment; an exhausted schedule would leave this `None`.
        prop_assert_eq!(final_balance, Some(Money::ZERO));
        prop_assert!(close(repaid, principal, 1e-9 * principal));
    }

    /// A schedule that amortises never stalls or rebounds: each installment leaves
    /// strictly less outstanding than the one before, so the balance descends
    /// monotonically from the principal to zero and the iterator terminates. This
    /// is the positive statement behind [`Schedule::with_payment`]'s rejection of a
    /// payment that cannot amortise (ADR-0027, ADR-0031).
    #[test]
    fn the_balance_falls_monotonically_to_zero(
        rate in 0.0f64..0.2,
        principal in 100.0f64..1e6,
        slice in 0.05f64..1.0,
    ) {
        let mut previous = Money::agnostic(principal).unwrap();
        for installment in amortizing_schedule(rate, principal, slice) {
            prop_assert!(installment.balance().value() < previous.value());
            previous = installment.balance();
        }
        prop_assert_eq!(previous, Money::ZERO);
    }

    /// **Every** schedule terminates — not only the ones `amortizing_schedule`
    /// constructs to be well behaved. The generators here range over the whole
    /// `with_payment` domain, including negative rates, payments a hair above the
    /// first period's interest, and payments far below it, so they reach the
    /// shapes that used to iterate forever: a principal reduction smaller than the
    /// ULP of the balance leaves `balance − principal == balance` and the schedule
    /// never moves again (ADR-0054).
    ///
    /// `take` bounds the work, and the assertion is that the bound was *not*
    /// reached — a schedule that ends on its own, rather than one merely cut short.
    /// The cap is far above any real loan; the longest healthy case these ranges
    /// produce is a few thousand periods.
    #[test]
    fn every_schedule_terminates(
        rate in -0.5f64..0.5,
        principal in 1.0f64..1e9,
        payment in -1e6f64..1e6,
    ) {
        const CAP: usize = 200_000;

        let schedule = Schedule::<Monthly>::with_payment(
            Rate::<Monthly>::new(rate).unwrap(),
            Payment(Money::agnostic(payment).unwrap()),
            Principal(Money::agnostic(principal).unwrap()),
        );
        // A rejected loan is terminated too — loudly, at construction.
        if let Ok(schedule) = schedule {
            prop_assert!(schedule.take(CAP).count() < CAP);
        }
    }

    /// Every installment a schedule yields strictly reduces the balance. That is
    /// what makes termination structural rather than incidental: the balance is a
    /// strictly decreasing sequence bounded below, and in `f64` a strictly
    /// decreasing sequence is finite. Unlike
    /// `the_balance_falls_monotonically_to_zero`, this holds over the *whole*
    /// domain — it does not claim the balance reaches zero, only that it always
    /// moves (ADR-0054).
    #[test]
    fn no_installment_ever_leaves_the_balance_where_it_was(
        rate in -0.5f64..0.5,
        principal in 1.0f64..1e9,
        payment in -1e6f64..1e6,
    ) {
        let Ok(schedule) = Schedule::<Monthly>::with_payment(
            Rate::<Monthly>::new(rate).unwrap(),
            Payment(Money::agnostic(payment).unwrap()),
            Principal(Money::agnostic(principal).unwrap()),
        ) else {
            return Ok(());
        };
        let mut previous = principal;
        for installment in schedule.take(200_000) {
            prop_assert!(installment.balance().value() < previous);
            previous = installment.balance().value();
        }
    }

    /// A `Rate<P>`'s ordering *is* the ordering of its per-period value — that is the
    /// whole claim behind giving it `Ord` (ADR-0059), so it is asserted over the
    /// domain rather than at a handful of points. `PartialOrd` is pinned to agree
    /// with `Ord` in the same breath: the order is total, so it never answers `None`.
    #[test]
    fn rate_ordering_agrees_with_the_per_period_value(
        a in -0.999f64..1e9,
        b in -0.999f64..1e9,
    ) {
        let (x, y) = (Rate::<Monthly>::new(a).unwrap(), Rate::<Monthly>::new(b).unwrap());

        prop_assert_eq!(x.cmp(&y), a.partial_cmp(&b).unwrap());
        prop_assert_eq!(x.partial_cmp(&y), Some(x.cmp(&y)));
        prop_assert_eq!(x > y, a > b);
        // Equality is spelled through `partial_cmp` rather than `a == b`: the crate
        // denies `clippy::float_cmp`, and this says the same thing.
        prop_assert_eq!(x == y, a.partial_cmp(&b).unwrap().is_eq());
    }

    /// The laws `Eq` and `Ord` promise, over three arbitrary rates: the comparison is
    /// *total* (exactly one of the three outcomes holds), antisymmetric, and
    /// transitive. These are the obligations that make deriving `Eq`/`Ord` on a float
    /// newtype sound only because the constructor rejects `NaN`, so they are the
    /// assumption worth pinning.
    #[test]
    fn rate_ordering_obeys_the_total_order_laws(
        a in -0.999f64..1e9,
        b in -0.999f64..1e9,
        c in -0.999f64..1e9,
    ) {
        use core::cmp::Ordering;

        let x = Rate::<Monthly>::new(a).unwrap();
        let y = Rate::<Monthly>::new(b).unwrap();
        let z = Rate::<Monthly>::new(c).unwrap();
        // A second, separately-constructed copy, so the reflexivity assertions below
        // are not comparing one expression with itself.
        let x_again = Rate::<Monthly>::new(a).unwrap();

        // Reflexive — the law a float newtype could only fail through `NaN`.
        prop_assert_eq!(x.cmp(&x_again), Ordering::Equal);
        prop_assert!(x == x_again);

        // Total: exactly one of less / equal / greater.
        prop_assert_eq!(u8::from(x < y) + u8::from(x == y) + u8::from(x > y), 1);

        // Antisymmetric, and `cmp` is its own reverse.
        prop_assert_eq!(x.cmp(&y), y.cmp(&x).reverse());

        // Transitive.
        if x <= y && y <= z {
            prop_assert!(x <= z);
        }
    }
}

#[cfg(any(feature = "std", feature = "libm"))]
proptest! {
    /// `Period<P>`'s ordering is the ordering of its count, for the same reason as
    /// `Rate`'s (ADR-0059) — and this is a comparison that did not compile before,
    /// because the derived `PartialOrd` it replaces carried an unsatisfiable
    /// `P: PartialOrd` bound.
    #[test]
    fn period_ordering_agrees_with_the_count(
        a in 0.0f64..1e9,
        b in 0.0f64..1e9,
    ) {
        use time_value::Period;

        let (x, y) = (Period::<Monthly>::new(a).unwrap(), Period::<Monthly>::new(b).unwrap());

        prop_assert_eq!(x.cmp(&y), a.partial_cmp(&b).unwrap());
        prop_assert_eq!(x.partial_cmp(&y), Some(x.cmp(&y)));
        prop_assert_eq!(x < y, a < b);
        prop_assert_eq!(x == y, a.partial_cmp(&b).unwrap().is_eq());
    }

    /// `ContinuousRate`'s ordering is the ordering of its force of interest. Its
    /// domain is every finite `f64`, including values at or below `−1` that a
    /// per-period `Rate` cannot hold (ADR-0036), so the generator spans the sign.
    #[test]
    fn continuous_rate_ordering_agrees_with_the_force(
        a in -1e9f64..1e9,
        b in -1e9f64..1e9,
    ) {
        use time_value::ContinuousRate;

        let (x, y) = (ContinuousRate::new(a).unwrap(), ContinuousRate::new(b).unwrap());

        prop_assert_eq!(x.cmp(&y), a.partial_cmp(&b).unwrap());
        prop_assert_eq!(x.partial_cmp(&y), Some(x.cmp(&y)));
        prop_assert_eq!(x > y, a > b);
        prop_assert_eq!(x == y, a.partial_cmp(&b).unwrap().is_eq());
    }
}

#[cfg(any(feature = "std", feature = "libm"))]
proptest! {
    /// Present value undoes future value: compounding an amount forward then
    /// discounting it back recovers the original, for any rate and horizon.
    #[test]
    fn present_value_inverts_future_value(
        amount in 1.0f64..1e6,
        rate in -0.9f64..1.0,
        periods in 0.0f64..60.0,
    ) {
        use time_value::{single_sum, Period};

        let rate = Rate::<Monthly>::new(rate).unwrap();
        let periods = Period::new(periods).unwrap();
        let amount = Money::agnostic(amount).unwrap();

        let future = single_sum::future_value(rate, periods, amount).unwrap();
        let back = single_sum::present_value(rate, periods, future).unwrap();
        // Round-trips through the same compound factor, so the error is a few
        // ulps of the amount — a relative tolerance keeps it scale-independent.
        prop_assert!(close(back.value(), amount.value(), 1e-6 * amount.value()));
    }

    /// The level annuity payment is the inverse of the annuity present value:
    /// pricing a payment stream then amortising that price recovers the payment.
    #[test]
    fn annuity_payment_inverts_present_value(
        payment in 1.0f64..1e5,
        rate in -0.9f64..1.0,
        periods in 1.0f64..120.0,
    ) {
        use time_value::{annuity, Period};

        let rate = Rate::<Monthly>::new(rate).unwrap();
        // At least one period, so the amortisation is not degenerate.
        let periods = Period::new(periods).unwrap();
        let payment = Money::agnostic(payment).unwrap();

        let present = annuity::present_value(rate, periods, payment).unwrap();
        let recovered = annuity::payment(rate, periods, present).unwrap();
        prop_assert!(close(recovered.value(), payment.value(), 1e-6 * payment.value()));
    }

    /// The same inverse relationship holds for the annuity-due variant: pricing a
    /// start-of-period payment stream then amortising that price recovers it.
    #[test]
    fn due_payment_inverts_due_present_value(
        payment in 1.0f64..1e5,
        rate in -0.9f64..1.0,
        periods in 1.0f64..120.0,
    ) {
        use time_value::{annuity, Period};

        let rate = Rate::<Monthly>::new(rate).unwrap();
        let periods = Period::new(periods).unwrap();
        let payment = Money::agnostic(payment).unwrap();

        let present = annuity::due::present_value(rate, periods, payment).unwrap();
        let recovered = annuity::due::payment(rate, periods, present).unwrap();
        prop_assert!(close(recovered.value(), payment.value(), 1e-6 * payment.value()));
    }

    /// A periodicity conversion preserves economic value, so converting a monthly
    /// rate to annual and back recovers it (ADR-0024). The quantity compared is
    /// the *growth factor* `1 + r`, relative to its size.
    ///
    /// The range is bounded to realistic per-period rates (−50% … +200%). Far
    /// below that, the intermediate *annual* growth factor `(1+r)^12` becomes
    /// tiny, and representing it as a rate (`−1 + ε`) loses ε to catastrophic
    /// cancellation — so the round-trip degrades near −100% by nature, not by
    /// bug. That degenerate regime is pinned down by dedicated unit tests instead.
    #[test]
    fn converting_a_rate_there_and_back_preserves_the_growth_factor(rate in -0.5f64..2.0) {
        let monthly = Rate::<Monthly>::new(rate).unwrap();
        let round_trip = monthly
            .effective_annual()
            .unwrap()
            .convert::<Monthly>()
            .unwrap();
        prop_assert!(close(1.0 + round_trip.value(), 1.0 + rate, 1e-9 * (1.0 + rate)));
    }

    /// Solving a single sum for `n` (NPER) inverts compounding: the number of
    /// periods that grows `present` to its own future value is the periods used.
    /// A positive rate keeps the growth unambiguous (a zero rate has no solution).
    #[test]
    fn single_sum_periods_inverts_future_value(
        present in 1.0f64..1e6,
        rate in 0.001f64..1.0,
        periods in 1.0f64..120.0,
    ) {
        use time_value::{single_sum, FutureValue, Period, PresentValue};

        let r = Rate::<Monthly>::new(rate).unwrap();
        let present = Money::agnostic(present).unwrap();
        let n = Period::new(periods).unwrap();

        let future = single_sum::future_value(r, n, present).unwrap();
        let recovered = single_sum::periods(r, PresentValue(present), FutureValue(future)).unwrap();
        prop_assert!(close(recovered.value(), periods, 1e-6 * periods));
    }

    /// Solving a single sum for `r` (RATE) inverts compounding: the rate that
    /// grows `present` to its own future value is the rate used. Compared as the
    /// growth factor `1 + r`, relative to its size (as the conversion test does).
    #[test]
    fn single_sum_rate_inverts_future_value(
        present in 1.0f64..1e6,
        rate in -0.5f64..1.0,
        periods in 1.0f64..120.0,
    ) {
        use time_value::{single_sum, FutureValue, Period, PresentValue};

        let r = Rate::<Monthly>::new(rate).unwrap();
        let present = Money::agnostic(present).unwrap();
        let n = Period::new(periods).unwrap();

        let future = single_sum::future_value(r, n, present).unwrap();
        let recovered = single_sum::rate::<Monthly>(n, PresentValue(present), FutureValue(future)).unwrap();
        prop_assert!(close(1.0 + recovered.value(), 1.0 + rate, 1e-6 * (1.0 + rate)));
    }

    /// Solving an annuity for `n` (NPER) inverts pricing: the number of payments
    /// that amortise a stream's own present value is the count used. A positive
    /// rate keeps the payment above the period's interest, so `n` is defined.
    ///
    /// The range is bounded to a well-conditioned regime (`n·ln(1+r)` modest).
    /// Beyond it the round-trip degrades *by nature*: `present_value` forms
    /// `1 − (1+r)⁻ⁿ`, and once `(1+r)⁻ⁿ` underflows toward `0` the present value
    /// saturates at `PMT/r`, so `n` is no longer recoverable from it — a
    /// cancellation limit at the pricing step, not a solver bug. (The single-sum
    /// and future-value NPER use clean ratios and don't hit it.)
    #[test]
    fn annuity_periods_inverts_present_value(
        payment in 1.0f64..1e5,
        rate in 0.001f64..0.2,
        periods in 1.0f64..60.0,
    ) {
        use time_value::{annuity, Payment, Period, PresentValue};

        let r = Rate::<Monthly>::new(rate).unwrap();
        let payment = Money::agnostic(payment).unwrap();
        let n = Period::new(periods).unwrap();

        let present = annuity::present_value(r, n, payment).unwrap();
        let recovered = annuity::periods(r, Payment(payment), PresentValue(present)).unwrap();
        prop_assert!(close(recovered.value(), periods, 1e-6 * periods));
    }

    /// Solving an annuity for `r` (RATE) inverts pricing: the iterative solver
    /// recovers the rate that prices a stream at its own present value. Compared
    /// as the growth factor `1 + r`, relative to its size.
    #[test]
    fn annuity_rate_inverts_present_value(
        payment in 1.0f64..1e5,
        rate in -0.5f64..1.0,
        periods in 1.0f64..120.0,
    ) {
        use time_value::{annuity, Payment, Period, PresentValue};

        let r = Rate::<Monthly>::new(rate).unwrap();
        let payment = Money::agnostic(payment).unwrap();
        let n = Period::new(periods).unwrap();

        let present = annuity::present_value(r, n, payment).unwrap();
        let recovered = annuity::rate::<Monthly>(n, Payment(payment), PresentValue(present)).unwrap();
        prop_assert!(close(1.0 + recovered.value(), 1.0 + rate, 1e-6 * (1.0 + rate)));
    }

    /// A conventional *dated* series — an outflow now, then inflows on strictly
    /// later, irregularly spaced dates that more than repay it — has an XIRR, and
    /// discounting at it zeroes the XNPV (ADR-0029). The dated analogue of
    /// `irr_zeroes_the_npv`.
    ///
    /// The regime is bounded to keep the *annualised* rate well-conditioned: each
    /// gap is at least a quarter-year and the outflow is at least 30% of the
    /// inflows. Both matter because annualising a large sub-period return over a
    /// short horizon explodes — an inflow a fortnight after a tiny outflow implies
    /// an astronomical annual rate that no finite solver can bracket. That is
    /// degenerate annualisation, not a solver fault; the realistic band is tested.
    #[test]
    fn xirr_zeroes_the_xnpv(
        spec in prop::collection::vec((1.0f64..1e3, 0.25f64..2.0), 1..=8),
        fraction in 0.3f64..0.95,
    ) {
        use time_value::{DatedCashflow, DatedCashflows};

        // Each (inflow, gap): cumulative gaps give strictly increasing year-offsets
        // after the reference outflow at t = 0.
        let total: f64 = spec.iter().map(|&(a, _)| a).sum();
        let outflow = total * fraction; // strictly below the inflows, so a root exists

        let mut flows =
            vec![DatedCashflow::new(0.0, Money::agnostic(-outflow).unwrap()).unwrap()];
        let mut t = 0.0;
        for (inflow, gap) in spec {
            t += gap;
            flows.push(DatedCashflow::new(t, Money::agnostic(inflow).unwrap()).unwrap());
        }
        let series = DatedCashflows::new(&flows);

        let irr = series.internal_rate_of_return().unwrap();
        prop_assert!(close(
            series.net_present_value(irr).unwrap().value(),
            0.0,
            1e-6 * total
        ));
    }

    /// The continuous bridge is a round trip: the force of interest equivalent to
    /// an effective annual rate converts back to that same rate (ADR-0036). This
    /// generalises the `bridges_to_and_from_the_effective_annual_rate` point test
    /// to the whole class of rates.
    ///
    /// The band is bounded to realistic effective-annual rates (−90% … +200%);
    /// far outside it the intermediate `e^δ` over-/under-flows, a regime the
    /// dedicated overflow/floor unit tests pin instead. The quantity compared is
    /// the growth factor `1 + r`, relative to its size.
    #[test]
    fn continuous_bridge_round_trips_the_effective_annual_rate(rate in -0.9f64..2.0) {
        use time_value::{Annual, ContinuousRate};

        let effective = Rate::<Annual>::new(rate).unwrap();
        let force = ContinuousRate::from_effective_annual(effective);
        // δ = ln(1 + r), then e^δ − 1 recovers r.
        let back = force.effective_annual().unwrap();
        prop_assert!(close(1.0 + back.value(), 1.0 + rate, 1e-9 * (1.0 + rate)));
    }

    /// Continuous discounting inverts continuous compounding: growing an amount at
    /// a force of interest over a horizon then discounting it back over the same
    /// horizon recovers the amount (ADR-0036). This generalises the
    /// `present_value_inverts_future_value_and_keeps_currency` point test.
    ///
    /// The force × horizon is bounded so `e^{δ·t}` stays comfortably finite; the
    /// error is a few ulps of the amount, so a relative tolerance keeps it
    /// scale-independent.
    #[test]
    fn continuous_present_value_inverts_future_value(
        amount in 1.0f64..1e6,
        force in -0.5f64..0.5,
        years in 0.0f64..30.0,
    ) {
        use time_value::{continuous, ContinuousRate};

        let rate = ContinuousRate::new(force).unwrap();
        let present = Money::agnostic(amount).unwrap();
        let future = continuous::future_value(rate, years, present).unwrap();
        let back = continuous::present_value(rate, years, future).unwrap();
        prop_assert!(close(back.value(), present.value(), 1e-6 * present.value()));
    }

    /// A term-sized schedule runs for exactly the term requested: the payment
    /// [`annuity::payment`](time_value::annuity::payment) computes retires the
    /// principal on period `n`, neither leaving a stub on `n + 1` nor finishing
    /// early (ADR-0027). This generalises the `runs_exactly_the_term_and_clears_
    /// the_balance` point test to the whole class of terms.
    ///
    /// Landing on `n` at all depends on `FINAL_INSTALLMENT_SLACK` absorbing the
    /// floating-point residual of a *computed* level payment — this property is
    /// what pins that constant to its job. The band is bounded to well-conditioned
    /// monthly loans (0.1%–5% per period, up to ten years): the level payment is
    /// formed from `1 − (1+r)⁻ⁿ`, so far outside it the payment itself loses
    /// precision to cancellation and the closing period is no longer determined by
    /// the schedule's arithmetic. Whole periods are used because a fractional term
    /// has no "final period" to land on.
    #[test]
    fn for_term_lands_the_final_installment_on_the_requested_period(
        rate in 0.001f64..0.05,
        periods in 1u32..=120,
        principal in 100.0f64..1e7,
    ) {
        use time_value::Period;

        let schedule = Schedule::for_term(
            Rate::<Monthly>::new(rate).unwrap(),
            Period::new(f64::from(periods)).unwrap(),
            Money::agnostic(principal).unwrap(),
        )
        .unwrap();

        let mut count = 0u32;
        let mut repaid = 0.0;
        let mut final_balance = None;
        for installment in schedule {
            count += 1;
            repaid += installment.principal().value();
            final_balance = Some(installment.balance());
        }
        prop_assert_eq!(count, periods);
        prop_assert_eq!(final_balance, Some(Money::ZERO));
        // Up to 120 roundings, and the computed payment carries its own residual,
        // so this is looser than the exact-payment schedule's conservation law.
        prop_assert!(close(repaid, principal, 1e-6 * principal));
    }

    /// A growing annuity with **zero growth** is a level annuity: both the present
    /// and the future value agree with their `annuity` counterparts (ADR-0048).
    /// This is the reduction the growing factors are built to satisfy, and it is a
    /// universal over every rate and term, so it is a property rather than a case.
    #[test]
    fn growing_at_zero_growth_is_the_level_annuity(
        payment in 1.0f64..1e5,
        rate in -0.5f64..1.0,
        periods in 1.0f64..120.0,
    ) {
        use time_value::{annuity, Growth, Period};

        let r = Rate::<Monthly>::new(rate).unwrap();
        let g = Growth(Rate::<Monthly>::new(0.0).unwrap());
        let n = Period::new(periods).unwrap();
        let payment = Money::agnostic(payment).unwrap();

        let level_pv = annuity::present_value(r, n, payment).unwrap().value();
        let growing_pv = annuity::growing_present_value(r, g, n, payment).unwrap().value();
        // The two factors reach the same value by different routes — `(1+r)⁻ⁿ`
        // against `((1+0)/(1+r))ⁿ` — so they agree to a few ulps, not exactly.
        prop_assert!(close(growing_pv, level_pv, 1e-9 * level_pv.abs()));

        let level_fv = annuity::future_value(r, n, payment).unwrap().value();
        let growing_fv = annuity::growing_future_value(r, g, n, payment).unwrap().value();
        prop_assert!(close(growing_fv, level_fv, 1e-9 * level_fv.abs()));
    }

    /// The growing future value is the growing present value carried forward over
    /// the whole term: `FV = PV · (1 + r)ⁿ` (ADR-0048). The band is bounded so the
    /// compounded value stays comfortably finite.
    #[test]
    fn growing_future_value_is_the_present_value_compounded(
        payment in 1.0f64..1e5,
        rate in -0.5f64..0.5,
        growth in -0.5f64..0.5,
        periods in 1.0f64..40.0,
    ) {
        use time_value::{annuity, Growth, Period};

        let r = Rate::<Monthly>::new(rate).unwrap();
        let g = Growth(Rate::<Monthly>::new(growth).unwrap());
        let n = Period::new(periods).unwrap();
        let payment = Money::agnostic(payment).unwrap();

        let pv = annuity::growing_present_value(r, g, n, payment).unwrap().value();
        let fv = annuity::growing_future_value(r, g, n, payment).unwrap().value();
        let compounded = pv * (1.0 + rate).powf(periods);
        prop_assert!(close(fv, compounded, 1e-6 * compounded.abs()));
    }

    /// As the term grows without bound the growing annuity approaches the growing
    /// *perpetuity* `PMT / (r − g)`, whenever `r > g` (ADR-0048). Generated as a
    /// growth plus a strictly positive spread, so the perpetuity converges and is
    /// a legal comparison in the first place.
    ///
    /// A 2000-period term is "without bound" enough here: the residual is
    /// `((1+g)/(1+r))ⁿ`, which at the narrowest spread in range is already about
    /// `1e-9` of the answer.
    #[test]
    fn a_long_growing_annuity_approaches_the_growing_perpetuity(
        payment in 1.0f64..1e5,
        growth in -0.2f64..0.2,
        spread in 0.01f64..0.5,
    ) {
        use time_value::{annuity, Growth, Period};

        let g = Growth(Rate::<Monthly>::new(growth).unwrap());
        let r = Rate::<Monthly>::new(growth + spread).unwrap();
        let n = Period::new(2000.0).unwrap();
        let payment = Money::agnostic(payment).unwrap();

        let finite = annuity::growing_present_value(r, g, n, payment).unwrap().value();
        let forever = annuity::growing_perpetuity(r, g, payment).unwrap().value();
        prop_assert!(close(finite, forever, 1e-6 * forever));
    }

    /// Each growing annuity-due is its ordinary counterpart scaled by `(1 + r)` —
    /// the same relationship ADR-0015 established for the level case, now holding
    /// across the growing pair too (ADR-0048).
    #[test]
    fn growing_due_is_the_ordinary_value_scaled_by_one_plus_the_rate(
        payment in 1.0f64..1e5,
        rate in -0.5f64..0.5,
        growth in -0.5f64..0.5,
        periods in 1.0f64..40.0,
    ) {
        use time_value::{annuity, Growth, Period};

        let r = Rate::<Monthly>::new(rate).unwrap();
        let g = Growth(Rate::<Monthly>::new(growth).unwrap());
        let n = Period::new(periods).unwrap();
        let payment = Money::agnostic(payment).unwrap();

        let ordinary_present = annuity::growing_present_value(r, g, n, payment).unwrap().value();
        let due_present = annuity::due::growing_present_value(r, g, n, payment).unwrap().value();
        let scaled_present = ordinary_present * (1.0 + rate);
        prop_assert!(close(due_present, scaled_present, 1e-9 * scaled_present.abs()));

        let ordinary_future = annuity::growing_future_value(r, g, n, payment).unwrap().value();
        let due_future = annuity::due::growing_future_value(r, g, n, payment).unwrap().value();
        let scaled_future = ordinary_future * (1.0 + rate);
        prop_assert!(close(due_future, scaled_future, 1e-9 * scaled_future.abs()));
    }

    /// Rounding to a currency's minor unit is **idempotent**: the result is already
    /// on the minor-unit grid, so rounding it again changes nothing. Asserted
    /// exactly, and over every currency in the enum rather than a chosen few —
    /// `index` selects from [`Currency::ALL`], so the whole closed set is in range
    /// (ADR-0034, ADR-0045).
    #[test]
    fn rounding_to_a_currency_is_idempotent(
        amount in -1e9f64..1e9,
        index in 0usize..time_value::Currency::ALL.len(),
    ) {
        let money = Money::new(amount, time_value::Currency::ALL[index]).unwrap();
        let once = money.round_to_currency();
        prop_assert_eq!(once.round_to_currency(), once);
    }

    /// Rounding is a *presentation* step, so it never changes what the amount is
    /// denominated in (ADR-0033/0034) and never moves the magnitude by as much as a
    /// whole minor unit — at most half of one, since it rounds to the nearest.
    ///
    /// The allowance on top of the half-unit is a flat `1e-6`, and it is
    /// deliberately *absolute* rather than relative to the amount. It has to clear
    /// the error of scaling by `10^exponent` and back, which peaks around `2e-7` at
    /// the top of this magnitude range, while staying far below the tightest
    /// half-unit in the enum — `5e-5`, for the exponent-4 currencies. A relative
    /// allowance would fail that second test: at a magnitude of `1e9` even
    /// `1e-9 × amount` is `1.0`, which swamps every half-unit and would let the
    /// assertion pass no matter what rounding did.
    ///
    /// Currencies with no minor unit have no grid and are excluded from the
    /// distance bound — `a_currency_without_a_minor_unit_is_never_rounded` pins
    /// those exhaustively instead.
    #[test]
    fn rounding_preserves_the_currency_and_moves_by_under_a_minor_unit(
        amount in -1e9f64..1e9,
        index in 0usize..time_value::Currency::ALL.len(),
    ) {
        let currency = time_value::Currency::ALL[index];
        let rounded = Money::new(amount, currency).unwrap().round_to_currency();
        prop_assert_eq!(rounded.currency(), currency);

        if let Some(exponent) = currency.minor_unit_exponent() {
            let unit = 0.1f64.powi(i32::from(exponent));
            let moved = (rounded.value() - amount).abs();
            prop_assert!(moved <= unit / 2.0 + 1e-6);
        }
    }

    /// The dated XNPV and the periodic NPV are the same calculation seen two ways:
    /// place the flows at whole-year offsets `0, 1, 2, …` and discount at an annual
    /// rate, and `DatedCashflows` must agree with `Cashflows<Annual>` (ADR-0029).
    ///
    /// This is the strongest available check on the dated engine, because the two
    /// share no code — one raises `(1 + r)` to a fractional power per flow, the
    /// other folds a running factor — so agreement is real corroboration rather
    /// than a restatement.
    #[test]
    fn xnpv_agrees_with_the_periodic_npv_on_whole_year_offsets(
        amounts in prop::collection::vec(-1e5f64..1e5, 1..=12),
        rate in -0.5f64..1.0,
    ) {
        use time_value::{Annual, DatedCashflow, DatedCashflows};

        let annual = Rate::<Annual>::new(rate).unwrap();

        let periodic: Vec<Money> = amounts.iter().map(|&a| Money::agnostic(a).unwrap()).collect();
        let periodic_npv = Cashflows::<Annual>::new(&periodic)
            .net_present_value(annual)
            .unwrap()
            .value();

        let dated: Vec<DatedCashflow> = amounts
            .iter()
            .enumerate()
            .map(|(i, &a)| {
                #[allow(clippy::cast_precision_loss)]
                let offset = i as f64;
                DatedCashflow::new(offset, Money::agnostic(a).unwrap()).unwrap()
            })
            .collect();
        let dated_npv = DatedCashflows::new(&dated)
            .net_present_value(annual)
            .unwrap()
            .value();

        // `powf` and the folded factor round differently, so compare relative to
        // the scale of the flows rather than by a fixed epsilon.
        let scale: f64 = amounts.iter().map(|a| a.abs()).sum::<f64>().max(1.0);
        prop_assert!(close(dated_npv, periodic_npv, 1e-9 * scale));
    }

    /// Only the *gaps* between dated flows matter, not where the timeline starts:
    /// every flow is discounted by `(1 + r)^(tᵢ − t₀)` against the **first** flow,
    /// so sliding every offset by the same amount leaves the XNPV unchanged
    /// (ADR-0029). This generalises the `xirr_is_invariant_to_shifting_the_reference`
    /// point test from the solver to the valuation itself.
    #[test]
    fn xnpv_is_invariant_to_sliding_every_offset(
        spec in prop::collection::vec((-1e5f64..1e5, 0.0f64..3.0), 1..=10),
        shift in -50.0f64..50.0,
        rate in -0.5f64..1.0,
    ) {
        use time_value::{Annual, DatedCashflow, DatedCashflows};

        let annual = Rate::<Annual>::new(rate).unwrap();
        let mut offset = 0.0;
        let mut base = Vec::new();
        let mut slid = Vec::new();
        for (amount, gap) in spec {
            offset += gap;
            let money = Money::agnostic(amount).unwrap();
            base.push(DatedCashflow::new(offset, money).unwrap());
            slid.push(DatedCashflow::new(offset + shift, money).unwrap());
        }

        let here = DatedCashflows::new(&base).net_present_value(annual).unwrap().value();
        let there = DatedCashflows::new(&slid).net_present_value(annual).unwrap().value();
        // The shifted offsets are differences of larger numbers, so they lose a
        // little precision before `powf` ever sees them; scale the tolerance.
        prop_assert!(close(there, here, 1e-6 * here.abs().max(1.0)));
    }

    /// At a zero rate nothing is discounted however the flows are dated, so the
    /// XNPV collapses to the arithmetic sum — the dated twin of
    /// `npv_at_zero_rate_is_the_plain_sum`.
    #[test]
    fn xnpv_at_zero_rate_is_the_plain_sum(
        spec in prop::collection::vec((-1e6f64..1e6, 0.0f64..5.0), 1..=12),
    ) {
        use time_value::{Annual, DatedCashflow, DatedCashflows};

        let mut offset = 0.0;
        let mut flows = Vec::new();
        let mut sum = 0.0;
        for (amount, gap) in spec {
            offset += gap;
            sum += amount;
            flows.push(DatedCashflow::new(offset, Money::agnostic(amount).unwrap()).unwrap());
        }

        let npv = DatedCashflows::new(&flows)
            .net_present_value(Rate::<Annual>::new(0.0).unwrap())
            .unwrap()
            .value();
        prop_assert!(close(npv, sum, 1e-6));
    }

    /// The annuity present value is **non-increasing** in the discount rate:
    /// discount a positive stream harder and it is worth no more. This is the
    /// property `annuity::rate`'s rustdoc cites as the reason its residual has a
    /// single root, so it is the correctness argument for solving, not a nicety —
    /// and the literal `(1 - (1+r)⁻ⁿ)/r` broke it in a band around zero, where
    /// cancellation left the factor *rising* with the rate and the solver
    /// answering with the wrong sign (ADR-0054).
    ///
    /// The generated pair is deliberately allowed to be arbitrarily close, and the
    /// `1e-12` band around zero is sampled directly, because that is exactly where
    /// the closed form failed.
    #[test]
    fn the_annuity_present_value_is_non_increasing_in_the_rate(
        low in -0.9f64..2.0,
        gap in 0.0f64..2.0,
        periods in 1.0f64..600.0,
        payment in 1.0f64..1e6,
    ) {
        use time_value::{annuity, Period};

        let payment = Money::agnostic(payment).unwrap();
        let periods = Period::<Monthly>::new(periods).unwrap();
        let pv = |r: f64| {
            annuity::present_value(Rate::<Monthly>::new(r).unwrap(), periods, payment)
                .map(Money::value)
        };
        // A rate near −100% over a long term compounds the discount factor past
        // `f64::MAX`, which is an `Overflow` rather than a value to compare
        // (ADR-0021). Ordering is only claimed where both ends have one.
        if let (Ok(lower), Ok(higher)) = (pv(low), pv(low + gap)) {
            prop_assert!(lower >= higher, "PV rose from {lower} to {higher}");
        }
    }

    /// The same law, sampled where it actually broke: an interval straddling zero
    /// at the scale of the cancellation. Split out from the wide-range property so
    /// the generator cannot spend its budget on comfortable rates and miss the one
    /// decade that mattered.
    #[test]
    fn the_annuity_present_value_is_non_increasing_across_a_zero_rate(
        low in -1e-7f64..0.0,
        high in 0.0f64..1e-7,
        periods in 1.0f64..600.0,
    ) {
        use time_value::{annuity, Period};

        let payment = Money::agnostic(1000.0).unwrap();
        let periods = Period::<Monthly>::new(periods).unwrap();
        let pv = |r: f64| {
            annuity::present_value(Rate::<Monthly>::new(r).unwrap(), periods, payment)
                .unwrap()
                .value()
        };
        prop_assert!(pv(low) >= pv(0.0));
        prop_assert!(pv(0.0) >= pv(high));
    }

    /// Rounding a `Money` yields a `Money`, and every `Money` is finite (money.rs,
    /// lib.rs). The existing rounding properties bound the magnitude to `±1e9`,
    /// which is why they never saw `round_to_currency` scale a large amount to
    /// `inf` and write it straight into the struct: for `USD` the boundary is
    /// `f64::MAX / 100 ≈ 1.8e306` (ADR-0054). This one ranges over the entire
    /// exponent, so the boundary is inside the domain rather than outside it.
    #[test]
    fn rounding_any_representable_amount_stays_finite(
        mantissa in -10.0f64..10.0,
        exponent in -320i32..309,
        index in 0usize..time_value::Currency::ALL.len(),
    ) {
        let magnitude = mantissa * 10.0f64.powi(exponent);
        prop_assume!(magnitude.is_finite());

        let currency = time_value::Currency::ALL[index];
        let rounded = Money::new(magnitude, currency).unwrap().round_to_currency();
        prop_assert!(rounded.value().is_finite());
        prop_assert_eq!(rounded.currency(), currency);

        // Above `2^53` every `f64` is already an integer, so it is already an exact
        // multiple of any minor unit and rounding has nothing to do — which is what
        // makes returning the amount unchanged on overflow *exact* rather than a
        // fallback. (Scaling up and back can still cost a couple of ULP where the
        // scaling does happen, hence the relative band rather than equality; the
        // `rounding_a_huge_magnitude_stays_finite_and_unchanged` unit test pins the
        // overflow boundary itself exactly.)
        if !close(magnitude, 0.0, 9.1e15) {
            prop_assert!(close(rounded.value(), magnitude, 1e-12 * magnitude.abs()));
        }
    }
}

/// Every currency without a minor unit is returned unchanged by
/// [`Money::round_to_currency`] — `Xxx`, the precious metals, and the fund/testing
/// codes. The domain is a small finite set, so this iterates it exhaustively
/// rather than sampling (ADR-0045); a currency that gains or loses a minor-unit
/// exponent fails here.
#[cfg(any(feature = "std", feature = "libm"))]
#[test]
fn a_currency_without_a_minor_unit_is_never_rounded() {
    use time_value::Currency;

    let mut checked = 0;
    for &currency in Currency::ALL {
        if currency.minor_unit_exponent().is_some() {
            continue;
        }
        checked += 1;
        for magnitude in [0.0, 1234.5678, -0.999, 1e9 + 0.5] {
            let money = Money::new(magnitude, currency).unwrap();
            assert_eq!(
                money.round_to_currency(),
                money,
                "{currency:?} has no minor unit, so rounding must be the identity",
            );
        }
    }
    // The set is non-empty — `Xxx` alone guarantees it — so a filter that silently
    // matched nothing would not pass as a vacuous success.
    assert!(checked > 0, "no minor-unit-free currency found");
}

/// The owned series' `serde` wire format (ADR-0060) round-trips *every* series, not
/// just the worked examples in `tests/serde.rs`: any length (including empty), any
/// finite amount, any currency. The imports are inside the module so no other
/// feature configuration sees them.
///
/// **On exactness.** The format itself is lossless — `serde_json` writes the shortest
/// decimal that identifies the `f64` uniquely — but recovering the bits exactly is
/// the *deserializer's* job, and `serde_json`'s default float parser is best-effort
/// (its `float_roundtrip` feature, off by default, is what makes it exact). So these
/// properties assert recovery to within a few ULP rather than bit equality; the
/// point tests in `tests/serde.rs`, whose amounts are exactly representable, pin the
/// shape exactly. This caveat is not specific to the series — it applies to every
/// `f64` the wire format carries (ADR-0060).
#[cfg(all(feature = "serde", feature = "alloc"))]
mod owned_cashflows_wire {
    use super::close;
    use proptest::prelude::*;
    use time_value::{Currency, Money, Monthly, OwnedCashflows, Rate};

    /// Relative closeness, for comparing an amount with its round-tripped self.
    fn close_relative(a: f64, b: f64) -> bool {
        let scale = if a < 0.0 { -a } else { a };
        // A few ULP of the value's own magnitude (and an absolute floor at zero).
        close(a, b, 8.0 * f64::EPSILON * scale + f64::MIN_POSITIVE)
    }

    proptest! {
        /// Serializing a series and deserializing the result recovers it: the same
        /// number of flows, in the same order, in the same currency, each amount
        /// back to within the deserializer's float precision.
        #[test]
        fn serializing_then_deserializing_recovers_the_series(
            amounts in prop::collection::vec(-1e12f64..1e12, 0..=32),
            currency_index in 0usize..Currency::ALL.len(),
        ) {
            let currency = Currency::ALL[currency_index];
            let series = OwnedCashflows::<Monthly>::new(
                amounts.iter().map(|&a| Money::new(a, currency).unwrap()).collect(),
            );

            let document = serde_json::to_string(&series).unwrap();
            let back: OwnedCashflows<Monthly> = serde_json::from_str(&document).unwrap();

            prop_assert_eq!(back.len(), series.len());
            for (recovered, original) in back.as_slice().iter().zip(series.as_slice()) {
                prop_assert_eq!(recovered.currency(), original.currency());
                prop_assert!(
                    close_relative(recovered.value(), original.value()),
                    "{} != {}",
                    recovered.value(),
                    original.value(),
                );
            }
        }

        /// Round-tripping preserves the *behaviour*, not just the data: the series
        /// off the wire discounts to the same net present value.
        #[test]
        fn a_round_tripped_series_has_the_same_net_present_value(
            amounts in prop::collection::vec(-1e6f64..1e6, 1..=16),
            rate in 0.0f64..0.5,
        ) {
            let series = OwnedCashflows::<Monthly>::new(
                amounts.iter().map(|&a| Money::agnostic(a).unwrap()).collect(),
            );
            let back: OwnedCashflows<Monthly> =
                serde_json::from_str(&serde_json::to_string(&series).unwrap()).unwrap();

            let rate = Rate::<Monthly>::new(rate).unwrap();
            // Up to 16 discounted addends, each |·| ≤ 1e6, so this tolerance is far
            // above the round-tripping error and far below any real difference.
            prop_assert!(close(
                back.net_present_value(rate).unwrap().value(),
                series.net_present_value(rate).unwrap().value(),
                1e-6,
            ));
        }
    }
}
