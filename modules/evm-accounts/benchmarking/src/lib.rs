//! Benchmarks for the evm-accounts module.
// This is separated into its own crate due to cyclic dependency issues.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(all(feature = "runtime-benchmarks", test))]
mod mock;

