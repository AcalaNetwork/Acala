use super::utils::{lookup_of_account, set_ausd_balance};
use crate::{AccountId, Balance, CurrencyId, Runtime, TokenSymbol, Tokens, DOLLARS};

use sp_std::prelude::*;

use frame_benchmarking::account;
use frame_system::RawOrigin;

use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, orml_tokens }

	_ {
		let d in 1 .. MAX_DOLLARS => ();
	}

	transfer {
		let amount: Balance = DOLLARS.saturating_mul(d);

		let from = account("from", 0, SEED);
		set_ausd_balance(&from, amount);

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: _(RawOrigin::Signed(from), to_lookup, CurrencyId::Token(TokenSymbol::AUSD), amount)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(CurrencyId::Token(TokenSymbol::AUSD), &to), amount);
	}

	transfer_all {
		let amount: Balance = DOLLARS.saturating_mul(d);

		let from = account("from", 0, SEED);
		set_ausd_balance(&from, amount);

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to);
	}: _(RawOrigin::Signed(from.clone()), to_lookup, CurrencyId::Token(TokenSymbol::AUSD))
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(CurrencyId::Token(TokenSymbol::AUSD), &from), 0);
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
