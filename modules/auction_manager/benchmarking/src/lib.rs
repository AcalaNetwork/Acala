//! Benchmarks for the auction manager module.
// This is separated into its own crate due to cyclic dependency issues.

#![cfg_attr(not(feature = "std"), no_std)]

mod mock;

use sp_std::prelude::*;
use sp_std::vec;

use auction_manager::Module as AuctionManager;
use auction_manager::*;
use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use orml_traits::{DataFeeder, MultiCurrency};
use primitives::{AuctionId, Balance, CurrencyId, TokenSymbol};
use sp_runtime::{DispatchError, FixedPointNumber};
use support::{AuctionManager as AuctionManagerTrait, CDPTreasury, Price};

pub struct Module<T: Trait>(auction_manager::Module<T>);

pub trait Trait:
	auction_manager::Trait + orml_oracle::Trait<orml_oracle::Instance1> + prices::Trait + emergency_shutdown::Trait
{
}

const SEED: u32 = 0;

fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	d.saturating_mul(1_000_000_000_000_000_000)
}

fn feed_price<T: Trait>(currency_id: CurrencyId, price: Price) -> Result<(), &'static str> {
	let oracle_operators = orml_oracle::Module::<T, orml_oracle::Instance1>::members().0;
	for operator in oracle_operators {
		<T as prices::Trait>::Source::feed_value(operator.clone(), currency_id, price)?;
	}
	Ok(())
}

fn emergency_shutdown<T: Trait>() -> Result<(), DispatchError> {
	emergency_shutdown::Module::<T>::emergency_shutdown(RawOrigin::Root.into())
}

benchmarks! {
	_ { }

	// `cancel` a surplus auction, worst case:
	// auction have been already bid
	cancel_surplus_auction {
		let u in 0 .. 1000;

		let bidder: T::AccountId = account("bidder", u, SEED);
		let native_currency_id = <T as auction_manager::Trait>::GetNativeCurrencyId::get();

		// set balance
		<T as auction_manager::Trait>::Currency::deposit(native_currency_id, &bidder, dollar(10))?;

		// create surplus auction
		AuctionManager::<T>::new_surplus_auction(dollar(1))?;
		let auction_id: AuctionId = Default::default();

		// bid surplus auction
		let _ = AuctionManager::<T>::surplus_auction_bid_handler(1.into(), auction_id, (bidder, dollar(1)), None);

		// shutdown
		emergency_shutdown::<T>()?;
	}: cancel(RawOrigin::None, auction_id)

	// `cancel` a debit auction, worst case:
	// auction have been already bid
	cancel_debit_auction {
		let u in 0 .. 1000;

		let bidder: T::AccountId = account("bidder", u, SEED);
		let stable_currency_id = <T as auction_manager::Trait>::GetStableCurrencyId::get();

		// set balance
		<T as auction_manager::Trait>::Currency::deposit(stable_currency_id, &bidder, dollar(20))?;

		// create debit auction
		AuctionManager::<T>::new_debit_auction(dollar(1), dollar(10))?;
		let auction_id: AuctionId = Default::default();

		// bid debit auction
		let _ = AuctionManager::<T>::debit_auction_bid_handler(1.into(), auction_id, (bidder, dollar(20)), None);

		// shutdown
		emergency_shutdown::<T>()?;
	}: cancel(RawOrigin::None, auction_id)

	// `cancel` a collateral auction, worst case:
	// auction have been already bid
	cancel_collateral_auction {
		let u in 0 .. 1000;

		let bidder: T::AccountId = account("bidder", u, SEED);
		let funder: T::AccountId = account("funder", u, SEED);
		let stable_currency_id = <T as auction_manager::Trait>::GetStableCurrencyId::get();

		// set balance
		<T as auction_manager::Trait>::Currency::deposit(stable_currency_id, &bidder, dollar(80))?;
		<T as auction_manager::Trait>::Currency::deposit(CurrencyId::Token(TokenSymbol::DOT), &funder, dollar(1))?;
		<T as auction_manager::Trait>::CDPTreasury::deposit_collateral(&funder, CurrencyId::Token(TokenSymbol::DOT), dollar(1))?;

		// feed price
		feed_price::<T>(CurrencyId::Token(TokenSymbol::DOT), Price::saturating_from_integer(120))?;

		// create collateral auction
		AuctionManager::<T>::new_collateral_auction(&funder, CurrencyId::Token(TokenSymbol::DOT), dollar(1), dollar(100))?;
		let auction_id: AuctionId = Default::default();

		// bid collateral auction
		let _ = AuctionManager::<T>::collateral_auction_bid_handler(1.into(), auction_id, (bidder, dollar(80)), None);

		// shutdown
		emergency_shutdown::<T>()?;
	}: cancel(RawOrigin::None, auction_id)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Runtime};
	use frame_support::assert_ok;

	#[test]
	fn cancel_surplus_auction() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_cancel_surplus_auction::<Runtime>());
		});
	}

	#[test]
	fn cancel_debit_auction() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_cancel_debit_auction::<Runtime>());
		});
	}

	#[test]
	fn cancel_collateral_auction() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_cancel_collateral_auction::<Runtime>());
		});
	}
}
