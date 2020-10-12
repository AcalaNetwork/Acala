use crate::{AccountId, AllNonNativeCurrencyIds, Balance, GetNativeCurrencyId, Runtime, DOLLARS};

use super::utils::set_balance;
use frame_benchmarking::account;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use sp_std::prelude::*;

const SEED: u32 = 0;

fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	DOLLARS.saturating_mul(d)
}

runtime_benchmarks! {
	{ Runtime, module_accounts }

	_ {}

	close_account {
		let c in 0 .. AllNonNativeCurrencyIds::get().len().saturating_sub(1) as u32;
		let currency_ids = AllNonNativeCurrencyIds::get();
		let caller: AccountId = account("caller", 0, SEED);
		let native_currency_id = GetNativeCurrencyId::get();
		set_balance(native_currency_id, &caller, dollar(1000));

		for i in 0 .. c {
			let currency_id = currency_ids[i as usize];
			set_balance(currency_id, &caller, dollar(1000));
		}
	}: _(RawOrigin::Signed(caller), None)
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::assert_ok;

	fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap()
			.into()
	}

	#[test]
	fn test_close_account() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_close_account());
		});
	}
}
