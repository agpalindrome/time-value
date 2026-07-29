//! Role newtypes — which *argument* a value is, where two of them could be
//! transposed.
//!
//! A handful of operations take two adjacent arguments of the *same* type — two
//! [`Money`] amounts, or two [`Rate<P>`]s — and swapping them still compiles and
//! still returns a plausible number. `annuity::periods(rate, payment, present)`
//! answers "12 payments" for a year of level repayments and "0.089 payments" with
//! the two amounts the other way round; nothing in the type system objects, and
//! neither number carries a warning.
//!
//! These newtypes name the **role** each argument plays, so the transposition
//! becomes a compile error like the periodicity mismatch already is
//! (`docs/adr/0050-role-newtypes-for-ambiguous-arguments.md`). They are applied
//! only where a swap is expressible: an operation with a single [`Money`]
//! argument keeps taking a plain [`Money`], because there is nothing there to
//! confuse it with (ADR-0045's boundary — a newtype that catches no real failure
//! mode is not an improvement).
//!
//! They are **pure markers with no validation**. The inner value was already
//! validated by its own fallible constructor ([`Money::new`], [`Rate::new`]), so
//! wrapping it neither re-checks nor changes anything; the wrapper is zero-cost
//! and exists solely to distinguish one argument position from another.
//!
//! # Construction is explicit, extraction is free
//!
//! Wrapping is deliberately written out at the call site — `Payment(amount)` —
//! and there is **no** `From<Money>` impl (nor `From<Rate<P>>` for [`Growth`]).
//! A blanket conversion would make both argument orders compile again, which is
//! precisely what these types exist to prevent. Unwrapping is unrestricted: the
//! field is public, [`From`] converts into the inner type, and an inherent
//! [`money`](Payment::money) / [`rate`](Growth::rate) accessor reads it inline.
//!
//! ```
//! use time_value::{amortization::Schedule, Money, Monthly, Payment, Principal, Rate};
//!
//! let mut schedule = Schedule::with_payment(
//!     Rate::<Monthly>::new(0.10)?,
//!     Payment(Money::agnostic(500.0)?),
//!     Principal(Money::agnostic(1000.0)?),
//! )?;
//! assert!((schedule.next().unwrap().interest.value() - 100.0).abs() < 1e-9);
//! # Ok::<(), time_value::TvmError>(())
//! ```
//!
//! There is no way in, other than the constructor: a bare [`Money`] is not a
//! [`Payment`], and no `.into()` will make it one.
//!
//! ```compile_fail
//! use time_value::{amortization::Schedule, Money, Monthly, Principal, Rate};
//!
//! let _ = Schedule::with_payment(
//!     Rate::<Monthly>::new(0.10).unwrap(),
//!     Money::agnostic(500.0).unwrap().into(), // no `From<Money> for Payment` exists
//!     Principal(Money::agnostic(1000.0).unwrap()),
//! );
//! ```
//!
//! [`Rate<P>`]: Rate

use crate::{Money, Periodicity, Rate};

