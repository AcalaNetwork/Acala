//! # Auction Manager Module
//!
//! ## Overview
//!
//! Auction the assets of the system for maintain the normal operation of the business.
//! Auction types include:
//!   - `collateral auction`: sell collateral assets for getting stable coin to eliminate the system's bad debit by auction
//!   - `surplus auction`: sell excessive surplus for getting native coin to burn by auction
//!   - `debit auction`: inflation some native token to sell for getting stable coin to eliminate excessive bad debit by auction

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	debug, decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::Get,
	weights::{constants::WEIGHT_PER_MICROS, DispatchClass},
	IterableStorageMap,
};
use frame_system::{
	self as system, ensure_none,
	offchain::{SendTransactionTypes, SubmitTransaction},
};
use orml_traits::{Auction, AuctionHandler, Change, MultiCurrency, OnNewBidResult};
use primitives::{Balance, CurrencyId};
use sp_runtime::{
	traits::{BlakeTwo256, Hash, Saturating, Zero},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity, ValidTransaction,
	},
	DispatchResult, FixedPointNumber, RandomNumberGenerator, RuntimeDebug,
};
use sp_std::{
	cmp::{Eq, PartialEq},
	prelude::*,
};
use support::{AuctionManager, CDPTreasury, CDPTreasuryExtended, DEXManager, OnEmergencyShutdown, PriceProvider, Rate};
use utilities::{OffchainErr, OffchainLock};

mod mock;
mod tests;

const DB_PREFIX: &[u8] = b"acala/auction-manager-offchain-worker/";

/// Information of an collateral auction
#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct CollateralAuctionItem<AccountId, BlockNumber> {
	/// Refund recipient for may receive refund
	refund_recipient: AccountId,
	/// Collateral type for sale
	currency_id: CurrencyId,
	/// current collateral amount for sale
	#[codec(compact)]
	amount: Balance,
	/// Target sales amount want to get by this auction
	#[codec(compact)]
	target: Balance,
	/// Auction start time
	start_time: BlockNumber,
}

/// Information of an debit auction
#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct DebitAuctionItem<BlockNumber> {
	/// Current amount of native coin for sale
	#[codec(compact)]
	amount: Balance,
	/// Fix amount of debit value(stable coin) which want to get by this auction
	#[codec(compact)]
	fix: Balance,
	/// Auction start time
	start_time: BlockNumber,
}

/// Information of an surplus auction
#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct SurplusAuctionItem<BlockNumber> {
	/// Fixed amount of surplus(stable coin) for sale
	#[codec(compact)]
	amount: Balance,
	/// Auction start time
	start_time: BlockNumber,
}

type AuctionIdOf<T> =
	<<T as Trait>::Auction as Auction<<T as system::Trait>::AccountId, <T as system::Trait>::BlockNumber>>::AuctionId;

pub trait Trait: SendTransactionTypes<Call<Self>> + system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

	/// The minimum increment size of each bid compared to the previous one
	type MinimumIncrementSize: Get<Rate>;

	/// The extended time for the auction to end after each successful bid
	type AuctionTimeToClose: Get<Self::BlockNumber>;

	/// When the total duration of the auction exceeds this soft cap, push the auction to end more faster
	type AuctionDurationSoftCap: Get<Self::BlockNumber>;

	/// The stable currency id
	type GetStableCurrencyId: Get<CurrencyId>;

	/// The native currency id
	type GetNativeCurrencyId: Get<CurrencyId>;

	/// The decrement of amout in debit auction when restocking
	type GetAmountAdjustment: Get<Rate>;

	/// Currency to transfer assets
	type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

	/// Auction to manager the auction process
	type Auction: Auction<Self::AccountId, Self::BlockNumber, Balance = Balance>;

	/// CDP treasury to escrow assets related to auction
	type CDPTreasury: CDPTreasuryExtended<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;

	/// DEX to get exchange info
	type DEX: DEXManager<Self::AccountId, CurrencyId, Balance>;

	/// The price source of currencies
	type PriceSource: PriceProvider<CurrencyId>;

	/// A configuration for base priority of unsigned transactions.
	///
	/// This is exposed so that it can be tuned for particular runtime, when
	/// multiple modules send unsigned transactions.
	type UnsignedPriority: Get<TransactionPriority>;
}

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
		AuctionId = AuctionIdOf<T>,
		CurrencyId = CurrencyId,
		Balance = Balance,
	{
		/// Create a collateral auction (auction_id, collateral_type, collateral_amount, target_bid_price)
		NewCollateralAuction(AuctionId, CurrencyId, Balance, Balance),
		/// Create a debit auction (auction_id, initial_supply_amount, fix_payment_amount)
		NewDebitAuction(AuctionId, Balance, Balance),
		/// Create a surplus auction (auction_id, fix_surplus_amount)
		NewSurplusAuction(AuctionId, Balance),
		/// Cancel a specific active auction (auction_id)
		CancelAuction(AuctionId),
		/// Collateral auction dealed (auction_id, collateral_type, collateral_amount, winner, payment_amount).
		CollateralAuctionDealed(AuctionId, CurrencyId, Balance, AccountId, Balance),
		/// Surplus auction dealed (auction_id, surplus_amount, winner, payment_amount).
		SurplusAuctionDealed(AuctionId, Balance, AccountId, Balance),
		/// Debit auction dealed (auction_id, debit_currency_amount, winner, payment_amount).
		DebitAuctionDealed(AuctionId, Balance, AccountId, Balance),
		/// Dex take collateral auction (auction_id, collateral_type, collateral_amount, turnover, refund)
		DEXTakeCollateralAuction(AuctionId, CurrencyId, Balance, Balance),
	}
);

