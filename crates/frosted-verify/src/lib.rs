//! Independent verifier for Frosted (docs/PROJECT_SPEC_FROSTED.md F-12, US-16, US-17).
//!
//! It reads only your own node and the public API, recomputes every claim with `zcash-glass`,
//! and reports PASS/FAIL per check. The same code runs behind `GET /v1/markets/{id}/verify`,
//! which is a convenience that you should not trust: run the CLI against your own node.

pub mod checks;
pub mod cli;
pub mod report;
pub mod rpc;
pub mod source;

pub use checks::{Command, ProofFile, ThawOutcome, envelope_proof, run, thaw};
pub use report::{Check, Diff, Report, Status};
pub use source::{Api, Node, Note, Registration, Scan};
