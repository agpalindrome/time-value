//! Property-based tests over the public numeric API.
//!
//! These assert the *laws* the operations obey for whole families of inputs,
//! rather than a handful of worked examples: present and future value invert
//! each other, net present value is monotone in the discount rate and collapses
//! to the plain sum at a zero rate, the internal rate of return zeroes the net
//! present value, the annuity payment inverts the annuity present value (for both
//! ordinary and annuity-due), an amortization schedule conserves the principal it
//! repays, currency rounding is idempotent, the dated XNPV agrees with the
//! periodic NPV on whole-year offsets, and `Money`'s arithmetic obeys the usual
//! algebraic laws.
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
/// [`TvmError::Undefined`](time_value::TvmError::Undefined), not a schedule.
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
                installment.interest.value() + installment.principal.value(),
                installment.payment.value(),
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
                installment.interest,
                Money::agnostic(opening.value() * rate).unwrap()
            );
            prop_assert_eq!(
                installment.balance,
                opening.try_sub(installment.principal).unwrap()
            );
            opening = installment.balance;
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
            repaid += installment.principal.value();
            final_balance = Some(installment.balance);
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
            prop_assert!(installment.balance.value() < previous.value());
            previous = installment.balance;
        }
        prop_assert_eq!(previous, Money::ZERO);
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
            repaid += installment.principal.value();
            final_balance = Some(installment.balance);
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
