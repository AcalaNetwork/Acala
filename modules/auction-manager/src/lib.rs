// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! # Auction Manager Module
//!
//! ## Overview
//!
//! Auction the assets of the system for maintain the normal operation of the
//! business. Auction types include:
//!   - `collateral auction`: sell collateral assets for getting stable currency to eliminate the
//!     system's bad debit by auction

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

use frame_support::{log, pallet_prelude::*, transactional};
use frame_system::{
	offchain::{SendTransactionTypes, SubmitTransaction},
	pallet_prelude::*,
};
use orml_traits::{Auction, AuctionHandler, Change, MultiCurrency, OnNewBidResult};
use orml_utilities::{IterableStorageMapExtended, OffchainErr};
use primitives::{AuctionId, Balance, CurrencyId};
use sp_runtime::{
	offchain::{
		storage::StorageValueRef,
		storage_lock::{StorageLock, Time},
		Duration,
	},
	traits::{CheckedDiv, Saturating, Zero},
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity, ValidTransaction,
	},
	DispatchError, DispatchResult, FixedPointNumber, RuntimeDebug,
};
use sp_std::prelude::*;
use support::{AuctionManager, CDPTreasury, CDPTreasuryExtended, DEXManager, EmergencyShutdown, PriceProvider, Rate};

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

