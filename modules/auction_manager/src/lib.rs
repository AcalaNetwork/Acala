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
	weights::{DispatchClass, Weight},
};
use frame_system::{
	self as system, ensure_none,
	offchain::{SendTransactionTypes, SubmitTransaction},
};
use orml_traits::{Auction, AuctionHandler, Change, MultiCurrency, OnNewBidResult};
use orml_utilities::{with_transaction_result, IterableStorageMapExtended, OffchainErr};
use primitives::{AuctionId, Balance, CurrencyId};
use sp_runtime::{
	offchain::{
		storage::StorageValueRef,
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

mod default_weight;
mod mock;
mod tests;

pub trait WeightInfo {
	fn cancel_surplus_auction() -> Weight;
	fn cancel_debit_auction() -> Weight;
	fn cancel_collateral_auction() -> Weight;
}

const OFFCHAIN_WORKER_DATA: &[u8] = b"acala/auction-manager/data/";
const OFFCHAIN_WORKER_LOCK: &[u8] = b"acala/auction-manager/lock/";
const OFFCHAIN_WORKER_MAX_ITERATIONS: &[u8] = b"acala/auction-manager/max-iterations/";
const LOCK_DURATION: u64 = 100;
const DEFAULT_MAX_ITERATIONS: u32 = 1000;

/// Information of an collateral auction
#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct CollateralAuctionItem<AccountId, BlockNumber> {
	/// Refund recipient for may receive refund
	refund_recipient: AccountId,
	/// Collateral type for sale
	currency_id: CurrencyId,
	/// Initial collateral amount for sale
	#[codec(compact)]
	initial_amount: Balance,
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
	/// Initial amount of native currency for sale
	#[codec(compact)]
	initial_amount: Balance,
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

	/// Weight information for the extrinsics in this module.
	type WeightInfo: WeightInfo;
}

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
		AuctionId = AuctionId,
		CurrencyId = CurrencyId,
		Balance = Balance,
	{
		/// Collateral auction created. \[auction_id, collateral_type, collateral_amount, target_bid_price\]
		NewCollateralAuction(AuctionId, CurrencyId, Balance, Balance),
		/// Debit auction created. \[auction_id, initial_supply_amount, fix_payment_amount\]
		NewDebitAuction(AuctionId, Balance, Balance),
		/// Surplus auction created. \[auction_id, fix_surplus_amount\]
		NewSurplusAuction(AuctionId, Balance),
		/// Active auction cancelled. \[auction_id\]
		CancelAuction(AuctionId),
		/// Collateral auction dealt. \[auction_id, collateral_type, collateral_amount, winner, payment_amount\]
		CollateralAuctionDealt(AuctionId, CurrencyId, Balance, AccountId, Balance),
		/// Surplus auction dealt. \[auction_id, surplus_amount, winner, payment_amount\]
		SurplusAuctionDealt(AuctionId, Balance, AccountId, Balance),
		/// Debit auction dealt. \[auction_id, debit_currency_amount, winner, payment_amount\]
		DebitAuctionDealt(AuctionId, Balance, AccountId, Balance),
		/// Dex take collateral auction. \[auction_id, collateral_type, collateral_amount, turnover\]
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
		/// Invalid input amount
		InvalidAmount,
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
		/// Use the collateral auction worst case as default weight.
		#[weight = (T::WeightInfo::cancel_collateral_auction(), DispatchClass::Operational)]
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
		if let Err(err) = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()) {
			debug::info!(
				target: "auction-manager offchain worker",
				"submit unsigned auction cancel tx for \nAuctionId {:?} \nfailed: {:?}",
				auction_id,
				err,
			);
		}
	}

	fn _offchain_worker() -> Result<(), OffchainErr> {
		// acquire offchain worker lock.
		let lock_expiration = Duration::from_millis(LOCK_DURATION);
		let mut lock = StorageLock::<'_, Time>::with_deadline(&OFFCHAIN_WORKER_LOCK, lock_expiration);
		let mut guard = lock.try_lock().map_err(|_| OffchainErr::OffchainLock)?;

		let mut to_be_continue = StorageValueRef::persistent(&OFFCHAIN_WORKER_DATA);

		// get to_be_continue record,
		// if it exsits, iterator map storage start with previous key
		let (auction_type_num, start_key) = if let Some(Some((auction_type_num, last_iterator_previous_key))) =
			to_be_continue.get::<(u32, Vec<u8>)>()
		{
			(auction_type_num, Some(last_iterator_previous_key))
		} else {
			let random_seed = sp_io::offchain::random_seed();
			let mut rng = RandomNumberGenerator::<BlakeTwo256>::new(BlakeTwo256::hash(&random_seed[..]));
			(rng.pick_u32(2), None)
		};

		// get the max iterationns config
		let max_iterations = StorageValueRef::persistent(&OFFCHAIN_WORKER_MAX_ITERATIONS)
			.get::<u32>()
			.unwrap_or(Some(DEFAULT_MAX_ITERATIONS));

		debug::debug!(target: "auction-manager offchain worker", "max iterations is {:?}", max_iterations);

		// Randomly choose to start iterations to cancel collateral/surplus/debit
		// auctions
		match auction_type_num {
			0 => {
				let mut iterator =
					<DebitAuctions<T> as IterableStorageMapExtended<_, _>>::iter(max_iterations, start_key);
				while let Some((debit_auction_id, _)) = iterator.next() {
					Self::submit_cancel_auction_tx(debit_auction_id);
					guard.extend_lock().map_err(|_| OffchainErr::OffchainLock)?;
				}

				// if iteration for map storage finished, clear to be continue record
				// otherwise, update to be continue record
				if iterator.finished {
					to_be_continue.clear();
				} else {
					to_be_continue.set(&(auction_type_num, iterator.storage_map_iterator.previous_key));
				}
			}
			1 => {
				let mut iterator =
					<SurplusAuctions<T> as IterableStorageMapExtended<_, _>>::iter(max_iterations, start_key);
				while let Some((surplus_auction_id, _)) = iterator.next() {
					Self::submit_cancel_auction_tx(surplus_auction_id);
					guard.extend_lock().map_err(|_| OffchainErr::OffchainLock)?;
				}

				if iterator.finished {
					to_be_continue.clear();
				} else {
					to_be_continue.set(&(auction_type_num, iterator.storage_map_iterator.previous_key));
				}
			}
			_ => {
				let mut iterator =
					<CollateralAuctions<T> as IterableStorageMapExtended<_, _>>::iter(max_iterations, start_key);
				while let Some((collateral_auction_id, _)) = iterator.next() {
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

				if iterator.finished {
					to_be_continue.clear();
				} else {
					to_be_continue.set(&(auction_type_num, iterator.storage_map_iterator.previous_key));
				}
			}
		}

		// Consume the guard but **do not** unlock the underlying lock.
		guard.forget();

		Ok(())
	}

	fn cancel_surplus_auction(id: AuctionId, surplus_auction: SurplusAuctionItem<T::BlockNumber>) -> DispatchResult {
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

		Ok(())
	}

	fn cancel_debit_auction(id: AuctionId, debit_auction: DebitAuctionItem<T::BlockNumber>) -> DispatchResult {
		// if there's bid
		if let Some((bidder, _)) = Self::get_last_bid(id) {
			// refund stable token to the bidder
			T::CDPTreasury::issue_debit(&bidder, debit_auction.fix, false)?;
			// decrease account ref of bidder
			system::Module::<T>::dec_ref(&bidder);
		}

		// decrease total debit in auction
		TotalDebitInAuction::mutate(|balance| *balance = balance.saturating_sub(debit_auction.fix));

		Ok(())
	}

	fn cancel_collateral_auction(
		id: AuctionId,
		collateral_auction: CollateralAuctionItem<T::AccountId, T::BlockNumber>,
	) -> DispatchResult {
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
		let (new_bidder, new_bid_price) = new_bid;
		ensure!(!new_bid_price.is_zero(), Error::<T>::InvalidBidPrice);

		with_transaction_result(|| -> sp_std::result::Result<T::BlockNumber, DispatchError> {
			// use `with_transaction_result` to ensure operation is atomic
			<CollateralAuctions<T>>::try_mutate_exists(
				id,
				|collateral_auction| -> sp_std::result::Result<T::BlockNumber, DispatchError> {
					let mut collateral_auction = collateral_auction.as_mut().ok_or(Error::<T>::AuctionNotExists)?;
					let last_bid_price = last_bid.clone().map_or(Zero::zero(), |(_, price)| price); // get last bid price

					// ensure new bid price is valid
					ensure!(
						Self::check_minimum_increment(
							new_bid_price,
							last_bid_price,
							collateral_auction.target,
							Self::get_minimum_increment_size(now, collateral_auction.start_time),
						),
						Error::<T>::InvalidBidPrice
					);

					let last_bidder = last_bid.as_ref().map(|(who, _)| who);

					let mut payment = collateral_auction.payment_amount(new_bid_price);

					// if there's bid before, return stablecoin from new bidder to last bidder
					if let Some(last_bidder) = last_bidder {
						let refund = collateral_auction.payment_amount(last_bid_price);
						T::Currency::transfer(T::GetStableCurrencyId::get(), &new_bidder, last_bidder, refund)?;

						payment = payment
							.checked_sub(refund)
							// This should never fail because new bid payment are always greater or equal to last bid
							// payment.
							.ok_or(Error::<T>::InvalidBidPrice)?;
					}

					// transfer remain payment from new bidder to CDP treasury
					T::CDPTreasury::deposit_surplus(&new_bidder, payment)?;

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

					Self::swap_bidders(&new_bidder, last_bidder);

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

					ensure!(
						Self::check_minimum_increment(
							new_bid_price,
							last_bid_price,
							debit_auction.fix,
							Self::get_minimum_increment_size(now, debit_auction.start_time),
						) && new_bid_price >= debit_auction.fix,
						Error::<T>::InvalidBidPrice,
					);

					let last_bidder = last_bid.as_ref().map(|(who, _)| who);

					if let Some(last_bidder) = last_bidder {
						// there's bid before, transfer the stablecoin from new bidder to last bidder
						T::Currency::transfer(
							T::GetStableCurrencyId::get(),
							&new_bidder,
							last_bidder,
							debit_auction.fix,
						)?;
					} else {
						// there's no bid before, transfer stablecoin to CDP treasury
						T::CDPTreasury::deposit_surplus(&new_bidder, debit_auction.fix)?;
					}

					Self::swap_bidders(&new_bidder, last_bidder);

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
		let (new_bidder, new_bid_price) = new_bid;
		ensure!(!new_bid_price.is_zero(), Error::<T>::InvalidBidPrice);

		with_transaction_result(|| -> sp_std::result::Result<T::BlockNumber, DispatchError> {
			// use `with_transaction_result` to ensure operation is atomic
			let surplus_auction = Self::surplus_auctions(id).ok_or(Error::<T>::AuctionNotExists)?;
			let last_bid_price = last_bid.clone().map_or(Zero::zero(), |(_, price)| price); // get last bid price
			let native_currency_id = T::GetNativeCurrencyId::get();

			ensure!(
				Self::check_minimum_increment(
					new_bid_price,
					last_bid_price,
					Zero::zero(),
					Self::get_minimum_increment_size(now, surplus_auction.start_time),
				),
				Error::<T>::InvalidBidPrice,
			);

			let last_bidder = last_bid.as_ref().map(|(who, _)| who);

			let burn_amount = if let Some(last_bidder) = last_bidder {
				// refund last bidder
				T::Currency::transfer(native_currency_id, &new_bidder, last_bidder, last_bid_price)?;
				new_bid_price.saturating_sub(last_bid_price)
			} else {
				new_bid_price
			};

			// burn remain native token from new bidder
			T::Currency::withdraw(native_currency_id, &new_bidder, burn_amount)?;

			Self::swap_bidders(&new_bidder, last_bidder);

			Ok(now + Self::get_auction_time_to_close(now, surplus_auction.start_time))
		})
	}

	fn collateral_auction_end_handler(
		auction_id: AuctionId,
		collateral_auction: CollateralAuctionItem<T::AccountId, T::BlockNumber>,
		winner: Option<(T::AccountId, Balance)>,
	) {
		if let Some((bidder, bid_price)) = winner {
			let mut should_deal = true;

			// if bid_price doesn't reach target and trading with DEX will get better result
			if !collateral_auction.in_reverse_stage(bid_price)
				&& bid_price
					< T::DEX::get_target_amount(
						collateral_auction.currency_id,
						T::GetStableCurrencyId::get(),
						collateral_auction.amount,
					)
					.unwrap_or_default()
			{
				// try trade with DEX
				if let Ok(amount) = T::CDPTreasury::swap_collateral_to_stable(
					collateral_auction.currency_id,
					collateral_auction.amount,
					Zero::zero(),
				) {
					// swap successfully, will not deal
					should_deal = false;

					// refund stable currency to the last bidder, it shouldn't fail and affect the
					// process. but even it failed, just the winner did not get the amount. it can
					// be fixed by treasury council.
					let _ = T::CDPTreasury::issue_debit(&bidder, bid_price, false);

					if collateral_auction.in_reverse_stage(amount) {
						// refund extra stable currency to recipient
						let refund_amount = amount
							.checked_sub(collateral_auction.target)
							.expect("ensured amount > target; qed");
						// it shouldn't fail and affect the process.
						// but even it failed, just the winner did not get the amount. it can be fixed
						// by treasury council.
						let _ = T::CDPTreasury::issue_debit(&collateral_auction.refund_recipient, refund_amount, false);
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
				// transfer collateral to winner from CDP treasury, it shouldn't fail and affect
				// the process. but even it failed, just the winner did not get the amount. it
				// can be fixed by treasury council.
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
					bidder,
					payment_amount,
				));
			}
		} else {
			<Module<T>>::deposit_event(RawEvent::CancelAuction(auction_id));
		}

		// decrement recipient account reference
		system::Module::<T>::dec_ref(&collateral_auction.refund_recipient);

		// update auction records
		TotalCollateralInAuction::mutate(collateral_auction.currency_id, |balance| {
			*balance = balance.saturating_sub(collateral_auction.amount)
		});
		TotalTargetInAuction::mutate(|balance| *balance = balance.saturating_sub(collateral_auction.target));
	}

	fn debit_auction_end_handler(
		auction_id: AuctionId,
		debit_auction: DebitAuctionItem<T::BlockNumber>,
		winner: Option<(T::AccountId, Balance)>,
	) {
		if let Some((bidder, _)) = winner {
			// issue native token to winner, it shouldn't fail and affect the process.
			// but even it failed, just the winner did not get the amount. it can be fixed
			// by treasury council. TODO: transfer from RESERVED TREASURY instead of issuing
			let _ = T::Currency::deposit(T::GetNativeCurrencyId::get(), &bidder, debit_auction.amount);

			<Module<T>>::deposit_event(RawEvent::DebitAuctionDealt(
				auction_id,
				debit_auction.amount,
				bidder,
				debit_auction.fix,
			));
		} else {
			<Module<T>>::deposit_event(RawEvent::CancelAuction(auction_id));
		}

		TotalDebitInAuction::mutate(|balance| *balance = balance.saturating_sub(debit_auction.fix));
	}

	fn surplus_auction_end_handler(
		auction_id: AuctionId,
		surplus_auction: SurplusAuctionItem<T::BlockNumber>,
		winner: Option<(T::AccountId, Balance)>,
	) {
		if let Some((bidder, bid_price)) = winner {
			// deposit unbacked stable token to winner by CDP treasury, it shouldn't fail
			// and affect the process. but even it failed, just the winner did not get the
			// amount. it can be fixed by treasury council.
			let _ = T::CDPTreasury::issue_debit(&bidder, surplus_auction.amount, false);

			<Module<T>>::deposit_event(RawEvent::SurplusAuctionDealt(
				auction_id,
				surplus_auction.amount,
				bidder,
				bid_price,
			));
		} else {
			<Module<T>>::deposit_event(RawEvent::CancelAuction(auction_id));
		}

		TotalSurplusInAuction::mutate(|balance| *balance = balance.saturating_sub(surplus_auction.amount));
	}

	/// increment `new_bidder` reference and decrement `last_bidder` reference
	/// if any
	fn swap_bidders(new_bidder: &T::AccountId, last_bidder: Option<&T::AccountId>) {
		system::Module::<T>::inc_ref(new_bidder);

		if let Some(who) = last_bidder {
			system::Module::<T>::dec_ref(who);
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
		if let Some((bidder, _)) = &winner {
			// decrease account ref of winner
			system::Module::<T>::dec_ref(bidder);
		}

		if let Some(collateral_auction) = <CollateralAuctions<T>>::take(id) {
			Self::collateral_auction_end_handler(id, collateral_auction, winner);
		} else if let Some(debit_auction) = <DebitAuctions<T>>::take(id) {
			Self::debit_auction_end_handler(id, debit_auction, winner);
		} else if let Some(surplus_auction) = <SurplusAuctions<T>>::take(id) {
			Self::surplus_auction_end_handler(id, surplus_auction, winner);
		}
	}
}

impl<T: Trait> AuctionManager<T::AccountId> for Module<T> {
	type CurrencyId = CurrencyId;
	type Balance = Balance;
	type AuctionId = AuctionId;

	fn new_collateral_auction(
		refund_recipient: &T::AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
		target: Self::Balance,
	) -> DispatchResult {
		ensure!(!amount.is_zero(), Error::<T>::InvalidAmount);
		TotalCollateralInAuction::try_mutate(currency_id, |total| -> DispatchResult {
			*total = total.checked_add(amount).ok_or(Error::<T>::InvalidAmount)?;
			Ok(())
		})?;

		if !target.is_zero() {
			// no-op if target is zero
			TotalTargetInAuction::try_mutate(|total| -> DispatchResult {
				*total = total.checked_add(target).ok_or(Error::<T>::InvalidAmount)?;
				Ok(())
			})?;
		}

		let start_time = <system::Module<T>>::block_number();

		// do not set end time for collateral auction
		let auction_id = T::Auction::new_auction(start_time, None)?;

		<CollateralAuctions<T>>::insert(
			auction_id,
			CollateralAuctionItem {
				refund_recipient: refund_recipient.clone(),
				currency_id,
				initial_amount: amount,
				amount,
				target,
				start_time,
			},
		);

		// increment recipient account reference
		system::Module::<T>::inc_ref(&refund_recipient);

		<Module<T>>::deposit_event(RawEvent::NewCollateralAuction(auction_id, currency_id, amount, target));
		Ok(())
	}

	fn new_debit_auction(initial_amount: Self::Balance, fix_debit: Self::Balance) -> DispatchResult {
		ensure!(
			!initial_amount.is_zero() && !fix_debit.is_zero(),
			Error::<T>::InvalidAmount,
		);
		TotalDebitInAuction::try_mutate(|total| -> DispatchResult {
			*total = total.checked_add(fix_debit).ok_or(Error::<T>::InvalidAmount)?;
			Ok(())
		})?;

		let start_time = <system::Module<T>>::block_number();
		let end_block = start_time + T::AuctionTimeToClose::get();

		// set end time for debit auction
		let auction_id = T::Auction::new_auction(start_time, Some(end_block))?;

		<DebitAuctions<T>>::insert(
			auction_id,
			DebitAuctionItem {
				initial_amount,
				amount: initial_amount,
				fix: fix_debit,
				start_time,
			},
		);

		<Module<T>>::deposit_event(RawEvent::NewDebitAuction(auction_id, initial_amount, fix_debit));
		Ok(())
	}

	fn new_surplus_auction(amount: Self::Balance) -> DispatchResult {
		ensure!(!amount.is_zero(), Error::<T>::InvalidAmount,);
		TotalSurplusInAuction::try_mutate(|total| -> DispatchResult {
			*total = total.checked_add(amount).ok_or(Error::<T>::InvalidAmount)?;
			Ok(())
		})?;

		let start_time = <system::Module<T>>::block_number();

		// do not set end time for surplus auction
		let auction_id = T::Auction::new_auction(start_time, None)?;

		<SurplusAuctions<T>>::insert(auction_id, SurplusAuctionItem { amount, start_time });

		<Module<T>>::deposit_event(RawEvent::NewSurplusAuction(auction_id, amount));
		Ok(())
	}

	fn cancel_auction(id: Self::AuctionId) -> DispatchResult {
		if let Some(collateral_auction) = <CollateralAuctions<T>>::take(id) {
			Self::cancel_collateral_auction(id, collateral_auction)?;
		} else if let Some(debit_auction) = <DebitAuctions<T>>::take(id) {
			Self::cancel_debit_auction(id, debit_auction)?;
		} else if let Some(surplus_auction) = <SurplusAuctions<T>>::take(id) {
			Self::cancel_surplus_auction(id, surplus_auction)?;
		} else {
			return Err(Error::<T>::AuctionNotExists.into());
		}
		T::Auction::remove_auction(id);
		Ok(())
	}

	fn get_total_collateral_in_auction(id: Self::CurrencyId) -> Self::Balance {
		Self::total_collateral_in_auction(id)
	}

	fn get_total_surplus_in_auction() -> Self::Balance {
		Self::total_surplus_in_auction()
	}

	fn get_total_debit_in_auction() -> Self::Balance {
		Self::total_debit_in_auction()
	}

	fn get_total_target_in_auction() -> Self::Balance {
		Self::total_target_in_auction()
	}
}

impl<T: Trait> frame_support::unsigned::ValidateUnsigned for Module<T> {
	type Call = Call<T>;

	fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
		if let Call::cancel(auction_id) = call {
			if !T::EmergencyShutdown::is_shutdown() {
				return InvalidTransaction::Call.into();
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
