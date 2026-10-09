//! Concrete harness adapters, one module per CLI.
//!
//! To add a harness, create `harnesses/<name>/` with a type implementing
//! [`crate::harness::Harness`], export it here and register it in
//! [`crate::harness::HarnessRegistry::with_defaults`].

pub mod omp;
pub mod pi;

pub use omp::Omp;
pub use pi::Pi;
