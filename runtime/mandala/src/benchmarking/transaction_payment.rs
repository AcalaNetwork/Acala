use crate::{AccountId, CurrencyId, Runtime, System, TokenSymbol, TransactionPayment};
use frame_benchmarking::account;
use frame_support::traits::OnFinalize;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use sp_std::prelude::*;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, module_transaction_payment }

	_ {}

	set_default_fee_token {
		let caller: AccountId = account("caller", 0, SEED);
		let currency_id = CurrencyId::Token(TokenSymbol::AUSD);
	}: _(RawOrigin::Signed(caller.clone()), Some(currency_id))
	verify {
		assert_eq!(TransactionPayment::default_fee_currency_id(&caller), Some(currency_id));
	}

	on_finalize {
	}: {
		TransactionPayment::on_finalize(System::block_number());
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
	fn test_set_default_fee_token() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_set_default_fee_token());
		});
	}

	#[test]
	fn test_on_finalize() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_on_finalize());
		});
	}
}
