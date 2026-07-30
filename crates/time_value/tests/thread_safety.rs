//! The thread-safety contract (ADR-0046): the owned public value types are
//! `Send + Sync + 'static`, and the borrowing views are `Send + Sync`.
//!
//! All hold trivially today — the types are plain data with no interior
//! mutability — so this test is green immediately. Its job is to *lock* the
//! profile: it fails to compile the moment a field regresses it (e.g. an `Rc` or
//! `Cell` added to a public type silently dropping `Sync`), turning an invisible
//! semver break into a build error. The lock is zero-cost and zero-dependency.
//!
//! The periodicity tag is `PhantomData`, so one representative marker witnesses
//! the whole family. Assertions are feature-gated to mirror each type's own gate
//! (`alloc` for `OwnedCashflows`; `std`/`libm` for the transcendental-math types).

use time_value::*;

fn assert_send_sync_static<T: Send + Sync + 'static>() {}
fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn owned_value_types_are_send_sync_static() {
    // Always available (default `no_std`, zero-dep build).
    assert_send_sync_static::<Money>();
    assert_send_sync_static::<Currency>();
    assert_send_sync_static::<FxRate>();
    assert_send_sync_static::<TvmError>();
    assert_send_sync_static::<Rate<Annual>>();
    assert_send_sync_static::<Installment>();
    assert_send_sync_static::<Schedule<Annual>>();

    // Periodicity markers.
    assert_send_sync_static::<Annual>();
    assert_send_sync_static::<SemiAnnual>();
    assert_send_sync_static::<Quarterly>();
    assert_send_sync_static::<Monthly>();
    assert_send_sync_static::<Weekly>();
    assert_send_sync_static::<Daily>();

    // Owned, behind `alloc`.
    #[cfg(feature = "alloc")]
    assert_send_sync_static::<OwnedCashflows<Annual>>();

    // Owned, behind the transcendental-math feature (`std` / `libm`).
    #[cfg(any(feature = "std", feature = "libm"))]
    {
        assert_send_sync_static::<Period<Monthly>>();
        assert_send_sync_static::<ContinuousRate>();
        assert_send_sync_static::<DatedCashflow>();

        // The owned dated series needs `alloc` on top of the math feature.
        #[cfg(feature = "alloc")]
        assert_send_sync_static::<OwnedDatedCashflows>();
    }
}

#[test]
fn borrowing_views_are_send_sync() {
    // The views borrow with some lifetime `'a` (witnessed by the slice parameter);
    // they are `Send + Sync` for any `'a` — their `Sync` rides on `Money: Sync` —
    // independent of `'static`.
    fn check<'a>(_witness: &'a [Money]) {
        assert_send_sync::<Cashflows<'a, Annual>>();
        #[cfg(any(feature = "std", feature = "libm"))]
        assert_send_sync::<DatedCashflows<'a>>();
    }
    check(&[]);
}
