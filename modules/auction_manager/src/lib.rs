//! # Auction Manager Module
//!
//! ## Overview
//!
//! Auction the assets of the system for maintain the normal operation of the
//! business. Auction types include:
//!   - `collateral auction`: sell collateral assets for getting stable currency
//!     to eliminate the system's bad debit by auction
//!   - `surplus auction`: sell excessive surplus for getting native coin to
//!     burn by auction
//!   - `debit auction`: inflation some native token to sell for getting stable
//!     coin to eliminate excessive bad debit by auction

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
use orml_utilities::with_transaction_result;
use primitives::{AuctionId, Balance, CurrencyId};
use sp_runtime::{
	offchain::{
		storage_lock::{StorageLock, Time},
		Duration,
	},
	traits::{BlakeTwo256, CheckedDiv, Hash, Saturating, Zero},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity, ValidTransaction,
	},
	DispatchError, DispatchResult, FixedPointNumber, RandomNumberGenerator, RuntimeDebug,
};
use sp_std::{
	cmp::{Eq, PartialEq},
	prelude::*,
};
use support::{AuctionManager, CDPTreasury, CDPTreasuryExtended, DEXManager, EmergencyShutdown, PriceProvider, Rate};
use utilities::OffchainErr;

mod mock;
mod tests;

const OFFCHAIN_WORKER_LOCK: &[u8] = b"acala/auction-manager/lock/";
const LOCK_DURATION: u64 = 100;

/// Information of an collateral auction
#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct CollateralAuctionItem<AccountId, BlockNumber> {
	/// Refund recipient for may receive refund
	refund_recipient: AccountId,
	/// Collateral type for sale
	currency_id: CurrencyId,
	/// Current collateral amount for sale
	#[codec(compact)]
	amount: Balance,
	/// Target sales amount of this auction
	/// if zero, collateral auction will never be reverse stage,
	/// otherwise, target amount is the actual payment amount of active bidder
	#[codec(compact)]
	target: Balance,
	/// Auction start time
	start_time: BlockNumber,
}

impl<AccountId, BlockNumber> CollateralAuctionItem<AccountId, BlockNumber> {
	/// Return the collateral auction will never be reverse stage
	fn always_forward(&self) -> bool {
		self.target.is_zero()
	}

	/// Return whether the collateral auction is in reverse stage at specific
	/// bid price
	fn in_reverse_stage(&self, bid_price: Balance) -> bool {
		!self.always_forward() && bid_price >= self.target
	}

	/// Return the actual number of stablecoins to be paid
	fn payment_amount(&self, bid_price: Balance) -> Balance {
		if self.always_forward() {
			bid_price
		} else {
			sp_std::cmp::min(self.target, bid_price)
		}
	}

	/// Return new collateral amount at specific last bid price and new bid
	/// price
	fn collateral_amount(&self, last_bid_price: Balance, new_bid_price: Balance) -> Balance {
		if self.in_reverse_stage(new_bid_price) && new_bid_price > last_bid_price {
			Rate::checked_from_rational(sp_std::cmp::max(last_bid_price, self.target), new_bid_price)
				.and_then(|n| n.checked_mul_int(self.amount))
				.unwrap_or(self.amount)
		} else {
			self.amount
		}
	}
}

/// Information of an debit auction
#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct DebitAuctionItem<BlockNumber> {
	/// Current amount of native currency for sale
	#[codec(compact)]
	amount: Balance,
	/// Fix amount of debit value(stable currency) which want to get by this
	/// auction
	#[codec(compact)]
	fix: Balance,
	/// Auction start time
	start_time: BlockNumber,
}

impl<BlockNumber> DebitAuctionItem<BlockNumber> {
	/// Return amount for sale at specific last bid price and new bid price
	fn amount_for_sale(&self, last_bid_price: Balance, new_bid_price: Balance) -> Balance {
		if new_bid_price > last_bid_price && new_bid_price > self.fix {
			Rate::checked_from_rational(sp_std::cmp::max(last_bid_price, self.fix), new_bid_price)
				.and_then(|n| n.checked_mul_int(self.amount))
				.unwrap_or(self.amount)
		} else {
			self.amount
		}
	}
}

