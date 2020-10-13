use crate::{Balance, CdpTreasury, CollateralCurrencyIds, Currencies, CurrencyId, Runtime, DOLLARS};

use frame_system::RawOrigin;
use module_support::CDPTreasury;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use sp_std::prelude::*;

fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	DOLLARS.saturating_mul(d)
}

runtime_benchmarks! {
	{ Runtime, module_cdp_treasury }

	_ {}

	auction_surplus {
		CdpTreasury::on_system_surplus(dollar(100))?;
	}: _(RawOrigin::Root, dollar(100))

	auction_debit {
		CdpTreasury::on_system_debit(dollar(100))?;
	}: _(RawOrigin::Root,dollar(100), dollar(200))

	auction_collateral {
		let currency_id: CurrencyId = CollateralCurrencyIds::get()[0];
		Currencies::deposit(currency_id, &CdpTreasury::account_id(), dollar(10000))?;
	}: _(RawOrigin::Root, currency_id, dollar(1000), dollar(1000), true)

	set_collateral_auction_maximum_size {
		let currency_id: CurrencyId = CollateralCurrencyIds::get()[0];
	}: _(RawOrigin::Root,currency_id, 200)
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
