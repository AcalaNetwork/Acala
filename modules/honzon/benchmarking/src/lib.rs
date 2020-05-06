//! Benchmarks for the honzon module.
// This is separated into its own crate due to cyclic dependency issues.

#![cfg_attr(not(feature = "std"), no_std)]

mod mock;

use sp_std::prelude::*;
use sp_std::vec;

// use frame_system::RawOrigin;
// use frame_benchmarking::{account, benchmarks};
// use frame_support::traits::Get;
// use sp_runtime::traits::UniqueSaturatedInto;
// use sp_std::prelude::*;

// use module_honzon::*;
// use module_honzon::Module as Honzon;
// use module_cdp_engine::Module as CdpEngine;
// use module_loans::Module as Loans;
// use orml_oracle::Module as Oracle;

// pub struct Module<T: Trait>(pallet_session::Module<T>);

// pub trait Trait: module_honzon::Trait + orml_oracle::Trait {}
