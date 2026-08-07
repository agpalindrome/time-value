#![doc = include_str!("../README.md")]

pub mod amount;
pub mod error;
pub mod factor;
pub mod periods;
pub mod rate;

// The arithmetic, over `f64`. Private now that the typed surface calls it: an
// untyped API alongside the typed one would offer the same operations with none
// of the invariants, which is the opposite of the point.
mod simple;

pub use amount::Amount;
pub use error::{Error, Result};
pub use factor::{SimpleAccumulationFactor, future_value};
pub use periods::ElapsedPeriods;
pub use rate::SimpleInterestRate;
