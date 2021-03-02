use crate::{dollar, CdpTreasury, Currencies, CurrencyId, Runtime, ACA, AUSD, DOT};

use frame_system::RawOrigin;
use module_support::CDPTreasury;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use sp_std::prelude::*;

runtime_benchmarks! {
	{ Runtime, module_cdp_treasury }

	_ {}

	auction_surplus {
		CdpTreasury::on_system_surplus(100 * dollar(AUSD))?;
	}: _(RawOrigin::Root, 100 * dollar(AUSD))

	auction_debit {
		CdpTreasury::on_system_debit(100 * dollar(AUSD))?;
	}: _(RawOrigin::Root, 100 * dollar(AUSD), 200 * dollar(ACA))

	auction_collateral {
		let currency_id: CurrencyId = DOT;
		Currencies::deposit(currency_id, &CdpTreasury::account_id(), 10_000 * dollar(currency_id))?;
	}: _(RawOrigin::Root, currency_id, 1_000 * dollar(currency_id), 1_000 * dollar(AUSD), true)

	set_collateral_auction_maximum_size {
		let currency_id: CurrencyId = DOT;
	}: _(RawOrigin::Root, currency_id, 200 * dollar(currency_id))
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
	fn test_auction_surplus() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_auction_surplus());
		});
	}

	#[test]
	fn test_auction_debit() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_auction_debit());
		});
	}

	#[test]
	fn test_auction_collateral() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_auction_collateral());
		});
	}

	#[test]
	fn test_set_collateral_auction_maximum_size() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_set_collateral_auction_maximum_size());
		});
	}
}
