use super::utils::lookup_of_account;
use crate::{AccountId, Balance, CurrencyId, Runtime, Tokens};

use sp_runtime::traits::SaturatedConversion;
use sp_std::prelude::*;

use frame_benchmarking::account;
use frame_system::RawOrigin;

use orml_benchmarking::runtime_benchmarks;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};

const SEED: u32 = 0;
const MAX_USER_INDEX: u32 = 1000;

fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	d.saturating_mul(1_000_000_000_000_000_000)
}

runtime_benchmarks! {
	{ Runtime, orml_tokens }

	_ {
		let u in 1 .. MAX_USER_INDEX => ();
	}

	// `transfer`
	transfer {
		let u in ...;

		let from = account("from", u, SEED);
		let to: AccountId = account("to", u, SEED);
		let to_lookup = lookup_of_account(to.clone());
		let amount = dollar(u);

		let _ = <Tokens as MultiCurrencyExtended<_>>::update_balance(CurrencyId::AUSD, &from, amount.saturated_into());
	}: _(RawOrigin::Signed(from), to_lookup, CurrencyId::AUSD, amount)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(CurrencyId::AUSD, &to), amount);
	}

	// `transfer_all`
	transfer_all {
		let u in ...;

		let from = account("from", u, SEED);
		let to: AccountId = account("to", u, SEED);
		let to_lookup = lookup_of_account(to);
		let amount = dollar(u);

		let _ = <Tokens as MultiCurrencyExtended<_>>::update_balance(CurrencyId::AUSD, &from, amount.saturated_into());
	}: _(RawOrigin::Signed(from.clone()), to_lookup, CurrencyId::AUSD)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(CurrencyId::AUSD, &from), 0);
	}
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
	fn transfer() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_transfer());
		});
	}

	#[test]
	fn transfer_all() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_transfer_all());
		});
	}
}