/// Information of an surplus auction
#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct SurplusAuctionItem<BlockNumber> {
	/// Fixed amount of surplus(stable currency) for sale
	#[codec(compact)]
	amount: Balance,
	/// Auction start time
	start_time: BlockNumber,
}

pub trait Trait: SendTransactionTypes<Call<Self>> + system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

	/// The minimum increment size of each bid compared to the previous one
	type MinimumIncrementSize: Get<Rate>;

	/// The extended time for the auction to end after each successful bid
	type AuctionTimeToClose: Get<Self::BlockNumber>;

	/// When the total duration of the auction exceeds this soft cap, push the
	/// auction to end more faster
	type AuctionDurationSoftCap: Get<Self::BlockNumber>;

	/// The stable currency id
	type GetStableCurrencyId: Get<CurrencyId>;

	/// The native currency id
	type GetNativeCurrencyId: Get<CurrencyId>;

	/// The decrement of amount in debit auction when restocking
	type GetAmountAdjustment: Get<Rate>;

	/// Currency to transfer assets
	type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

	/// Auction to manager the auction process
	type Auction: Auction<Self::AccountId, Self::BlockNumber, AuctionId = AuctionId, Balance = Balance>;

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

	/// Emergency shutdown.
	type EmergencyShutdown: EmergencyShutdown;
}

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
		AuctionId = AuctionId,
		CurrencyId = CurrencyId,
		Balance = Balance,
	{
		/// Collateral auction created. [auction_id, collateral_type, collateral_amount, target_bid_price]
		NewCollateralAuction(AuctionId, CurrencyId, Balance, Balance),
		/// Debit auction created. [auction_id, initial_supply_amount, fix_payment_amount]
		NewDebitAuction(AuctionId, Balance, Balance),
		/// Surplus auction created. [auction_id, fix_surplus_amount]
		NewSurplusAuction(AuctionId, Balance),
		/// Active auction cancelled. [auction_id]
		CancelAuction(AuctionId),
		/// Collateral auction dealt. [auction_id, collateral_type, collateral_amount, winner, payment_amount]
		CollateralAuctionDealt(AuctionId, CurrencyId, Balance, AccountId, Balance),
		/// Surplus auction dealt. [auction_id, surplus_amount, winner, payment_amount]
		SurplusAuctionDealt(AuctionId, Balance, AccountId, Balance),
		/// Debit auction dealt. [auction_id, debit_currency_amount, winner, payment_amount]
		DebitAuctionDealt(AuctionId, Balance, AccountId, Balance),
		/// Dex take collateral auction. [auction_id, collateral_type, collateral_amount, turnover]
		DEXTakeCollateralAuction(AuctionId, CurrencyId, Balance, Balance),
	}
);

