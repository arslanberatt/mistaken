//! `mistaken-bench`: Spec 05's local ASR benchmark harness library surface.
//! The binary (`src/main.rs`) is a thin CLI wrapper over these modules so
//! the scoring/validation/protocol logic is independently unit-testable
//! against committed fixtures (Spec 05 section 7, "Developer verification
//! flow").

pub mod adapter;
pub mod candidate;
pub mod manifest;
pub mod normalize;
pub mod report;
pub mod run_record;
pub mod scoring;
