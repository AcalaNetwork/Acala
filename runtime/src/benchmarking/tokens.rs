use super::utils::lookup_of_account;
use crate::{AccountId, CurrencyId, Runtime, Tokens};

use sp_runtime::traits::SaturatedConversion;
use sp_std::prelude::*;

use frame_benchmarking::account;
use frame_system::RawOrigin;

use orml_benchmarking::runtime_benchmarks;
use orml_tokens::Trait as TokensTrait;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};

const SEED: u32 = 0;
const MAX_EXISTENTIAL_DEPOSIT: u32 = 1000;
const MAX_USER_INDEX: u32 = 1000;

runtime_benchmarks! {
	{ Runtime, orml_tokens }

	_ {
		let u in 1 .. MAX_USER_INDEX => ();
		let e in 2 .. MAX_EXISTENTIAL_DEPOSIT => ();
	}

	// `transfer` worst case:
	// - Sender account would be killed.
	// - Recipient account would be created.
	transfer {
		let u in ...;
		let e in ...;

		let existential_deposit = <Runtime as TokensTrait>::ExistentialDeposit::get();
		let from = account("from", u, SEED);

		let balance = existential_deposit.saturating_mul(e.into());
		let _ = <Tokens as MultiCurrencyExtended<_>>::update_balance(CurrencyId::AUSD, &from, balance.saturated_into());

		let to: AccountId = account("to", u, SEED);
		let to_lookup = lookup_of_account(to.clone());
		let amount = existential_deposit.saturating_mul((e - 1).into()) + 1;
	}: _(RawOrigin::Signed(from), to_lookup, CurrencyId::AUSD, amount)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(CurrencyId::AUSD, &to), amount);
	}

	// `transfer` best case:
	// - Both accounts exist and would continue to exist.
	transfer_best_case {
		let u in ...;
		let e in ...;

		let existential_deposit = <Runtime as TokensTrait>::ExistentialDeposit::get();
		let from = account("from", u, SEED);

		let balance = existential_deposit.saturating_mul(e.into());
		let _ = <Tokens as MultiCurrencyExtended<_>>::update_balance(CurrencyId::AUSD, &from, balance.saturated_into());

		let to: AccountId = account("to", u, SEED);
		let _ = <Tokens as MultiCurrencyExtended<_>>::update_balance(CurrencyId::AUSD, &to, existential_deposit.saturated_into());

		let to_lookup = lookup_of_account(to);
		let amount = existential_deposit.saturating_mul((e - 1).into());
	}: transfer(RawOrigin::Signed(from), to_lookup, CurrencyId::AUSD, amount)

	// `transfer_all` worst case:
	// - Recipient account would be created.
	transfer_all {
		let u in ...;
		let e in ...;

		let existential_deposit = <Runtime as TokensTrait>::ExistentialDeposit::get();
		let from = account("from", u, SEED);

		let balance = existential_deposit.saturating_mul(e.into());
		let _ = <Tokens as MultiCurrencyExtended<_>>::update_balance(CurrencyId::AUSD, &from, balance.saturated_into());

		let to: AccountId = account("to", u, SEED);
		let to_lookup = lookup_of_account(to);
	}: _(RawOrigin::Signed(from.clone()), to_lookup, CurrencyId::AUSD)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(CurrencyId::AUSD, &from), 0);
	}

	// `transfer_all` best case:
	// - Recipient account exists.
	transfer_all_best_case {
		let u in ...;
		let e in ...;

		let existential_deposit = <Runtime as TokensTrait>::ExistentialDeposit::get();
		let from = account("from", u, SEED);

		let balance = existential_deposit.saturating_mul(e.into());
		let _ = <Tokens as MultiCurrencyExtended<_>>::update_balance(CurrencyId::AUSD, &from, balance.saturated_into());

		let to: AccountId = account("to", u, SEED);
		let _ = <Tokens as MultiCurrencyExtended<_>>::update_balance(CurrencyId::AUSD, &to, existential_deposit.saturated_into());
		let to_lookup = lookup_of_account(to);
	}: transfer_all(RawOrigin::Signed(from.clone()), to_lookup, CurrencyId::AUSD)
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
	fn transfer_best_case() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_transfer_best_case());
		});
	}

	#[test]
	fn transfer_all() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_transfer_all());
		});
	}

	#[test]
	fn transfer_all_best_case() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_transfer_all_best_case());
		});
	}
}
