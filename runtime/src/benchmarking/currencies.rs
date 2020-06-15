use super::utils::{lookup_of_account, set_balance};
use crate::{AcaExistentialDeposit, AccountId, Amount, Balance, Currencies, CurrencyId, Runtime, DOLLARS};

use sp_std::prelude::*;

use frame_benchmarking::account;
use frame_system::RawOrigin;
use sp_runtime::traits::UniqueSaturatedInto;

use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;

const SEED: u32 = 0;
const MAX_USER_INDEX: u32 = 1000;
const MAX_DOLLARS: u32 = 1000;
const MAX_ACA_EXISTENTIAL_DEPOSIT: u32 = 1000;

runtime_benchmarks! {
	{ Runtime, orml_currencies }

	_ {
		let u in 1 .. MAX_USER_INDEX => ();
		let d in 1 .. MAX_DOLLARS => ();
		let e in 2 .. MAX_ACA_EXISTENTIAL_DEPOSIT => ();
	}

	// `transfer` non-native currency
	transfer_non_native_currency {
		let u in ...;
		let d in ...;

		let amount: Balance = DOLLARS.saturating_mul(d.into());
		let currency_id = CurrencyId::DOT;
		let from = account("from", u, SEED);
		set_balance(currency_id, &from, amount);

		let to: AccountId = account("to", u, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: transfer(RawOrigin::Signed(from), to_lookup, currency_id, amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(currency_id, &to), amount);
	}

	// `transfer` native currency and in worst case
	transfer_native_currency_worst_case {
		let u in ...;
		let e in ...;

		let aca_existential_deposit = AcaExistentialDeposit::get();
		let amount: Balance = aca_existential_deposit.saturating_mul(e.into());
		let native_currency_id = CurrencyId::ACA;
		let from = account("from", u, SEED);
		set_balance(native_currency_id, &from, amount);

		let to: AccountId = account("to", u, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: transfer(RawOrigin::Signed(from), to_lookup, native_currency_id, amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(native_currency_id, &to), amount);
	}

	// `transfer_native_currency` in worst case
	// * will create the `to` account.
	// * will kill the `from` account.
	transfer_native_currency {
		let u in ...;
		let e in ...;

		let aca_existential_deposit = AcaExistentialDeposit::get();
		let amount: Balance = aca_existential_deposit.saturating_mul(e.into());
		let native_currency_id = CurrencyId::ACA;
		let from = account("from", u, SEED);
		set_balance(native_currency_id, &from, amount);

		let to: AccountId = account("to", u, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: _(RawOrigin::Signed(from), to_lookup, amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(native_currency_id, &to), amount);
	}

	// `update_balance` for non-native currency
	update_balance_non_native_currency {
		let u in ...;
		let d in ...;

		let balance: Balance = DOLLARS.saturating_mul(d.into());
		let amount: Amount = balance.unique_saturated_into();
		let currency_id = CurrencyId::DOT;
		let who: AccountId = account("who", u, SEED);
		let who_lookup = lookup_of_account(who.clone());
	}: update_balance(RawOrigin::Root, who_lookup, currency_id, amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(currency_id, &who), balance);
	}

	// `update_balance` for native currency
	// * will create the `who` account.
	update_balance_native_currency_creating {
		let u in ...;
		let e in ...;

		let aca_existential_deposit = AcaExistentialDeposit::get();
		let balance: Balance = aca_existential_deposit.saturating_mul(e.into());
		let amount: Amount = balance.unique_saturated_into();
		let native_currency_id = CurrencyId::ACA;
		let who: AccountId = account("who", u, SEED);
		let who_lookup = lookup_of_account(who.clone());
	}: update_balance(RawOrigin::Root, who_lookup, native_currency_id, amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(native_currency_id, &who), balance);
	}

	// `update_balance` for native currency
	// * will kill the `who` account.
	update_balance_native_currency_killing {
		let u in ...;
		let e in ...;

		let aca_existential_deposit = AcaExistentialDeposit::get();
		let balance: Balance = aca_existential_deposit.saturating_mul(e.into());
		let amount: Amount = balance.unique_saturated_into();
		let native_currency_id = CurrencyId::ACA;
		let who: AccountId = account("who", u, SEED);
		let who_lookup = lookup_of_account(who.clone());
		set_balance(native_currency_id, &who, balance);
	}: update_balance(RawOrigin::Root, who_lookup, native_currency_id, -amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(native_currency_id, &who), 0);
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
	fn transfer_non_native_currency() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_transfer_non_native_currency());
		});
	}

	#[test]
	fn transfer_native_currency_worst_case() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_transfer_native_currency_worst_case());
		});
	}

	#[test]
	fn update_balance_non_native_currency() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_update_balance_non_native_currency());
		});
	}

	#[test]
	fn update_balance_native_currency_creating() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_update_balance_native_currency_creating());
		});
	}

	#[test]
	fn update_balance_native_currency_killing() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_update_balance_native_currency_killing());
		});
	}
}
