//! # Adapted from Root-io (https://github.com/cbourjau/alice-rs/)
//! - updates ttree, tbranch to work with versions >=20, >=13, respectively
//! - removes dependencies on reqwest, tokio, ... for remote file fetching
#![allow(clippy::cognitive_complexity)]
#![recursion_limit = "256"]
#[macro_use]
extern crate bitflags;
extern crate nom;
#[macro_use]
extern crate quote;
#[macro_use]
extern crate failure;
extern crate flate2;
extern crate lzma_rs;

// pub mod core_types;
mod code_gen;
pub mod core;
pub mod test_utils;
mod tests;
pub mod tree_reader;

// anyblox-specific
pub mod anyblox;

pub use crate::core::{FileItem, RootFile, Source};

/// Offset when using Context; should be in `Context`, maybe?
const MAP_OFFSET: u64 = 2;
