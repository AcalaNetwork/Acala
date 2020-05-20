use super::utils::set_balance;
use crate::{Auction, AuctionId, AuctionManager, CdpTreasury, CurrencyId, Runtime, DOLLARS};
use module_support::{AuctionManager as AuctionManagerTrait, CDPTreasury};

use sp_std::prelude::*;

use frame_benchmarking::account;
use frame_system::RawOrigin;

use orml_benchmarking::runtime_benchmarks;

const SEED: u32 = 0;
const MAX_USER_INDEX: u32 = 1000;
const MAX_DOLLARS: u32 = 100;

runtime_benchmarks! {
	{ Runtime, orml_auction }

	_ {
		let u in 1 .. MAX_USER_INDEX => ();
		let d in 1 .. MAX_DOLLARS => ();
	}

	// `bid` a collateral auction, best cases:
	// there's no bidder before and bid price doesn't exceed target amount
	bid_collateral_auction_as_first_bidder {
		let u in ...;
		let d in ...;

		let bidder = account("bidder", u, SEED);
		let funder = account("funder", u, SEED);
		let currency_id = CurrencyId::DOT;
		let collateral_amount = DOLLARS.saturating_mul(100);
		let target_amount = DOLLARS.saturating_mul(10000);
		let bid_price = DOLLARS.saturating_mul((5000 + d).into());
		let auction_id: AuctionId = 0;

		set_balance(currency_id, &funder, collateral_amount);
		set_balance(CurrencyId::AUSD, &bidder, bid_price);
		<CdpTreasury as CDPTreasury<_>>::transfer_collateral_from(currency_id, &funder, collateral_amount)?;
		AuctionManager::new_collateral_auction(&funder, currency_id, collateral_amount, target_amount);
	}: bid(RawOrigin::Signed(bidder), auction_id, bid_price)

	// `bid` a collateral auction, worst cases:
	// there's bidder before and bid price will exceed target amount
	bid_collateral_auction {
		let u in ...;
		let d in ...;

		let bidder = account("bidder", u, SEED);
		let previous_bidder = account("previous_bidder", u, SEED);
		let funder = account("funder", u, SEED);
		let currency_id = CurrencyId::DOT;
		let collateral_amount = DOLLARS.saturating_mul(100);
		let target_amount = DOLLARS.saturating_mul(10000);
		let previous_bid_price = DOLLARS.saturating_mul((5000 + d).into());
		let bid_price = DOLLARS.saturating_mul((10000 + d).into());
		let auction_id: AuctionId = 0;

		set_balance(currency_id, &funder, collateral_amount);
		set_balance(CurrencyId::AUSD, &bidder, bid_price);
		set_balance(CurrencyId::AUSD, &previous_bidder, previous_bid_price);
		<CdpTreasury as CDPTreasury<_>>::transfer_collateral_from(currency_id, &funder, collateral_amount)?;
		AuctionManager::new_collateral_auction(&funder, currency_id, collateral_amount, target_amount);
		Auction::bid(RawOrigin::Signed(previous_bidder).into(), auction_id, previous_bid_price)?;
	}: bid(RawOrigin::Signed(bidder), auction_id, bid_price)

	// `bid` a surplus auction, best cases:
	// there's no bidder before
	bid_surplus_auction_as_first_bidder {
		let u in ...;
		let d in ...;

		let bidder = account("bidder", u, SEED);

		let surplus_amount = DOLLARS.saturating_mul(100);
		let bid_price = DOLLARS.saturating_mul(d.into());
		let auction_id: AuctionId = 0;

		set_balance(CurrencyId::ACA, &bidder, bid_price);
		AuctionManager::new_surplus_auction(surplus_amount);
	}: bid(RawOrigin::Signed(bidder), auction_id, bid_price)

	// `bid` a surplus auction, worst cases:
	// there's bidder before
	bid_surplus_auction {
		let u in ...;
		let d in ...;

		let bidder = account("bidder", u, SEED);
		let previous_bidder = account("previous_bidder", u, SEED);
		let surplus_amount = DOLLARS.saturating_mul(100);
		let bid_price = DOLLARS.saturating_mul((d * 2).into());
		let previous_bid_price = DOLLARS.saturating_mul(d.into());
		let auction_id: AuctionId = 0;

		set_balance(CurrencyId::ACA, &bidder, bid_price);
		set_balance(CurrencyId::ACA, &previous_bidder, previous_bid_price);
		AuctionManager::new_surplus_auction(surplus_amount);
		Auction::bid(RawOrigin::Signed(previous_bidder).into(), auction_id, previous_bid_price)?;
	}: bid(RawOrigin::Signed(bidder), auction_id, bid_price)

	// `bid` a debit auction, best cases:
	// there's no bidder before and bid price happens to be debit amount
	bid_debit_auction_as_first_bidder {
		let u in ...;

		let bidder = account("bidder", u, SEED);

		let fix_debit_amount = DOLLARS.saturating_mul(100);
		let initial_amount = DOLLARS.saturating_mul(10);
		let auction_id: AuctionId = 0;

		set_balance(CurrencyId::AUSD, &bidder, fix_debit_amount);
		AuctionManager::new_debit_auction(initial_amount ,fix_debit_amount);
	}: bid(RawOrigin::Signed(bidder), auction_id, fix_debit_amount)

	// `bid` a debit auction, worst cases:
	// there's bidder before
	bid_debit_auction {
		let u in ...;

		let bidder = account("bidder", u, SEED);
		let previous_bidder = account("previous_bidder", u, SEED);
		let fix_debit_amount = DOLLARS.saturating_mul(100);
		let initial_amount = DOLLARS.saturating_mul(10);
		let previous_bid_price = fix_debit_amount;
		let bid_price = fix_debit_amount * 2;
		let auction_id: AuctionId = 0;

		set_balance(CurrencyId::AUSD, &bidder, bid_price);
		set_balance(CurrencyId::AUSD, &previous_bidder, previous_bid_price);
		AuctionManager::new_debit_auction(initial_amount ,fix_debit_amount);
		Auction::bid(RawOrigin::Signed(previous_bidder).into(), auction_id, previous_bid_price)?;
	}: bid(RawOrigin::Signed(bidder), auction_id, bid_price)
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
	fn bid_collateral_auction_as_first_bidder() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_bid_collateral_auction_as_first_bidder());
		});
	}

	#[test]
	fn bid_collateral_auction() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_bid_collateral_auction());
		});
	}

	#[test]
	fn bid_surplus_auction_as_first_bidder() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_bid_surplus_auction_as_first_bidder());
		});
	}

	#[test]
	fn bid_surplus_auction() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_bid_surplus_auction());
		});
	}

	#[test]
	fn bid_debit_auction_as_first_bidder() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_bid_debit_auction_as_first_bidder());
		});
	}

	#[test]
	fn bid_debit_auction() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_bid_debit_auction());
		});
	}
}
