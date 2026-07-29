//! Robust one-dimensional root finding, shared across the crate.
//!
//! The bracketing bisection here is arithmetic-only (no transcendental math), so
//! it lives in the default `no_std`, zero-dependency build alongside its first
//! caller, [`Cashflows::internal_rate_of_return`](crate::Cashflows). The
//! transcendental solve-for-rate operations (`annuity::rate`) reuse the same
//! bracketing fallback (`docs/adr/0020-robust-irr-newton-with-bisection-fallback.md`,
//! `docs/adr/0025-solve-for-periods-and-rate.md`).

/// `|x| < tolerance`, without `f64::abs` (which is not in `core`).
pub(crate) fn within(x: f64, tolerance: f64) -> bool {
    x < tolerance && x > -tolerance
}

/// `|x|`, without `f64::abs` (which is not in `core`).
pub(crate) fn abs(x: f64) -> f64 {
    if x < 0.0 {
        -x
    } else {
        x
    }
}

/// Whether `a` and `b` are both non-zero and of opposite sign.
pub(crate) fn opposite_signs(a: f64, b: f64) -> bool {
    (a < 0.0 && b > 0.0) || (a > 0.0 && b < 0.0)
}

/// One sample of the residual a solver is driving to zero: its `value` at a
/// candidate rate, together with the `scale` that decides whether that value
/// counts as zero **at that same rate**.
///
/// The scale is the sum of the magnitudes of the terms that were combined to
/// produce the value — `Σ|CFₜ|·(1 + r)⁻ᵗ` for an NPV, `|PMT·a(r,n)| + |target|`
/// for a solve-for-rate. Pairing the two is the whole point: a residual is only
/// meaningfully zero relative to what went into it, and what goes into a
/// discounted sum shrinks as the rate rises (ADR-0054).
#[derive(Clone, Copy)]
pub(crate) struct Residual {
    pub(crate) value: f64,
    pub(crate) scale: f64,
}

impl Residual {
    /// Whether this sample is a root: its value is negligible against the terms
    /// that produced it.
    ///
    /// The tolerance is relative to [`scale`](Self::scale) *at this rate*, not to
    /// a scale fixed in advance from the raw inputs. The fixed form — `Σ|CFₜ|`,
    /// floored at `1` — was the ADR-0020/0021 original and it accepted nonsense:
    /// as `r → ∞` every discounted term vanishes and the NPV tends to `CF₀`, so a
    /// series whose first cashflow is smaller than the tolerance made *every*
    /// sufficiently large rate a "root". On `[0, 0, −100, 110]`, whose only IRR is
    /// exactly `0.1`, that returned rates in the millions of percent. Judging the
    /// residual against the terms present at the same rate removes the loophole
    /// without bounding the search: the ratio tends to `±1` there, never to `0`
    /// (ADR-0054).
    ///
    /// A non-finite scale would make the tolerance infinite — accepting
    /// everything — so it is rejected outright. A `NaN` value fails
    /// [`within`](self::within) and so is never a root.
    pub(crate) fn is_root(self) -> bool {
        /// Residual magnitude, as a fraction of the scale, below which a sample
        /// counts as a root.
        const RELATIVE: f64 = 1e-9;

        self.scale.is_finite() && within(self.value, RELATIVE * self.scale)
    }
}

/// Newton–Raphson from `guess` for a residual in the per-period rate `r`, given a
/// closure returning the [`Residual`] and its derivative `d(value)/dr` at a
/// candidate `r`.
///
/// `None` if it does not reach a root within its iteration budget, the derivative
/// goes flat, or an iterate leaves the valid rate domain (`r ≤ −1`, or a
/// non-finite value — `is_finite` also rejects `NaN`, so a diverging iterate fails
/// cleanly rather than looping). Callers pair this with [`bracket_and_bisect`] as a
/// robust fallback (ADR-0020).
pub(crate) fn newton(
    residual_and_derivative: impl Fn(f64) -> (Residual, f64),
    guess: f64,
) -> Option<f64> {
    const MAX_ITERATIONS: u32 = 128;
    const MIN_DERIVATIVE: f64 = 1e-12;

    let mut rate = guess;
    for _ in 0..MAX_ITERATIONS {
        if !rate.is_finite() || rate <= -1.0 {
            return None;
        }
        let (residual, derivative) = residual_and_derivative(rate);
        if residual.is_root() {
            return Some(rate);
        }
        if within(derivative, MIN_DERIVATIVE) {
            return None;
        }
        rate -= residual.value / derivative;
    }
    None
}

/// Bisect for the root of `f` in `[lo, hi]`, where `f` has opposite signs at the
/// ends (`f_lo` is `f(lo)`). Returns as soon as a sample is a root, or the
/// midpoint after `max` steps.
fn bisect(
    f: impl Fn(f64) -> Residual,
    mut lo: f64,
    mut hi: f64,
    mut f_lo: Residual,
    max: u32,
) -> f64 {
    for _ in 0..max {
        let mid = 0.5 * (lo + hi);
        let f_mid = f(mid);
        if f_mid.is_root() {
            return mid;
        }
        if opposite_signs(f_lo.value, f_mid.value) {
            hi = mid;
        } else {
            lo = mid;
            f_lo = f_mid;
        }
    }
    0.5 * (lo + hi)
}

/// Scan the valid rate domain (`r > −1`) for a sign change in `f` and bisect the
/// first bracket found, returning the lowest bracketed root. `None` if `f` never
/// changes sign over the scan (no root).
///
/// `f` is a residual in the per-period rate `r` — for the IRR it is the NPV, for a
/// solve-for-rate it is `value(r) − target`. `1 + r` is sampled geometrically from
/// just above `0` upward, a ratio fine enough not to step over a lone root of a
/// monotone residual.
pub(crate) fn bracket_and_bisect(f: impl Fn(f64) -> Residual) -> Option<f64> {
    const MAX_BISECTIONS: u32 = 200;
    const START: f64 = 1e-4; // 1 + r, i.e. r = -0.9999
    const RATIO: f64 = 1.25;
    const SAMPLES: u32 = 160; // reaches 1 + r ≈ 1e15

    let mut lo = START - 1.0;
    let mut f_lo = f(lo);
    let mut growth = START;
    for _ in 0..SAMPLES {
        if f_lo.is_root() {
            return Some(lo);
        }
        growth *= RATIO;
        let hi = growth - 1.0;
        let f_hi = f(hi);
        if opposite_signs(f_lo.value, f_hi.value) {
            return Some(bisect(&f, lo, hi, f_lo, MAX_BISECTIONS));
        }
        lo = hi;
        f_lo = f_hi;
    }
    None
}