pub const OFFCHAIN_WORKER_DATA: &[u8] = b"acala/auction-manager/data/";
pub const OFFCHAIN_WORKER_LOCK: &[u8] = b"acala/auction-manager/lock/";
pub const OFFCHAIN_WORKER_MAX_ITERATIONS: &[u8] = b"acala/auction-manager/max-iterations/";
pub const LOCK_DURATION: u64 = 100;
pub const DEFAULT_MAX_ITERATIONS: u32 = 1000;

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
	/// otherwise, target amount is the actual payment amount of active
	/// bidder
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

	/// Return whether the collateral auction is in reverse stage at
	/// specific bid price
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

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config + SendTransactionTypes<Call<Self>> {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The minimum increment size of each bid compared to the previous one
		#[pallet::constant]
		type MinimumIncrementSize: Get<Rate>;

		/// The extended time for the auction to end after each successful bid
		#[pallet::constant]
		type AuctionTimeToClose: Get<Self::BlockNumber>;

		/// When the total duration of the auction exceeds this soft cap, push
		/// the auction to end more faster
		#[pallet::constant]
		type AuctionDurationSoftCap: Get<Self::BlockNumber>;

		/// The stable currency id
		#[pallet::constant]
		type GetStableCurrencyId: Get<CurrencyId>;

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
		#[pallet::constant]
		type UnsignedPriority: Get<TransactionPriority>;

		/// Emergency shutdown.
		type EmergencyShutdown: EmergencyShutdown;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
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

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Collateral auction created. \[auction_id, collateral_type,
		/// collateral_amount, target_bid_price\]
		NewCollateralAuction(AuctionId, CurrencyId, Balance, Balance),
		/// Active auction cancelled. \[auction_id\]
		CancelAuction(AuctionId),
		/// Collateral auction dealt. \[auction_id, collateral_type,
		/// collateral_amount, winner, payment_amount\]
		CollateralAuctionDealt(AuctionId, CurrencyId, Balance, T::AccountId, Balance),
		/// Dex take collateral auction. \[auction_id, collateral_type,
		/// collateral_amount, turnover\]
		DEXTakeCollateralAuction(AuctionId, CurrencyId, Balance, Balance),
	}

	/// Mapping from auction id to collateral auction info
	///
	/// CollateralAuctions: map AuctionId => Option<CollateralAuctionItem>
	#[pallet::storage]
	#[pallet::getter(fn collateral_auctions)]
	pub type CollateralAuctions<T: Config> =
		StorageMap<_, Twox64Concat, AuctionId, CollateralAuctionItem<T::AccountId, T::BlockNumber>, OptionQuery>;

	/// Record of the total collateral amount of all active collateral auctions
	/// under specific collateral type CollateralType -> TotalAmount
	///
	/// TotalCollateralInAuction: map CurrencyId => Balance
	#[pallet::storage]
	#[pallet::getter(fn total_collateral_in_auction)]
	pub type TotalCollateralInAuction<T: Config> = StorageMap<_, Twox64Concat, CurrencyId, Balance, ValueQuery>;

	/// Record of total target sales of all active collateral auctions
	///
	/// TotalTargetInAuction: Balance
	#[pallet::storage]
	#[pallet::getter(fn total_target_in_auction)]
	pub type TotalTargetInAuction<T: Config> = StorageValue<_, Balance, ValueQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		/// Start offchain worker in order to submit unsigned tx to cancel
		/// active auction after system shutdown.
		fn offchain_worker(now: T::BlockNumber) {
			if T::EmergencyShutdown::is_shutdown() && sp_io::offchain::is_validator() {
				if let Err(e) = Self::_offchain_worker() {
					log::info!(
						target: "auction-manager",
						"offchain worker: cannot run offchain worker at {:?}: {:?}",
						now, e,
					);
				} else {
					log::debug!(
						target: "auction-manager",
						"offchain worker: offchain worker start at block: {:?} already done!",
						now,
					);
				}
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Cancel active auction after system shutdown
		///
		/// The dispatch origin of this call must be _None_.
		#[pallet::weight(T::WeightInfo::cancel_collateral_auction())]
		#[transactional]
		pub fn cancel(origin: OriginFor<T>, id: AuctionId) -> DispatchResultWithPostInfo {
			ensure_none(origin)?;
			ensure!(T::EmergencyShutdown::is_shutdown(), Error::<T>::MustAfterShutdown);
			<Self as AuctionManager<T::AccountId>>::cancel_auction(id)?;
			Self::deposit_event(Event::CancelAuction(id));
			Ok(().into())
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
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
				} else {
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
}

impl<T: Config> Pallet<T> {
	fn get_last_bid(auction_id: AuctionId) -> Option<(T::AccountId, Balance)> {
		T::Auction::auction_info(auction_id).and_then(|auction_info| auction_info.bid)
	}

	fn submit_cancel_auction_tx(auction_id: AuctionId) {
		let call = Call::<T>::cancel(auction_id);
		if let Err(err) = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()) {
			log::info!(
				target: "auction-manager",
				"offchain worker: submit unsigned auction cancel tx for AuctionId {:?} failed: {:?}",
				auction_id, err,
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
		let start_key = to_be_continue.get::<Vec<u8>>().flatten();

		// get the max iterationns config
		let max_iterations = StorageValueRef::persistent(&OFFCHAIN_WORKER_MAX_ITERATIONS)
			.get::<u32>()
			.unwrap_or(Some(DEFAULT_MAX_ITERATIONS));

		log::debug!(
			target: "auction-manager",
			"offchain worker: max iterations is {:?}",
			max_iterations
		);

		// start iterations to cancel collateral auctions
		let mut iterator = <CollateralAuctions<T> as IterableStorageMapExtended<_, _>>::iter(max_iterations, start_key);

		#[allow(clippy::while_let_on_iterator)]
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
			to_be_continue.set(&iterator.storage_map_iterator.previous_key);
		}

		// Consume the guard but **do not** unlock the underlying lock.
		guard.forget();

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
			frame_system::Pallet::<T>::dec_consumers(&bidder);
		}

		// decrease account ref of refund recipient
		frame_system::Pallet::<T>::dec_consumers(&collateral_auction.refund_recipient);

		// decrease total collateral and target in auction
		TotalCollateralInAuction::<T>::mutate(collateral_auction.currency_id, |balance| {
			*balance = balance.saturating_sub(collateral_auction.amount)
		});
		TotalTargetInAuction::<T>::mutate(|balance| *balance = balance.saturating_sub(collateral_auction.target));

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

	/// Handles collateral auction new bid. Returns
	/// `Ok(new_auction_end_time)` if bid accepted.
	///
	/// Ensured atomic.
	#[transactional]
	pub fn collateral_auction_bid_handler(
		now: T::BlockNumber,
		id: AuctionId,
		new_bid: (T::AccountId, Balance),
		last_bid: Option<(T::AccountId, Balance)>,
	) -> sp_std::result::Result<T::BlockNumber, DispatchError> {
		let (new_bidder, new_bid_price) = new_bid;
		ensure!(!new_bid_price.is_zero(), Error::<T>::InvalidBidPrice);

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
						TotalCollateralInAuction::<T>::mutate(collateral_auction.currency_id, |balance| {
							*balance = balance.saturating_sub(refund_collateral_amount)
						});
						collateral_auction.amount = new_collateral_amount;
					}
				}

				Self::swap_bidders(&new_bidder, last_bidder);

				Ok(now + Self::get_auction_time_to_close(now, collateral_auction.start_time))
			},
		)
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
					< T::DEX::get_swap_target_amount(
						&[collateral_auction.currency_id, T::GetStableCurrencyId::get()],
						collateral_auction.amount,
						None,
					)
					.unwrap_or_default()
			{
				// try swap collateral in auction with DEX to get stable
				if let Ok(stable_amount) = T::CDPTreasury::swap_exact_collateral_to_stable(
					collateral_auction.currency_id,
					collateral_auction.amount,
					Zero::zero(),
					None,
					None,
					true,
				) {
					// swap successfully, will not deal
					should_deal = false;

					// refund stable currency to the last bidder, it shouldn't fail and affect the
					// process. but even it failed, just the winner did not get the bid price. it
					// can be fixed by treasury council.
					let res = T::CDPTreasury::issue_debit(&bidder, bid_price, false);
					if let Err(e) = res {
						log::warn!(
							target: "auction-manager",
							"issue_debit: failed to issue stable {:?} to {:?}: {:?}. \
							This is unexpected but should be safe",
							bid_price, bidder, e
						);
						debug_assert!(false);
					}

					if collateral_auction.in_reverse_stage(stable_amount) {
						// refund extra stable currency to recipient
						let refund_amount = stable_amount
							.checked_sub(collateral_auction.target)
							.expect("ensured stable_amount > target; qed");
						// it shouldn't fail and affect the process.
						// but even it failed, just the winner did not get the refund amount. it can be
						// fixed by treasury council.
						let res =
							T::CDPTreasury::issue_debit(&collateral_auction.refund_recipient, refund_amount, false);
						if let Err(e) = res {
							log::warn!(
								target: "auction-manager",
								"issue_debit: failed to issue stable {:?} to {:?}: {:?}. \
								This is unexpected but should be safe",
								refund_amount, collateral_auction.refund_recipient, e
							);
							debug_assert!(false);
						}
					}

					Self::deposit_event(Event::DEXTakeCollateralAuction(
						auction_id,
						collateral_auction.currency_id,
						collateral_auction.amount,
						stable_amount,
					));
				}
			}

			if should_deal {
				// transfer collateral to winner from CDP treasury, it shouldn't fail and affect
				// the process. but even it failed, just the winner did not get the amount. it
				// can be fixed by treasury council.
				let res = T::CDPTreasury::withdraw_collateral(
					&bidder,
					collateral_auction.currency_id,
					collateral_auction.amount,
				);
				if let Err(e) = res {
					log::warn!(
						target: "auction-manager",
						"withdraw_collateral: failed to withdraw {:?} {:?} from CDP treasury to {:?}: {:?}. \
						This is unexpected but should be safe",
						collateral_auction.amount, collateral_auction.currency_id, bidder, e
					);
					debug_assert!(false);
				}

				let payment_amount = collateral_auction.payment_amount(bid_price);
				Self::deposit_event(Event::CollateralAuctionDealt(
					auction_id,
					collateral_auction.currency_id,
					collateral_auction.amount,
					bidder,
					payment_amount,
				));
			}
		} else {
			Self::deposit_event(Event::CancelAuction(auction_id));
		}

		// decrement recipient account reference
		frame_system::Pallet::<T>::dec_consumers(&collateral_auction.refund_recipient);

		// update auction records
		TotalCollateralInAuction::<T>::mutate(collateral_auction.currency_id, |balance| {
			*balance = balance.saturating_sub(collateral_auction.amount)
		});
		TotalTargetInAuction::<T>::mutate(|balance| *balance = balance.saturating_sub(collateral_auction.target));
	}

	/// increment `new_bidder` reference and decrement `last_bidder`
	/// reference if any
	fn swap_bidders(new_bidder: &T::AccountId, last_bidder: Option<&T::AccountId>) {
		if frame_system::Pallet::<T>::inc_consumers(new_bidder).is_err() {
			// No providers for the locks. This is impossible under normal circumstances
			// since the funds that are under the lock will themselves be stored in the
			// account and therefore will need a reference.
			log::warn!(
				target: "auction-manager",
				"inc_consumers: failed for {:?}. \
				This is impossible under normal circumstances.",
				new_bidder.clone()
			);
		}

		if let Some(who) = last_bidder {
			frame_system::Pallet::<T>::dec_consumers(who);
		}
	}
}