/// Defines a [`Money`] role newtype: a transparent wrapper that names which
/// argument an amount is, with extraction but deliberately no injection.
macro_rules! money_role {
    ($(#[$doc:meta])* $name:ident, $article:literal) => {
        $(#[$doc])*
        ///
        /// A pure role marker over an already-validated [`Money`] — it performs no
        /// validation of its own. Construct it explicitly (there is no
        /// `From<Money>`, by design); read it back with
        #[doc = concat!("[`money`](", stringify!($name), "::money), [`From`], or the public field.")]
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct $name(
            /// The amount playing this role.
            pub Money,
        );

        impl $name {
            #[doc = concat!("The amount ", $article, " denotes, as a plain [`Money`].")]
            #[must_use]
            pub const fn money(self) -> Money {
                self.0
            }
        }

        #[doc = concat!("Unwraps ", $article, " to the [`Money`] it wraps. The reverse conversion is")]
        /// deliberately absent: an implicit `Money` → role would let a transposed
        /// call compile again (ADR-0050).
        impl From<$name> for Money {
            fn from(role: $name) -> Self {
                role.0
            }
        }
    };
}

money_role!(
    /// A **payment**: the amount paid each period of an annuity or a loan.
    Payment,
    "a `Payment`"
);
money_role!(
    /// A **present value**: an amount valued as of today.
    PresentValue,
    "a `PresentValue`"
);
money_role!(
    /// A **future value**: an amount valued as of the end of the horizon.
    FutureValue,
    "a `FutureValue`"
);
money_role!(
    /// A **principal**: the balance a loan starts from and amortises away.
    Principal,
    "a `Principal`"
);

/// A **growth rate**: the per-period rate at which a payment *grows*, as opposed
/// to the discount [`Rate<P>`] it is priced at.
///
/// The two share the periodicity `P` — a monthly rate with an annual growth is
/// already a compile error — but before this newtype they shared their *type*
/// too, so `growing_present_value(rate, growth, …)` and
/// `growing_present_value(growth, rate, …)` both compiled and returned different
/// numbers (979.32 against 1386.73 for the same inputs). Tagging the second
/// position makes the swap a compile error (ADR-0050).
///
/// A pure role marker over an already-validated [`Rate<P>`] — it performs no
/// validation of its own. Construct it explicitly (there is no `From<Rate<P>>`,
/// by design); read it back with [`rate`](Growth::rate), [`From`], or the public
/// field.
///
/// ```
/// use time_value::{annuity, Growth, Money, Monthly, Period, Rate};
///
/// // First payment 100 at month end, growing 2%/month for a year, at 5%/month.
/// let pv = annuity::growing_present_value(
///     Rate::<Monthly>::new(0.05)?,
///     Growth(Rate::new(0.02)?),
///     Period::new(12.0)?,
///     Money::agnostic(100.0)?,
/// )?;
/// assert!((pv.value() - 979.318).abs() < 1e-2);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// [`Rate<P>`]: Rate
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Growth<P: Periodicity>(
    /// The per-period growth rate.
    pub Rate<P>,
);

impl<P: Periodicity> Growth<P> {
    /// The growth rate as a plain [`Rate<P>`](Rate).
    #[must_use]
    pub const fn rate(self) -> Rate<P> {
        self.0
    }

    /// The per-period growth as a plain `f64` — shorthand for
    /// `self.rate().value()`.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0.value()
    }
}

/// Unwraps a `Growth<P>` to the [`Rate<P>`](Rate) it wraps. The reverse
/// conversion is deliberately absent: an implicit `Rate<P>` → `Growth<P>` would
/// let a transposed call compile again (ADR-0050).
impl<P: Periodicity> From<Growth<P>> for Rate<P> {
    fn from(growth: Growth<P>) -> Self {
        growth.0
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use crate::{FutureValue, Growth, Money, Monthly, Payment, PresentValue, Principal, Rate};

    /// The wrappers are transparent: what goes in comes back out unchanged, by
    /// every route (field, accessor, `From`). They validate nothing, so there is
    /// nothing else for them to do.
    #[test]
    fn money_roles_are_transparent() {
        let amount = Money::agnostic(123.45).unwrap();
        assert_eq!(Payment(amount).0, amount);
        assert_eq!(Payment(amount).money(), amount);
        assert_eq!(Money::from(Payment(amount)), amount);
        assert_eq!(Money::from(PresentValue(amount)), amount);
        assert_eq!(Money::from(FutureValue(amount)), amount);
        assert_eq!(Money::from(Principal(amount)), amount);
    }

    #[test]
    fn growth_is_transparent() {
        let r = Rate::<Monthly>::new(0.02).unwrap();
        assert_eq!(Growth(r).0, r);
        assert_eq!(Growth(r).rate(), r);
        assert_eq!(Growth(r).value(), 0.02);
        assert_eq!(Rate::from(Growth(r)), r);
    }

    /// A role wrapper carries no state of its own, so it costs nothing: each is
    /// the size of what it wraps.
    #[test]
    fn the_wrappers_are_zero_cost() {
        use core::mem::size_of;
        assert_eq!(size_of::<Payment>(), size_of::<Money>());
        assert_eq!(size_of::<PresentValue>(), size_of::<Money>());
        assert_eq!(size_of::<FutureValue>(), size_of::<Money>());
        assert_eq!(size_of::<Principal>(), size_of::<Money>());
        assert_eq!(size_of::<Growth<Monthly>>(), size_of::<Rate<Monthly>>());
    }
}