decl_error! {
	/// Error for auction manager module.
	pub enum Error for Module<T: Trait> {
		/// The auction dose not exist
		AuctionNotExists,
		/// The collateral auction is in reverse stage now
		InReverseStage,
		/// Feed price is invalid
		InvalidFeedPrice,
		/// Must after system shutdown
		MustAfterShutdown,
		/// Bid price is invalid
		InvalidBidPrice,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as AuctionManager {
		/// Mapping from auction id to collateral auction info
		pub CollateralAuctions get(fn collateral_auctions): map hasher(twox_64_concat) AuctionId =>
			Option<CollateralAuctionItem<T::AccountId, T::BlockNumber>>;

		/// Mapping from auction id to debit auction info
		pub DebitAuctions get(fn debit_auctions): map hasher(twox_64_concat) AuctionId =>
			Option<DebitAuctionItem<T::BlockNumber>>;

		/// Mapping from auction id to surplus auction info
		pub SurplusAuctions get(fn surplus_auctions): map hasher(twox_64_concat) AuctionId =>
			Option<SurplusAuctionItem<T::BlockNumber>>;

		/// Record of the total collateral amount of all active collateral auctions under specific collateral type
		/// CollateralType -> TotalAmount
		pub TotalCollateralInAuction get(fn total_collateral_in_auction): map hasher(twox_64_concat) CurrencyId => Balance;

		/// Record of total target sales of all active collateral auctions
		pub TotalTargetInAuction get(fn total_target_in_auction): Balance;

		/// Record of total fix amount of all active debit auctions
		pub TotalDebitInAuction get(fn total_debit_in_auction): Balance;

		/// Record of total surplus amount of all active surplus auctions
		pub TotalSurplusInAuction get(fn total_surplus_in_auction): Balance;
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

		/// The decrement of amount in debit auction when restocking
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
		///		- surplus auction worst case: 6
		///		- debit auction worst case: 5
		///		- collateral auction worst case: 15
		/// - Db writes:
		///		- surplus auction worst case: 3
		///		- debit auction worst case: 4
		///		- collateral auction worst case: 10
		/// -------------------
		/// Base Weight:
		///		- surplus auction worst case: 63.96 µs
		///		- debit auction worst case: 66.04 µs
		///		- collateral auction worst case: 197.5 µs
		/// # </weight>
		#[weight = (198 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(15, 10), DispatchClass::Operational)]
		pub fn cancel(origin, id: AuctionId) {
			with_transaction_result(|| {
				ensure_none(origin)?;
				ensure!(T::EmergencyShutdown::is_shutdown(), Error::<T>::MustAfterShutdown);
				<Module<T> as AuctionManager<T::AccountId>>::cancel_auction(id)?;
				<Module<T>>::deposit_event(RawEvent::CancelAuction(id));
				Ok(())
			})?;
		}

		/// Start offchain worker in order to submit unsigned tx to cancel active auction after system shutdown.
		fn offchain_worker(now: T::BlockNumber) {
			if T::EmergencyShutdown::is_shutdown() && sp_io::offchain::is_validator() {
				if let Err(e) = Self::_offchain_worker() {
					debug::info!(
						target: "auction-manager offchain worker",
						"cannot run offchain worker at {:?}: {:?}",
						now,
						e,
					);
				} else {
					debug::debug!(
						target: "auction-manager offchain worker",
						"offchain worker start at block: {:?} already done!",
						now,
					);
				}
			}
		}
	}
}

impl<T: Trait> Module<T> {
	fn get_last_bid(auction_id: AuctionId) -> Option<(T::AccountId, Balance)> {
		T::Auction::auction_info(auction_id).and_then(|auction_info| auction_info.bid)
	}

	fn submit_cancel_auction_tx(auction_id: AuctionId) {
		let call = Call::<T>::cancel(auction_id);
		if SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()).is_err() {
			debug::info!(
				target: "auction-manager offchain worker",
				"submit unsigned auction cancel tx for \nAuctionId {:?} \nfailed!",
				auction_id,
			);
		}
	}

	fn _offchain_worker() -> Result<(), OffchainErr> {
		let lock_expiration = Duration::from_millis(LOCK_DURATION);
		let mut lock = StorageLock::<'_, Time>::with_deadline(&OFFCHAIN_WORKER_LOCK, lock_expiration);

		// acquire offchain worker lock.
		let mut guard = lock.try_lock().map_err(|_| OffchainErr::OffchainLock)?;

		let random_seed = sp_io::offchain::random_seed();
		let mut rng = RandomNumberGenerator::<BlakeTwo256>::new(BlakeTwo256::hash(&random_seed[..]));

		// Randomly choose to start iterations to cancel collateral/surplus/debit
		// auctions
		match rng.pick_u32(2) {
			0 => {
				for (debit_auction_id, _) in <DebitAuctions<T>>::iter() {
					Self::submit_cancel_auction_tx(debit_auction_id);
					guard.extend_lock().map_err(|_| OffchainErr::OffchainLock)?;
				}
			}
			1 => {
				for (surplus_auction_id, _) in <SurplusAuctions<T>>::iter() {
					Self::submit_cancel_auction_tx(surplus_auction_id);
					guard.extend_lock().map_err(|_| OffchainErr::OffchainLock)?;
				}
			}
			_ => {
				for (collateral_auction_id, _) in <CollateralAuctions<T>>::iter() {
					if let (Some(collateral_auction), Some((_, last_bid_price))) = (
						Self::collateral_auctions(collateral_auction_id),
						Self::get_last_bid(collateral_auction_id),
					) {
						// if collateral auction has already been in reverse stage,
						// should skip it.
						if collateral_auction.in_reverse_stage(last_bid_price) {
							continue;
						}
					}
					Self::submit_cancel_auction_tx(collateral_auction_id);
					guard.extend_lock().map_err(|_| OffchainErr::OffchainLock)?;
				}
			}
		}

		Ok(())
	}

	fn cancel_surplus_auction(id: AuctionId) -> DispatchResult {
		let surplus_auction = <SurplusAuctions<T>>::take(id).ok_or(Error::<T>::AuctionNotExists)?;

		// if there's bid
		if let Some((bidder, bid_price)) = Self::get_last_bid(id) {
			// refund native token to the bidder
			// TODO: transfer from RESERVED TREASURY instead of issuing
			T::Currency::deposit(T::GetNativeCurrencyId::get(), &bidder, bid_price)?;

			// decrease account ref of bidder
			system::Module::<T>::dec_ref(&bidder);
		}

		// decrease total surplus in auction
		TotalSurplusInAuction::mutate(|balance| *balance = balance.saturating_sub(surplus_auction.amount));

		// remove the auction info
		T::Auction::remove_auction(id);

		Ok(())
	}

	fn cancel_debit_auction(id: AuctionId) -> DispatchResult {
		let debit_auction = <DebitAuctions<T>>::take(id).ok_or(Error::<T>::AuctionNotExists)?;

		// if there's bid
		if let Some((bidder, _)) = Self::get_last_bid(id) {
			// refund stable token to the bidder
			T::CDPTreasury::issue_debit(&bidder, debit_auction.fix, false)?;
			// decrease account ref of bidder
			system::Module::<T>::dec_ref(&bidder);
		}

		// decrease total debit in auction
		TotalDebitInAuction::mutate(|balance| *balance = balance.saturating_sub(debit_auction.fix));

		// remove the auction info
		T::Auction::remove_auction(id);

		Ok(())
	}

	fn cancel_collateral_auction(id: AuctionId) -> DispatchResult {
		let collateral_auction = <CollateralAuctions<T>>::get(id).ok_or(Error::<T>::AuctionNotExists)?;
		let last_bid = Self::get_last_bid(id);

		// collateral auction must not be in reverse stage
		if let Some((_, bid_price)) = last_bid {
			ensure!(
				!collateral_auction.in_reverse_stage(bid_price),
				Error::<T>::InReverseStage,
			);
		}

		// calculate how much collateral to offset target in settle price
		let stable_currency_id = T::GetStableCurrencyId::get();
		let settle_price = T::PriceSource::get_relative_price(stable_currency_id, collateral_auction.currency_id)
			.ok_or(Error::<T>::InvalidFeedPrice)?;
		let confiscate_collateral_amount = if collateral_auction.always_forward() {
			collateral_auction.amount
		} else {
			sp_std::cmp::min(
				settle_price.saturating_mul_int(collateral_auction.target),
				collateral_auction.amount,
			)
		};
		let refund_collateral_amount = collateral_auction.amount.saturating_sub(confiscate_collateral_amount);

		// refund remain collateral to refund recipient from CDP treasury
		T::CDPTreasury::withdraw_collateral(
			&collateral_auction.refund_recipient,
			collateral_auction.currency_id,
			refund_collateral_amount,
		)?;

		// if there's bid
		if let Some((bidder, bid_price)) = last_bid {
			// refund stable token to the bidder
			T::CDPTreasury::issue_debit(&bidder, bid_price, false)?;

			// decrease account ref of bidder
			system::Module::<T>::dec_ref(&bidder);
		}

		// decrease account ref of refund recipient
		system::Module::<T>::dec_ref(&collateral_auction.refund_recipient);

		// decrease total collateral and target in auction
		TotalCollateralInAuction::mutate(collateral_auction.currency_id, |balance| {
			*balance = balance.saturating_sub(collateral_auction.amount)
		});
		TotalTargetInAuction::mutate(|balance| *balance = balance.saturating_sub(collateral_auction.target));

		// remove the auction info
		<CollateralAuctions<T>>::remove(id);
		T::Auction::remove_auction(id);

		Ok(())
	}

	/// Return `true` if price increment rate is greater than or equal to
	/// minimum.
	///
	/// Formula: new_price - last_price >=
	///     max(last_price, target_price) * minimum_increment
	fn check_minimum_increment(
		new_price: Balance,
		last_price: Balance,
		target_price: Balance,
		minimum_increment: Rate,
	) -> bool {
		if let (Some(target), Some(result)) = (
			minimum_increment.checked_mul_int(sp_std::cmp::max(target_price, last_price)),
			new_price.checked_sub(last_price),
		) {
			result >= target
		} else {
			false
		}
	}

	fn get_minimum_increment_size(now: T::BlockNumber, start_block: T::BlockNumber) -> Rate {
		if now >= start_block + T::AuctionDurationSoftCap::get() {
			// double the minimum increment size when reach soft cap
			T::MinimumIncrementSize::get().saturating_mul(Rate::saturating_from_integer(2))
		} else {
			T::MinimumIncrementSize::get()
		}
	}

	fn get_auction_time_to_close(now: T::BlockNumber, start_block: T::BlockNumber) -> T::BlockNumber {
		if now >= start_block + T::AuctionDurationSoftCap::get() {
			// halve the extended time of bid when reach soft cap
			T::AuctionTimeToClose::get()
				.checked_div(&2u32.into())
				.expect("cannot overflow with positive divisor; qed")
		} else {
			T::AuctionTimeToClose::get()
		}
	}

	/// Handles collateral auction new bid. Returns `Ok(new_auction_end_time)`
	/// if bid accepted.
	///
	/// Ensured atomic.
	pub fn collateral_auction_bid_handler(
		now: T::BlockNumber,
		id: AuctionId,
		new_bid: (T::AccountId, Balance),
		last_bid: Option<(T::AccountId, Balance)>,
	) -> sp_std::result::Result<T::BlockNumber, DispatchError> {
		with_transaction_result(|| -> sp_std::result::Result<T::BlockNumber, DispatchError> {
			// use `with_transaction_result` to ensure operation is atomic
			<CollateralAuctions<T>>::try_mutate_exists(
				id,
				|collateral_auction| -> sp_std::result::Result<T::BlockNumber, DispatchError> {
					let mut collateral_auction = collateral_auction.as_mut().ok_or(Error::<T>::AuctionNotExists)?;
					let (new_bidder, new_bid_price) = new_bid;
					let last_bid_price = last_bid.clone().map_or(Zero::zero(), |(_, price)| price); // get last bid price

					// ensure new bid price is valid
					ensure!(
						!new_bid_price.is_zero()
							&& Self::check_minimum_increment(
								new_bid_price,
								last_bid_price,
								collateral_auction.target,
								Self::get_minimum_increment_size(now, collateral_auction.start_time),
							),
						Error::<T>::InvalidBidPrice
					);

					let mut payment = collateral_auction.payment_amount(new_bid_price);

					// if there's bid before, return stablecoin from new bidder to last bidder
					if let Some((last_bidder, _)) = last_bid {
						let refund = collateral_auction.payment_amount(last_bid_price);
						T::Currency::transfer(T::GetStableCurrencyId::get(), &new_bidder, &last_bidder, refund)?;
						payment = payment
							.checked_sub(refund)
							.expect("new bid price greater than last bid; qed");

						// decrease account ref of last bidder
						system::Module::<T>::dec_ref(&last_bidder);
					}

					// transfer remain payment from new bidder to CDP treasury
					T::CDPTreasury::deposit_surplus(&new_bidder, payment)?;

					// increase account ref of new bidder
					system::Module::<T>::inc_ref(&new_bidder);

					// if collateral auction will be in reverse stage, refund collateral to it's
					// origin from auction CDP treasury
					if collateral_auction.in_reverse_stage(new_bid_price) {
						let new_collateral_amount = collateral_auction.collateral_amount(last_bid_price, new_bid_price);
						let refund_collateral_amount = collateral_auction.amount.saturating_sub(new_collateral_amount);

						if !refund_collateral_amount.is_zero() {
							T::CDPTreasury::withdraw_collateral(
								&(collateral_auction.refund_recipient),
								collateral_auction.currency_id,
								refund_collateral_amount,
							)?;

							// update total collateral in auction after refund
							TotalCollateralInAuction::mutate(collateral_auction.currency_id, |balance| {
								*balance = balance.saturating_sub(refund_collateral_amount)
							});
							collateral_auction.amount = new_collateral_amount;
						}
					}

					Ok(now + Self::get_auction_time_to_close(now, collateral_auction.start_time))
				},
			)
		})
	}

	/// Handles debit auction new bid. Returns `Ok(new_auction_end_time)` if bid
	/// accepted.
	///
	/// Ensured atomic.
	pub fn debit_auction_bid_handler(
		now: T::BlockNumber,
		id: AuctionId,
		new_bid: (T::AccountId, Balance),
		last_bid: Option<(T::AccountId, Balance)>,
	) -> sp_std::result::Result<T::BlockNumber, DispatchError> {
		with_transaction_result(|| -> sp_std::result::Result<T::BlockNumber, DispatchError> {
			// use `with_transaction_result` to ensure operation is atomic
			<DebitAuctions<T>>::try_mutate_exists(
				id,
				|debit_auction| -> sp_std::result::Result<T::BlockNumber, DispatchError> {
					let mut debit_auction = debit_auction.as_mut().ok_or(Error::<T>::AuctionNotExists)?;
					let (new_bidder, new_bid_price) = new_bid;
					let last_bid_price = last_bid.clone().map_or(Zero::zero(), |(_, price)| price); // get last bid price
					let stable_currency_id = T::GetStableCurrencyId::get();

					ensure!(
						Self::check_minimum_increment(
							new_bid_price,
							last_bid_price,
							debit_auction.fix,
							Self::get_minimum_increment_size(now, debit_auction.start_time),
						) && new_bid_price >= debit_auction.fix,
						Error::<T>::InvalidBidPrice,
					);

					if let Some((last_bidder, _)) = last_bid {
						// there's bid before, transfer the stablecoin from new bidder to last bidder
						T::Currency::transfer(stable_currency_id, &new_bidder, &last_bidder, debit_auction.fix)?;

						// decrease account ref of last bidder
						system::Module::<T>::dec_ref(&last_bidder);
					} else {
						// there's no bid before, transfer stablecoin to CDP treasury
						T::CDPTreasury::deposit_surplus(&new_bidder, debit_auction.fix)?;
					}

					// increase account ref of new bidder
					system::Module::<T>::inc_ref(&new_bidder);

					debit_auction.amount = debit_auction.amount_for_sale(last_bid_price, new_bid_price);

					Ok(now + Self::get_auction_time_to_close(now, debit_auction.start_time))
				},
			)
		})
	}

	/// Handles surplus auction new bid. Returns `Ok(new_auction_end_time)` if
	/// bid accepted.
	///
	/// Ensured atomic.
	pub fn surplus_auction_bid_handler(
		now: T::BlockNumber,
		id: AuctionId,
		new_bid: (T::AccountId, Balance),
		last_bid: Option<(T::AccountId, Balance)>,
	) -> sp_std::result::Result<T::BlockNumber, DispatchError> {
		with_transaction_result(|| -> sp_std::result::Result<T::BlockNumber, DispatchError> {
			// use `with_transaction_result` to ensure operation is atomic
			let surplus_auction = Self::surplus_auctions(id).ok_or(Error::<T>::AuctionNotExists)?;
			let (new_bidder, new_bid_price) = new_bid;
			let last_bid_price = last_bid.clone().map_or(Zero::zero(), |(_, price)| price); // get last bid price
			let native_currency_id = T::GetNativeCurrencyId::get();

			ensure!(
				Self::check_minimum_increment(
					new_bid_price,
					last_bid_price,
					Zero::zero(),
					Self::get_minimum_increment_size(now, surplus_auction.start_time),
				) && !new_bid_price.is_zero(),
				Error::<T>::InvalidBidPrice,
			);

			let mut burn_native_currency_amount = new_bid_price;

			// if there's bid before, transfer the stablecoin from auction manager module to
			// last bidder
			if let Some((last_bidder, _)) = last_bid {
				burn_native_currency_amount = burn_native_currency_amount.saturating_sub(last_bid_price);
				T::Currency::transfer(native_currency_id, &new_bidder, &last_bidder, last_bid_price)?;

				// decrease account ref of last bidder
				system::Module::<T>::dec_ref(&last_bidder);
			}

			// burn remain native token from new bidder
			T::Currency::withdraw(native_currency_id, &new_bidder, burn_native_currency_amount)?;

			// increase account ref of new bidder
			system::Module::<T>::inc_ref(&new_bidder);

			Ok(now + Self::get_auction_time_to_close(now, surplus_auction.start_time))
		})
	}

	fn collateral_auction_end_handler(auction_id: AuctionId, winner: Option<(T::AccountId, Balance)>) {
		if let (Some(collateral_auction), Some((bidder, bid_price))) = (Self::collateral_auctions(auction_id), winner) {
			let stable_currency_id = T::GetStableCurrencyId::get();
			let mut should_deal = true;

			// if bid_price doesn't reach target and trading with DEX will get better result
			if !collateral_auction.in_reverse_stage(bid_price)
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

					// refund stable currency to the last bidder, ignore result to continue
					let _ = T::CDPTreasury::issue_debit(&bidder, bid_price, false);

					if collateral_auction.in_reverse_stage(amount) {
						// refund extra stable currency to recipient, ignore result to continue
						let _ = T::CDPTreasury::issue_debit(
							&collateral_auction.refund_recipient,
							amount
								.checked_sub(collateral_auction.target)
								.expect("ensured amount > target; qed"),
							false,
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
				// transfer collateral to winner from CDP treasury, ignore result to continue
				let _ = T::CDPTreasury::withdraw_collateral(
					&bidder,
					collateral_auction.currency_id,
					collateral_auction.amount,
				);

				let payment_amount = collateral_auction.payment_amount(bid_price);
				<Module<T>>::deposit_event(RawEvent::CollateralAuctionDealt(
					auction_id,
					collateral_auction.currency_id,
					collateral_auction.amount,
					bidder.clone(),
					payment_amount,
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

	fn debit_auction_end_handler(auction_id: AuctionId, winner: Option<(T::AccountId, Balance)>) {
		if let Some(debit_auction) = Self::debit_auctions(auction_id) {
			if let Some((bidder, _)) = winner {
				// issue native token to winner, ignore the result to continue
				// TODO: transfer from RESERVED TREASURY instead of issuing
				let _ = T::Currency::deposit(T::GetNativeCurrencyId::get(), &bidder, debit_auction.amount);

				// decrease account ref of winner
				system::Module::<T>::dec_ref(&bidder);

				// decrease debit in auction and delete auction
				TotalDebitInAuction::mutate(|balance| *balance = balance.saturating_sub(debit_auction.fix));
				<DebitAuctions<T>>::remove(auction_id);

				<Module<T>>::deposit_event(RawEvent::DebitAuctionDealt(
					auction_id,
					debit_auction.amount,
					bidder,
					debit_auction.fix,
				));
			} else {
				// there's no bidder until auction closed, adjust the native token amount
				let start_block = <system::Module<T>>::block_number();
				let end_block = start_block + T::AuctionTimeToClose::get();
				let new_debit_auction_id: AuctionId = T::Auction::new_auction(start_block, Some(end_block))
					.expect("AuctionId is sufficient large so this can never fail");
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

	fn surplus_auction_end_handler(auction_id: AuctionId, winner: Option<(T::AccountId, Balance)>) {
		if let (Some(surplus_auction), Some((bidder, bidder_price))) = (Self::surplus_auctions(auction_id), winner) {
			// deposit unbacked stable token to winner by CDP treasury, ignore Err
			let _ = T::CDPTreasury::issue_debit(&bidder, surplus_auction.amount, false);

			// decrease account ref of winner
			system::Module::<T>::dec_ref(&bidder);

			// decrease surplus in auction
			TotalSurplusInAuction::mutate(|balance| *balance = balance.saturating_sub(surplus_auction.amount));
			<SurplusAuctions<T>>::remove(auction_id);

			<Module<T>>::deposit_event(RawEvent::SurplusAuctionDealt(
				auction_id,
				surplus_auction.amount,
				bidder,
				bidder_price,
			));
		}
	}
}

impl<T: Trait> AuctionHandler<T::AccountId, Balance, T::BlockNumber, AuctionId> for Module<T> {
	fn on_new_bid(
		now: T::BlockNumber,
		id: AuctionId,
		new_bid: (T::AccountId, Balance),
		last_bid: Option<(T::AccountId, Balance)>,
	) -> OnNewBidResult<T::BlockNumber> {
		let bid_result = if <CollateralAuctions<T>>::contains_key(id) {
			Self::collateral_auction_bid_handler(now, id, new_bid, last_bid)
		} else if <DebitAuctions<T>>::contains_key(id) {
			Self::debit_auction_bid_handler(now, id, new_bid, last_bid)
		} else if <SurplusAuctions<T>>::contains_key(id) {
			Self::surplus_auction_bid_handler(now, id, new_bid, last_bid)
		} else {
			Err(Error::<T>::AuctionNotExists.into())
		};

		match bid_result {
			Ok(new_auction_end_time) => OnNewBidResult {
				accept_bid: true,
				auction_end_change: Change::NewValue(Some(new_auction_end_time)),
			},
			Err(_) => OnNewBidResult {
				accept_bid: false,
				auction_end_change: Change::NoChange,
			},
		}
	}

	fn on_auction_ended(id: AuctionId, winner: Option<(T::AccountId, Balance)>) {
		if <CollateralAuctions<T>>::contains_key(id) {
			Self::collateral_auction_end_handler(id, winner);
		} else if <DebitAuctions<T>>::contains_key(id) {
			Self::debit_auction_end_handler(id, winner);
		} else if <SurplusAuctions<T>>::contains_key(id) {
			Self::surplus_auction_end_handler(id, winner);
		}
	}
}

impl<T: Trait> AuctionManager<T::AccountId> for Module<T> {
	type CurrencyId = CurrencyId;
	type Balance = Balance;
	type AuctionId = AuctionId;

	fn new_collateral_auction(
		who: &T::AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
		target: Self::Balance,
	) {
		if let (Some(new_total_collateral), Some(new_total_target)) = (
			Self::total_collateral_in_auction(currency_id).checked_add(amount),
			Self::total_target_in_auction().checked_add(target),
		) {
			TotalCollateralInAuction::insert(currency_id, new_total_collateral);
			TotalTargetInAuction::put(new_total_target);

			let block_number = <system::Module<T>>::block_number();
			let auction_id: AuctionId = T::Auction::new_auction(block_number, None)
				.expect("AuctionId is sufficient large so this can never fail"); // do not set end time for collateral auction
			let collateral_auction = CollateralAuctionItem {
				refund_recipient: who.clone(),
				currency_id,
				amount,
				target,
				start_time: block_number,
			};

			// increase account ref of refund recipient
			system::Module::<T>::inc_ref(&who);

			<CollateralAuctions<T>>::insert(auction_id, collateral_auction);
			<Module<T>>::deposit_event(RawEvent::NewCollateralAuction(auction_id, currency_id, amount, target));
		}
	}

	fn new_debit_auction(initial_amount: Self::Balance, fix_debit: Self::Balance) {
		if let Some(new_total_debit) = Self::total_debit_in_auction().checked_add(fix_debit) {
			TotalDebitInAuction::put(new_total_debit);
			let start_block = <system::Module<T>>::block_number();
			let end_block = start_block + T::AuctionTimeToClose::get();
			// set close time for initial debit auction
			let auction_id: AuctionId = T::Auction::new_auction(start_block, Some(end_block))
				.expect("AuctionId is sufficient large so this can never fail");
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
		if let Some(new_total_surplus) = Self::total_surplus_in_auction().checked_add(amount) {
			TotalSurplusInAuction::put(new_total_surplus);
			// do not set end time for surplus auction
			let auction_id: AuctionId = T::Auction::new_auction(<system::Module<T>>::block_number(), None)
				.expect("AuctionId is sufficient large so this can never fail");
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
			Err(Error::<T>::AuctionNotExists.into())
		}
	}
}

#[allow(deprecated)]
impl<T: Trait> frame_support::unsigned::ValidateUnsigned for Module<T> {
	type Call = Call<T>;

	fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
		if let Call::cancel(auction_id) = call {
			if !T::EmergencyShutdown::is_shutdown() {
				return InvalidTransaction::Stale.into();
			}

			if let Some(collateral_auction) = Self::collateral_auctions(auction_id) {
				if let Some((_, bid_price)) = Self::get_last_bid(*auction_id) {
					// if collateral auction is in reverse stage, shouldn't cancel
					if collateral_auction.in_reverse_stage(bid_price) {
						return InvalidTransaction::Stale.into();
					}
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
