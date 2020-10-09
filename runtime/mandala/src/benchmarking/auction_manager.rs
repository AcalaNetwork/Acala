use crate::{
	AcalaOracle, AccountId, AuctionId, AuctionManager, Balance, CdpTreasury, Currencies, CurrencyId, EmergencyShutdown,
	GetNativeCurrencyId, GetStableCurrencyId, Price, Runtime, TokenSymbol, DOLLARS,
};

use super::utils::set_balance;
use frame_benchmarking::account;
use frame_system::RawOrigin;
use module_support::AuctionManager as AuctionManagerTrait;
use module_support::CDPTreasury;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use sp_runtime::FixedPointNumber;
use sp_std::prelude::*;

const SEED: u32 = 0;

fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	DOLLARS.saturating_mul(d)
}

runtime_benchmarks! {
	{ Runtime, module_auction_manager }

	_ {}

	// `cancel` a surplus auction, worst case:
	// auction have been already bid
	cancel_surplus_auction {
		let bidder: AccountId = account("bidder", 0, SEED);
		let native_currency_id = GetNativeCurrencyId::get();

		// set balance
		set_balance(native_currency_id, &bidder, dollar(10));

		// create surplus auction
		<AuctionManager as AuctionManagerTrait<AccountId>>::new_surplus_auction(dollar(1))?;
		let auction_id: AuctionId = Default::default();

		// bid surplus auction
		let _ = AuctionManager::surplus_auction_bid_handler(1, auction_id, (bidder, dollar(1)), None);

		// shutdown
		EmergencyShutdown::emergency_shutdown(RawOrigin::Root.into())?;
	}: cancel(RawOrigin::None, auction_id)

	// `cancel` a debit auction, worst case:
	// auction have been already bid
	cancel_debit_auction {
		let bidder: AccountId = account("bidder", 0, SEED);
		let stable_currency_id = GetStableCurrencyId::get();

		// set balance
		set_balance(stable_currency_id, &bidder, dollar(10));

		// create debit auction
		<AuctionManager as AuctionManagerTrait<AccountId>>::new_debit_auction(dollar(1), dollar(10))?;
		let auction_id: AuctionId = Default::default();

		// bid debit auction
		let _ = AuctionManager::debit_auction_bid_handler(1, auction_id, (bidder, dollar(20)), None);

		// shutdown
		EmergencyShutdown::emergency_shutdown(RawOrigin::Root.into())?;
	}: cancel(RawOrigin::None, auction_id)

	// `cancel` a collateral auction, worst case:
	// auction have been already bid
	cancel_collateral_auction {
		let bidder: AccountId = account("bidder", 0, SEED);
		let funder: AccountId = account("funder", 0, SEED);
		let stable_currency_id = GetStableCurrencyId::get();

		// set balance
		Currencies::deposit(stable_currency_id, &bidder, dollar(80))?;
		Currencies::deposit(CurrencyId::Token(TokenSymbol::DOT), &funder, dollar(1))?;
		CdpTreasury::deposit_collateral(&funder, CurrencyId::Token(TokenSymbol::DOT), dollar(1))?;

		// feed price
		AcalaOracle::feed_values(RawOrigin::Root.into(), vec![(CurrencyId::Token(TokenSymbol::DOT), Price::saturating_from_integer(120))])?;

		// create collateral auction
		AuctionManager::new_collateral_auction(&funder, CurrencyId::Token(TokenSymbol::DOT), dollar(1), dollar(100))?;
		let auction_id: AuctionId = Default::default();

		// bid collateral auction
		let _ = AuctionManager::collateral_auction_bid_handler(1, auction_id, (bidder, dollar(80)), None);

		// shutdown
		EmergencyShutdown::emergency_shutdown(RawOrigin::Root.into())?;
	}: cancel(RawOrigin::None, auction_id)
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
	fn test_cancel_surplus_auction() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_cancel_surplus_auction());
		});
	}

	#[test]
	fn test_cancel_debit_auction() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_cancel_debit_auction());
		});
	}

	#[test]
	fn test_cancel_collateral_auction() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_cancel_collateral_auction());
		});
	}
}