impl<T: Config> AuctionHandler<T::AccountId, Balance, T::BlockNumber, AuctionId> for Pallet<T> {
	fn on_new_bid(
		now: T::BlockNumber,
		id: AuctionId,
		new_bid: (T::AccountId, Balance),
		last_bid: Option<(T::AccountId, Balance)>,
	) -> OnNewBidResult<T::BlockNumber> {
		let bid_result = Self::collateral_auction_bid_handler(now, id, new_bid, last_bid);

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
		if let Some(collateral_auction) = <CollateralAuctions<T>>::take(id) {
			Self::collateral_auction_end_handler(id, collateral_auction, winner.clone());
		}

		if let Some((bidder, _)) = &winner {
			// decrease account ref of winner
			frame_system::Pallet::<T>::dec_consumers(bidder);
		}
	}
}

impl<T: Config> AuctionManager<T::AccountId> for Pallet<T> {
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
		TotalCollateralInAuction::<T>::try_mutate(currency_id, |total| -> DispatchResult {
			*total = total.checked_add(amount).ok_or(Error::<T>::InvalidAmount)?;
			Ok(())
		})?;

		if !target.is_zero() {
			// no-op if target is zero
			TotalTargetInAuction::<T>::try_mutate(|total| -> DispatchResult {
				*total = total.checked_add(target).ok_or(Error::<T>::InvalidAmount)?;
				Ok(())
			})?;
		}

		let start_time = <frame_system::Pallet<T>>::block_number();

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
		if frame_system::Pallet::<T>::inc_consumers(refund_recipient).is_err() {
			// No providers for the locks. This is impossible under normal circumstances
			// since the funds that are under the lock will themselves be stored in the
			// account and therefore will need a reference.
			log::warn!(
				target: "auction-manager",
				"Attempt to `inc_consumers` for {:?} failed. \
				This is unexpected but should be safe.",
				refund_recipient.clone()
			);
		}

		Self::deposit_event(Event::NewCollateralAuction(auction_id, currency_id, amount, target));
		Ok(())
	}

	fn cancel_auction(id: Self::AuctionId) -> DispatchResult {
		let collateral_auction = <CollateralAuctions<T>>::take(id).ok_or(Error::<T>::AuctionNotExists)?;
		Self::cancel_collateral_auction(id, collateral_auction)?;
		T::Auction::remove_auction(id);
		Ok(())
	}

	fn get_total_collateral_in_auction(id: Self::CurrencyId) -> Self::Balance {
		Self::total_collateral_in_auction(id)
	}

	fn get_total_target_in_auction() -> Self::Balance {
		Self::total_target_in_auction()
	}
}
