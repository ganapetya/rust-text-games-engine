//! One integration-test binary for the whole crate. Shared code: `tests/common/`. Scenarios: `tests/<scenario>/mod.rs`
//! (directory + `mod.rs` avoids Cargo auto-picking `tests/foo.rs` as a second test crate).
//!
//! Run: `cargo test -p shakti-game-engine --test integration_test`

mod common;
mod gap_fill_full_session;