decl_error! {
	/// Error for auction manager module.
	pub enum Error for Module<T: Trait> {
		/// The auction dose not exist
		AuctionNotExsits,
		/// The collateral auction is in reserved stage now
		InReservedStage,
		/// Feed price is invalid
		InvalidFeedPrice,
		/// Must after system shutdown
		MustAfterShutdown,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as AuctionManager {
		/// Mapping from auction id to collateral auction info
		pub CollateralAuctions get(fn collateral_auctions): map hasher(twox_64_concat) AuctionIdOf<T> =>
			Option<CollateralAuctionItem<T::AccountId, T::BlockNumber>>;

		/// Mapping from auction id to debit auction info
		pub DebitAuctions get(fn debit_auctions): map hasher(twox_64_concat) AuctionIdOf<T> =>
			Option<DebitAuctionItem<T::BlockNumber>>;

		/// Mapping from auction id to surplus auction info
		pub SurplusAuctions get(fn surplus_auctions): map hasher(twox_64_concat) AuctionIdOf<T> =>
			Option<SurplusAuctionItem<T::BlockNumber>>;

		/// Record of the total collateral amount of all ative collateral auctions under specific collateral type
		/// CollateralType -> TotalAmount
		pub TotalCollateralInAuction get(fn total_collateral_in_auction): map hasher(twox_64_concat) CurrencyId => Balance;

		/// Record of total target sales of all ative collateral auctions
		pub TotalTargetInAuction get(fn total_target_in_auction): Balance;

		/// Record of total fix amount of all ative debit auctions
		pub TotalDebitInAuction get(fn total_debit_in_auction): Balance;

		/// Record of total surplus amount of all ative surplus auctions
		pub TotalSurplusInAuction get(fn total_surplus_in_auction): Balance;

		/// System shutdown flag
		pub IsShutdown get(fn is_shutdown): bool;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;
		fn deposit_event() = default;

		/// The minimum increment size of each bid compared to the previous one
		const MinimumIncrementSize: Rate = T::MinimumIncrementSize::get();

		/// The extended time for the auction to end after each successful bid
		const AuctionTimeToClose: T::BlockNumber = T::AuctionTimeToClose::get();

		/// When the total duration of the auction exceeds this soft cap,
		/// double the effect of `MinimumIncrementSize`, halve the effect of `AuctionTimeToClose`
		const AuctionDurationSoftCap: T::BlockNumber = T::AuctionDurationSoftCap::get();

		/// The stable currency id
		const GetStableCurrencyId: CurrencyId = T::GetStableCurrencyId::get();

		/// The native currency id
		const GetNativeCurrencyId: CurrencyId = T::GetNativeCurrencyId::get();

		/// The decrement of amout in debit auction when restocking
		const GetAmountAdjustment: Rate = T::GetAmountAdjustment::get();

		/// Cancel active auction after system shutdown
		///
		/// The dispatch origin of this call must be _None_.
		///
		/// - `auction_id`: auction id
		///
		/// # <weight>
		/// - Preconditions:
		/// 	- T::Currency is orml_currencies
		/// 	- T::CDPTreasury is module_cdp_treasury
		/// 	- T::Auction is orml_auction
		/// - Complexity: `O(1)`
		/// - Db reads:
		///		- surplus auction worst case: `SurplusAuctions`, `TotalSurplusInAuction`, 1 item in orml_auction, 1 item in orml_currencies
		///		- debit auction worst case: `DebitAuctions`, `TotalDebitInAuction`, 1 item in orml_auction, 1 item in orml_currencies, 1 item in cdp_treasury
		///		- collateral auction worst case: `CollateralAuctions`, `TotalCollateralInAuction`, `TotalTargetInAuction`, 1 item in orml_auction, 3 item in orml_currencies, 2 item in cdp_treasury
		/// - Db writes:
		///		- surplus auction worst case: `SurplusAuctions`, `TotalSurplusInAuction`, 1 item in orml_auction, 1 item in orml_currencies
		///		- debit auction worst case: `DebitAuctions`, `TotalDebitInAuction`, 1 item in orml_auction, 1 item in orml_currencies, 1 item in cdp_treasury
		///		- collateral auction worst case: `CollateralAuctions`, `TotalCollateralInAuction`, `TotalTargetInAuction`, 1 item in orml_auction, 3 item in orml_currencies, 2 item in cdp_treasury
		/// -------------------
		/// Base Weight:
		///		- surplus auction worst case: 33.72 µs
		///		- debit auction worst case: 27.63 µs
		///		- collateral auction worst case: 80.13 µs
		/// # </weight>
		#[weight = (80 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(9, 9), DispatchClass::Operational)]
		pub fn cancel(origin, id: AuctionIdOf<T>) {
			ensure_none(origin)?;
			ensure!(Self::is_shutdown(), Error::<T>::MustAfterShutdown);
			<Module<T> as AuctionManager<T::AccountId>>::cancel_auction(id)?;
		}

		/// Start offchain worker in order to submit unsigned tx to cancel active auction
		/// after system shutdown.
		fn offchain_worker(now: T::BlockNumber) {
			if Self::is_shutdown() && sp_io::offchain::is_validator() {
				if let Err(e) = Self::_offchain_worker(now) {
					debug::info!(
						target: "auction-manager offchain worker",
						"cannot run offchain worker at {:?}: {:?}",
						now,
						e,
					);
				}
			}
		}
	}
}

impl<T: Trait> Module<T> {
	fn submit_cancel_auction_tx(auction_id: AuctionIdOf<T>) {
		let call = Call::<T>::cancel(auction_id);
		if SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()).is_err() {
			debug::warn!(
				target: "auction-manager offchain worker",
				"submit unsigned auction cancel tx for \nAuctionId {:?} failed : {:?}",
				auction_id, OffchainErr::SubmitTransaction,
			);
		} else {
			debug::debug!(
				target: "auction-manager offchain worker",
				"successfully submit unsigned auction cancel tx for \nAuctionId {:?}",
				auction_id,
			);
		}
	}

	fn _offchain_worker(now: T::BlockNumber) -> Result<(), OffchainErr> {
		// Acquire offchain worker lock.
		// If succeeded, update the lock, otherwise return error
		let offchain_lock = OffchainLock::new(DB_PREFIX.to_vec());
		offchain_lock.acquire_offchain_lock(|_: Option<()>| ())?;

		let random_seed = sp_io::offchain::random_seed();
		let mut rng = RandomNumberGenerator::<BlakeTwo256>::new(BlakeTwo256::hash(&random_seed[..]));
		match rng.pick_u32(2) {
			0 => {
				for (auction_id, _) in <DebitAuctions<T>>::iter() {
					Self::submit_cancel_auction_tx(auction_id);
					offchain_lock.extend_offchain_lock_if_needed::<()>();
				}
			}
			1 => {
				for (auction_id, _) in <SurplusAuctions<T>>::iter() {
					Self::submit_cancel_auction_tx(auction_id);
					offchain_lock.extend_offchain_lock_if_needed::<()>();
				}
			}
			_ => {
				for (auction_id, _) in <CollateralAuctions<T>>::iter() {
					if !Self::collateral_auction_in_reverse_stage(auction_id) {
						Self::submit_cancel_auction_tx(auction_id);
					}
					offchain_lock.extend_offchain_lock_if_needed::<()>();
				}
			}
		}

		// finally, reset the expire timestamp to now in order to release lock in advance.
		offchain_lock.release_offchain_lock(|_: ()| true);
		debug::debug!(
			target: "auction-manager offchain worker",
			"offchain worker start at block: {:?} already done!",
			now,
		);

		Ok(())
	}

	pub fn emergency_shutdown() {
		<IsShutdown>::put(true);
	}

	pub fn cancel_surplus_auction(id: AuctionIdOf<T>) -> DispatchResult {
		let surplus_auction = <SurplusAuctions<T>>::take(id).ok_or(Error::<T>::AuctionNotExsits)?;
		if let Some(auction_info) = T::Auction::auction_info(id) {
			// if these's bid, refund native token to the bidder
			if let Some((bidder, bid_price)) = auction_info.bid {
				let native_currency_id = T::GetNativeCurrencyId::get();
				if T::Currency::free_balance(native_currency_id, &bidder)
					.checked_add(bid_price)
					.is_some()
				{
					// TODO: transfer from RESERVED TREASURY instead of mint
					T::Currency::deposit(native_currency_id, &bidder, bid_price)
						.expect("never failed after overflow check");
				}

				// decrease account ref of bidder
				system::Module::<T>::dec_ref(&bidder);
			}
		}

		// decrease total surplus in auction
		TotalSurplusInAuction::mutate(|balance| *balance = balance.saturating_sub(surplus_auction.amount));

		// remove the auction info in auction module
		T::Auction::remove_auction(id);

		<Module<T>>::deposit_event(RawEvent::CancelAuction(id));
		Ok(())
	}

	pub fn cancel_debit_auction(id: AuctionIdOf<T>) -> DispatchResult {
		let debit_auction = <DebitAuctions<T>>::take(id).ok_or(Error::<T>::AuctionNotExsits)?;
		if let Some(auction_info) = T::Auction::auction_info(id) {
			// if these's bid, refund stable token to the bidder
			if let Some((bidder, _)) = auction_info.bid {
				T::CDPTreasury::deposit_unbacked_debit_to(&bidder, debit_auction.fix)?;
				// decrease account ref of bidder
				system::Module::<T>::dec_ref(&bidder);
			}
		}

		// derease total debit in auction
		TotalDebitInAuction::mutate(|balance| *balance = balance.saturating_sub(debit_auction.fix));

		// remove the auction info in auction module
		T::Auction::remove_auction(id);

		<Module<T>>::deposit_event(RawEvent::CancelAuction(id));
		Ok(())
	}

	pub fn cancel_collateral_auction(id: AuctionIdOf<T>) -> DispatchResult {
		let collateral_auction = Self::collateral_auctions(id).ok_or(Error::<T>::AuctionNotExsits)?;
		// must not in reverse bid stage
		ensure!(
			!Self::collateral_auction_in_reverse_stage(id),
			Error::<T>::InReservedStage
		);

		// calculate which amount of collateral to offset target
		// in settle price
		let stable_currency_id = T::GetStableCurrencyId::get();
		let settle_price = T::PriceSource::get_relative_price(stable_currency_id, collateral_auction.currency_id)
			.ok_or(Error::<T>::InvalidFeedPrice)?;
		let confiscate_collateral_amount = sp_std::cmp::min(
			settle_price.saturating_mul_int(collateral_auction.target),
			collateral_auction.amount,
		);
		let refund_collateral_amount = collateral_auction.amount.saturating_sub(confiscate_collateral_amount);

		// refund remain collateral to refund recipient from cdp treasury
		if !refund_collateral_amount.is_zero() {
			T::CDPTreasury::transfer_collateral_to(
				collateral_auction.currency_id,
				&collateral_auction.refund_recipient,
				refund_collateral_amount,
			)?;
		}

		if let Some(auction_info) = T::Auction::auction_info(id) {
			// if these's bid, refund stable token to the bidder
			if let Some((bidder, bid_price)) = auction_info.bid {
				T::CDPTreasury::deposit_unbacked_debit_to(&bidder, bid_price)?;
				// decrease account ref of bidder
				system::Module::<T>::dec_ref(&bidder);
			}
		}

		// decrease account ref of refund recipient
		system::Module::<T>::dec_ref(&collateral_auction.refund_recipient);

		// decrease total collateral and target in auction
		TotalCollateralInAuction::mutate(collateral_auction.currency_id, |balance| {
			*balance = balance.saturating_sub(collateral_auction.amount)
		});
		TotalTargetInAuction::mutate(|balance| *balance = balance.saturating_sub(collateral_auction.target));

		// remove collateral auction
		<CollateralAuctions<T>>::remove(id);
		T::Auction::remove_auction(id);

		<Module<T>>::deposit_event(RawEvent::CancelAuction(id));
		Ok(())
	}

	pub fn collateral_auction_in_reverse_stage(id: AuctionIdOf<T>) -> bool {
		if let Some(collateral_auction) = <CollateralAuctions<T>>::get(id) {
			if let Some(auction_info) = T::Auction::auction_info(id) {
				if let Some((_, bid_price)) = auction_info.bid {
					return bid_price >= collateral_auction.target;
				}
			}
		}
		false
	}

	/// Check `new_price` is larger than minimum increment
	/// Formula: new_price - last_price >= max(last_price, target_price) * minimum_increment
	pub fn check_minimum_increment(
		new_price: Balance,
		last_price: Balance,
		target_price: Balance,
		minimum_increment: Rate,
	) -> bool {
		if let (Some(target), Some(result)) = (
			minimum_increment.checked_mul_int(sp_std::cmp::max(target_price, last_price)),
			new_price.checked_sub(last_price),
		) {
			return result >= target;
		}
		false
	}

	pub fn get_minimum_increment_size(now: T::BlockNumber, start_block: T::BlockNumber) -> Rate {
		// reach soft cap
		if now >= start_block + T::AuctionDurationSoftCap::get() {
			// double the minimum increment size
			T::MinimumIncrementSize::get().saturating_mul(Rate::saturating_from_integer(2))
		} else {
			T::MinimumIncrementSize::get()
		}
	}

	pub fn get_auction_time_to_close(now: T::BlockNumber, start_block: T::BlockNumber) -> T::BlockNumber {
		// reach soft cap
		if now >= start_block + T::AuctionDurationSoftCap::get() {
			// halve the extended time of bid
			T::AuctionTimeToClose::get() / 2.into()
		} else {
			T::AuctionTimeToClose::get()
		}
	}

	pub fn collateral_auction_bid_handler(
		now: T::BlockNumber,
		id: AuctionIdOf<T>,
		new_bid: (T::AccountId, Balance),
		last_bid: Option<(T::AccountId, Balance)>,
	) -> OnNewBidResult<T::BlockNumber> {
		if let Some(mut collateral_auction) = Self::collateral_auctions(id) {
			// get last price, if these's no bid set 0
			let last_price: Balance = match last_bid {
				None => Zero::zero(),
				Some((_, price)) => price,
			};
			let stable_currency_id = T::GetStableCurrencyId::get();
			let mut payment = sp_std::cmp::min(collateral_auction.target, new_bid.1);

			// check new price is larger than minimum increment and new bidder has enough stable coin
			if Self::check_minimum_increment(
				new_bid.1,
				last_price,
				collateral_auction.target,
				Self::get_minimum_increment_size(now, collateral_auction.start_time),
			) && T::Currency::ensure_can_withdraw(stable_currency_id, &new_bid.0, payment).is_ok()
			{
				// increase account ref of new bidder
				system::Module::<T>::inc_ref(&new_bid.0);

				// if these's bid before, return stablecoin from new bidder to last bidder
				if let Some((last_bidder, last_price)) = last_bid {
					let refund = sp_std::cmp::min(last_price, collateral_auction.target);
					T::Currency::transfer(stable_currency_id, &new_bid.0, &last_bidder, refund)
						.expect("never failed after balance check");

					// decrease account ref of last bidder
					system::Module::<T>::dec_ref(&last_bidder);

					payment -= refund;
				}

				if !payment.is_zero() {
					// transfer stablecoin from new bidder to cdp treasury
					T::CDPTreasury::transfer_surplus_from(&new_bid.0, payment)
						.expect("never failed after balance check");
				}

				// if bid_price > target, the auction is in reverse, refund collateral to it's origin from auction cdp treasury
				if new_bid.1 > collateral_auction.target {
					let new_collateral_amount =
						Rate::checked_from_rational(sp_std::cmp::max(last_price, collateral_auction.target), new_bid.1)
							.and_then(|n| n.checked_mul_int(collateral_auction.amount))
							.unwrap_or(collateral_auction.amount);
					let deduct_collateral_amount = collateral_auction.amount.saturating_sub(new_collateral_amount);

					if T::CDPTreasury::transfer_collateral_to(
						collateral_auction.currency_id,
						&(collateral_auction.refund_recipient),
						deduct_collateral_amount,
					)
					.is_ok()
					{
						// update collateral auction when refund collateral to refund recipient success
						TotalCollateralInAuction::mutate(collateral_auction.currency_id, |balance| {
							*balance = balance.saturating_sub(deduct_collateral_amount)
						});
						collateral_auction.amount = new_collateral_amount;
						<CollateralAuctions<T>>::insert(id, collateral_auction.clone());
					}
				}

				return OnNewBidResult {
					accept_bid: true,
					auction_end_change: Change::NewValue(Some(
						now + Self::get_auction_time_to_close(now, collateral_auction.start_time),
					)),
				};
			}
		}

		OnNewBidResult {
			accept_bid: false,
			auction_end_change: Change::NoChange,
		}
	}

	pub fn debit_auction_bid_handler(
		now: T::BlockNumber,
		id: AuctionIdOf<T>,
		new_bid: (T::AccountId, Balance),
		last_bid: Option<(T::AccountId, Balance)>,
	) -> OnNewBidResult<T::BlockNumber> {
		if let Some(mut debit_auction) = Self::debit_auctions(id) {
			let last_price: Balance = match last_bid {
				None => Zero::zero(),
				Some((_, price)) => price,
			};
			let stable_currency_id = T::GetStableCurrencyId::get();

			if Self::check_minimum_increment(
				new_bid.1,
				last_price,
				debit_auction.fix,
				Self::get_minimum_increment_size(now, debit_auction.start_time),
			) && new_bid.1 >= debit_auction.fix
				&& T::Currency::ensure_can_withdraw(stable_currency_id, &new_bid.0, debit_auction.fix).is_ok()
			{
				if let Some((last_bidder, _)) = last_bid {
					// these's bid before, transfer the stablecoin from new bidder to last bidder
					T::Currency::transfer(stable_currency_id, &new_bid.0, &last_bidder, debit_auction.fix)
						.expect("never failed after balance check");

					// decrease account ref of last bidder
					system::Module::<T>::dec_ref(&last_bidder);
				} else {
					// these's no bid before, transfer stablecoin to cdp treasury
					T::CDPTreasury::transfer_surplus_from(&new_bid.0, debit_auction.fix)
						.expect("never failed after balance check");
				}

				// increase account ref of bidder
				system::Module::<T>::inc_ref(&new_bid.0);

				// if bid price is more than fix
				// calculate new amount of issue native token
				if new_bid.1 > debit_auction.fix {
					debit_auction.amount =
						Rate::checked_from_rational(sp_std::cmp::max(last_price, debit_auction.fix), new_bid.1)
							.and_then(|n| n.checked_mul_int(debit_auction.amount))
							.unwrap_or(debit_auction.amount);
					<DebitAuctions<T>>::insert(id, debit_auction.clone());
				}

				return OnNewBidResult {
					accept_bid: true,
					auction_end_change: Change::NewValue(Some(
						now + Self::get_auction_time_to_close(now, debit_auction.start_time),
					)),
				};
			}
		}

		OnNewBidResult {
			accept_bid: false,
			auction_end_change: Change::NoChange,
		}
	}

	pub fn surplus_auction_bid_handler(
		now: T::BlockNumber,
		id: AuctionIdOf<T>,
		new_bid: (T::AccountId, Balance),
		last_bid: Option<(T::AccountId, Balance)>,
	) -> OnNewBidResult<T::BlockNumber> {
		if let Some(surplus_auction) = Self::surplus_auctions(id) {
			let last_price: Balance = match last_bid {
				None => Zero::zero(),
				Some((_, price)) => price,
			};
			let native_currency_id = T::GetNativeCurrencyId::get();

			// check new price is larger than minimum increment and new bidder has enough native token
			if Self::check_minimum_increment(
				new_bid.1,
				last_price,
				Zero::zero(),
				Self::get_minimum_increment_size(now, surplus_auction.start_time),
			) && T::Currency::ensure_can_withdraw(native_currency_id, &new_bid.0, new_bid.1).is_ok()
				&& !new_bid.1.is_zero()
			{
				let mut burn_native_currency_amount = new_bid.1;

				// if these's bid before, transfer the stablecoin from auction manager module to last bidder
				if let Some((last_bidder, last_price)) = last_bid {
					burn_native_currency_amount = burn_native_currency_amount.saturating_sub(last_price);
					T::Currency::transfer(native_currency_id, &new_bid.0, &last_bidder, last_price)
						.expect("never failed after balance check");

					// decrease account ref of last bidder
					system::Module::<T>::dec_ref(&last_bidder);
				}

				// burn remain native token from new bidder
				T::Currency::withdraw(native_currency_id, &new_bid.0, burn_native_currency_amount)
					.expect("never failed after balance check");

				// increase account ref of bidder
				system::Module::<T>::inc_ref(&new_bid.0);

				return OnNewBidResult {
					accept_bid: true,
					auction_end_change: Change::NewValue(Some(
						now + Self::get_auction_time_to_close(now, surplus_auction.start_time),
					)),
				};
			}
		}

		OnNewBidResult {
			accept_bid: false,
			auction_end_change: Change::NoChange,
		}
	}

	pub fn collateral_auction_end_handler(auction_id: AuctionIdOf<T>, winner: Option<(T::AccountId, Balance)>) {
		if let (Some(collateral_auction), Some((bidder, bid_price))) = (Self::collateral_auctions(auction_id), winner) {
			let stable_currency_id = T::GetStableCurrencyId::get();
			let mut should_deal = true;

			// if bid_price doesn't reach target and trading with DEX will get better result
			if bid_price < collateral_auction.target
				&& bid_price
					< T::DEX::get_target_amount(
						collateral_auction.currency_id,
						stable_currency_id,
						collateral_auction.amount,
					) {
				// try trade with DEX
				if let Ok(amount) = T::CDPTreasury::swap_collateral_to_stable(
					collateral_auction.currency_id,
					collateral_auction.amount,
					Zero::zero(),
				) {
					// swap successfully, will not deal
					should_deal = false;

					// refund stable coin to the last bidder, ignore result to continue
					let _ = T::CDPTreasury::deposit_unbacked_debit_to(&bidder, bid_price);

					// extra stable coin will refund to refund recipient, ignore result to continue
					if amount > collateral_auction.target {
						let _ = T::CDPTreasury::deposit_unbacked_debit_to(
							&collateral_auction.refund_recipient,
							amount - collateral_auction.target,
						);
					}

					<Module<T>>::deposit_event(RawEvent::DEXTakeCollateralAuction(
						auction_id,
						collateral_auction.currency_id,
						collateral_auction.amount,
						amount,
					));
				}
			}

			if should_deal {
				// transfer collateral to winner from cdp treasury, ignore result to continue
				let _ = T::CDPTreasury::transfer_collateral_to(
					collateral_auction.currency_id,
					&bidder,
					collateral_auction.amount,
				);

				<Module<T>>::deposit_event(RawEvent::CollateralAuctionDealed(
					auction_id,
					collateral_auction.currency_id,
					collateral_auction.amount,
					bidder.clone(),
					sp_std::cmp::min(collateral_auction.target, bid_price),
				));
			}

			// decrease account ref of bidder and refund recipient
			system::Module::<T>::dec_ref(&bidder);
			system::Module::<T>::dec_ref(&collateral_auction.refund_recipient);

			// update auction records
			TotalCollateralInAuction::mutate(collateral_auction.currency_id, |balance| {
				*balance = balance.saturating_sub(collateral_auction.amount)
			});
			TotalTargetInAuction::mutate(|balance| *balance = balance.saturating_sub(collateral_auction.target));
			<CollateralAuctions<T>>::remove(auction_id);
		}
	}

	pub fn debit_auction_end_handler(auction_id: AuctionIdOf<T>, winner: Option<(T::AccountId, Balance)>) {
		if let Some(debit_auction) = Self::debit_auctions(auction_id) {
			if let Some((bidder, _)) = winner {
				// issue the amount of native token to winner
				if T::Currency::free_balance(T::GetNativeCurrencyId::get(), &bidder)
					.checked_add(debit_auction.amount)
					.is_some()
				{
					// TODO: transfer from RESERVED TREASURY instead of mint
					T::Currency::deposit(T::GetNativeCurrencyId::get(), &bidder, debit_auction.amount)
						.expect("never failed after overflow check");
				}

				// decrease account ref of winner
				system::Module::<T>::dec_ref(&bidder);

				// decrease debit in auction and delete auction
				TotalDebitInAuction::mutate(|balance| *balance = balance.saturating_sub(debit_auction.fix));
				<DebitAuctions<T>>::remove(auction_id);

				<Module<T>>::deposit_event(RawEvent::DebitAuctionDealed(
					auction_id,
					debit_auction.amount,
					bidder,
					debit_auction.fix,
				));
			} else {
				// there's no bidder until auction closed, adjust the native token amount
				let start_block = <system::Module<T>>::block_number();
				let end_block = start_block + T::AuctionTimeToClose::get();
				let new_debit_auction_id: AuctionIdOf<T> = T::Auction::new_auction(start_block, Some(end_block));
				let new_amount = T::GetAmountAdjustment::get().saturating_mul_acc_int(debit_auction.amount);
				let new_debit_auction = DebitAuctionItem {
					amount: new_amount,
					fix: debit_auction.fix,
					start_time: start_block,
				};
				<DebitAuctions<T>>::insert(new_debit_auction_id, new_debit_auction.clone());
				<DebitAuctions<T>>::remove(auction_id);

				<Module<T>>::deposit_event(RawEvent::CancelAuction(auction_id));
				<Module<T>>::deposit_event(RawEvent::NewDebitAuction(
					new_debit_auction_id,
					new_debit_auction.amount,
					new_debit_auction.fix,
				));
			}
		}
	}

	pub fn surplus_auction_end_handler(auction_id: AuctionIdOf<T>, winner: Option<(T::AccountId, Balance)>) {
		if let (Some(surplus_auction), Some((bidder, bidder_price))) = (Self::surplus_auctions(auction_id), winner) {
			// deposit unbacked stable token to winner by cdp treasury, ignore Err
			let _ = T::CDPTreasury::deposit_unbacked_debit_to(&bidder, surplus_auction.amount);

			// decrease account ref of winner
			system::Module::<T>::dec_ref(&bidder);

			// decrease surplus in auction
			TotalSurplusInAuction::mutate(|balance| *balance = balance.saturating_sub(surplus_auction.amount));
			<SurplusAuctions<T>>::remove(auction_id);

			<Module<T>>::deposit_event(RawEvent::SurplusAuctionDealed(
				auction_id,
				surplus_auction.amount,
				bidder,
				bidder_price,
			));
		}
	}
}

