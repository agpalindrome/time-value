#![doc = include_str!("../README.md")]

pub mod error;

// Public only until the typed surface exists to call it. Made private in the
// commit that adds the types: with no caller but tests, a private module trips
// both `dead_code` and `unreachable_pub` under the denied `unused` group.
pub mod simple;
