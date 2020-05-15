//! Benchmarks for the emergency shutdown module.
// This is separated into its own crate due to cyclic dependency issues.

#![cfg_attr(not(feature = "std"), no_std)]

mod mock;

use sp_std::prelude::*;
use sp_std::vec;

use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Get;
use frame_system::{self as system, RawOrigin};
use sp_runtime::traits::UniqueSaturatedInto;

use emergency_shutdown::Module as EmergencyShutdown;
use emergency_shutdown::*;
use orml_oracle::OperatorProvider;
use orml_traits::{DataProviderExtended, MultiCurrencyExtended};
use primitives::{Amount, Balance, CurrencyId};
use support::{Price, Rate, Ratio};

pub struct Module<T: Trait>(emergency_shutdown::Module<T>);

pub trait Trait: emergency_shutdown::Trait + orml_oracle::Trait + prices::Trait {}

const SEED: u32 = 0;

benchmarks! {
	_ { }

	call_emergency_shutdown {
		let u in 0 .. 1000;
	}: emergency_shutdown(RawOrigin::Root)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Runtime};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_call_emergency_shutdown::<Runtime>());
		});
	}
}