impl<T: Trait> AuctionHandler<T::AccountId, Balance, T::BlockNumber, AuctionIdOf<T>> for Module<T> {
	fn on_new_bid(
		now: T::BlockNumber,
		id: AuctionIdOf<T>,
		new_bid: (T::AccountId, Balance),
		last_bid: Option<(T::AccountId, Balance)>,
	) -> OnNewBidResult<T::BlockNumber> {
		if <CollateralAuctions<T>>::contains_key(id) {
			Self::collateral_auction_bid_handler(now, id, new_bid, last_bid)
		} else if <DebitAuctions<T>>::contains_key(id) {
			Self::debit_auction_bid_handler(now, id, new_bid, last_bid)
		} else if <SurplusAuctions<T>>::contains_key(id) {
			Self::surplus_auction_bid_handler(now, id, new_bid, last_bid)
		} else {
			OnNewBidResult {
				accept_bid: false,
				auction_end_change: Change::NoChange,
			}
		}
	}

	fn on_auction_ended(id: AuctionIdOf<T>, winner: Option<(T::AccountId, Balance)>) {
		if <CollateralAuctions<T>>::contains_key(id) {
			Self::collateral_auction_end_handler(id, winner)
		} else if <DebitAuctions<T>>::contains_key(id) {
			Self::debit_auction_end_handler(id, winner)
		} else if <SurplusAuctions<T>>::contains_key(id) {
			Self::surplus_auction_end_handler(id, winner)
		}
	}
}

