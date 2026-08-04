//! Shared, target-agnostic test support. Not a test target itself: `cargo` only
//! auto-discovers `tests/*.rs`, so everything under `tests/common/` is a library
//! for the real targets.

pub mod golden;