impl<T: Trait> AuctionManager<T::AccountId> for Module<T> {
	type CurrencyId = CurrencyId;
	type Balance = Balance;
	type AuctionId = AuctionIdOf<T>;

	fn new_collateral_auction(
		who: &T::AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
		target: Self::Balance,
	) {
		if Self::total_collateral_in_auction(currency_id)
			.checked_add(amount)
			.is_some() && Self::total_target_in_auction().checked_add(target).is_some()
		{
			TotalCollateralInAuction::mutate(currency_id, |balance| *balance += amount);
			TotalTargetInAuction::mutate(|balance| *balance += target);

			let block_number = <system::Module<T>>::block_number();
			let auction_id: AuctionIdOf<T> = T::Auction::new_auction(block_number, None); // do not set endtime for collateral auction
			let collateral_aution = CollateralAuctionItem {
				refund_recipient: who.clone(),
				currency_id,
				amount,
				target,
				start_time: block_number,
			};

			// decrease account ref of refund recipient
			system::Module::<T>::inc_ref(&who);

			<CollateralAuctions<T>>::insert(auction_id, collateral_aution);
			<Module<T>>::deposit_event(RawEvent::NewCollateralAuction(auction_id, currency_id, amount, target));
		}
	}

	fn new_debit_auction(initial_amount: Self::Balance, fix_debit: Self::Balance) {
		if Self::total_debit_in_auction().checked_add(fix_debit).is_some() {
			TotalDebitInAuction::mutate(|balance| *balance += fix_debit);
			let start_block = <system::Module<T>>::block_number();
			let end_block = start_block + T::AuctionTimeToClose::get();
			// set close time for initial debit auction
			let auction_id: AuctionIdOf<T> = T::Auction::new_auction(start_block, Some(end_block)); // set endtime for debit auction!
			let debit_auction = DebitAuctionItem {
				amount: initial_amount,
				fix: fix_debit,
				start_time: start_block,
			};
			<DebitAuctions<T>>::insert(auction_id, debit_auction);
			<Module<T>>::deposit_event(RawEvent::NewDebitAuction(auction_id, initial_amount, fix_debit));
		}
	}

	fn new_surplus_auction(amount: Self::Balance) {
		if Self::total_surplus_in_auction().checked_add(amount).is_some() {
			TotalSurplusInAuction::mutate(|balance| *balance += amount);
			let auction_id: AuctionIdOf<T> = T::Auction::new_auction(<system::Module<T>>::block_number(), None); // do not set endtime for surplus auction
			let surplus_auction = SurplusAuctionItem {
				amount,
				start_time: <system::Module<T>>::block_number(),
			};
			<SurplusAuctions<T>>::insert(auction_id, surplus_auction);
			<Module<T>>::deposit_event(RawEvent::NewSurplusAuction(auction_id, amount));
		}
	}

	fn get_total_debit_in_auction() -> Self::Balance {
		Self::total_debit_in_auction()
	}

	fn get_total_target_in_auction() -> Self::Balance {
		Self::total_target_in_auction()
	}

	fn get_total_collateral_in_auction(id: Self::CurrencyId) -> Self::Balance {
		Self::total_collateral_in_auction(id)
	}

	fn get_total_surplus_in_auction() -> Self::Balance {
		Self::total_surplus_in_auction()
	}

	fn cancel_auction(id: Self::AuctionId) -> DispatchResult {
		if <CollateralAuctions<T>>::contains_key(id) {
			Self::cancel_collateral_auction(id)
		} else if <DebitAuctions<T>>::contains_key(id) {
			Self::cancel_debit_auction(id)
		} else if <SurplusAuctions<T>>::contains_key(id) {
			Self::cancel_surplus_auction(id)
		} else {
			Err(Error::<T>::AuctionNotExsits.into())
		}
	}
}

impl<T: Trait> OnEmergencyShutdown for Module<T> {
	fn on_emergency_shutdown() {
		Self::emergency_shutdown();
	}
}

#[allow(deprecated)]
impl<T: Trait> frame_support::unsigned::ValidateUnsigned for Module<T> {
	type Call = Call<T>;

	fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
		if let Call::cancel(auction_id) = call {
			if !Self::is_shutdown() {
				return InvalidTransaction::Stale.into();
			} else if <CollateralAuctions<T>>::contains_key(auction_id) {
				if !Self::collateral_auction_in_reverse_stage(*auction_id) {
					return InvalidTransaction::Stale.into();
				}
			} else if !<SurplusAuctions<T>>::contains_key(auction_id) && !<DebitAuctions<T>>::contains_key(auction_id) {
				return InvalidTransaction::Stale.into();
			}

			ValidTransaction::with_tag_prefix("AuctionManagerOffchainWorker")
				.priority(T::UnsignedPriority::get())
				.and_provides(auction_id)
				.longevity(64_u64)
				.propagate(true)
				.build()
		} else {
			InvalidTransaction::Call.into()
		}
	}
}
