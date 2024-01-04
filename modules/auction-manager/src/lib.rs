// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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
#![allow(clippy::unnecessary_unwrap)]

use frame_support::{pallet_prelude::*, transactional};
use frame_system::{
	offchain::{SendTransactionTypes, SubmitTransaction},
	pallet_prelude::*,
};
use module_support::{
	AuctionManager, CDPTreasury, CDPTreasuryExtended, EmergencyShutdown, PriceProvider, Rate, SwapLimit,
};
use orml_traits::{Auction, AuctionHandler, Change, MultiCurrency, OnNewBidResult};
use orml_utilities::OffchainErr;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use primitives::{AuctionId, Balance, CurrencyId};
use scale_info::TypeInfo;
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
#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
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
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The minimum increment size of each bid compared to the previous one
		#[pallet::constant]
		type MinimumIncrementSize: Get<Rate>;

		/// The extended time for the auction to end after each successful bid
		#[pallet::constant]
		type AuctionTimeToClose: Get<BlockNumberFor<Self>>;

		/// When the total duration of the auction exceeds this soft cap, push
		/// the auction to end more faster
		#[pallet::constant]
		type AuctionDurationSoftCap: Get<BlockNumberFor<Self>>;

		/// The stable currency id
		#[pallet::constant]
		type GetStableCurrencyId: Get<CurrencyId>;

		/// Currency to transfer assets
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// Auction to manager the auction process
		type Auction: Auction<Self::AccountId, BlockNumberFor<Self>, AuctionId = AuctionId, Balance = Balance>;

		/// CDP treasury to escrow assets related to auction
		type CDPTreasury: CDPTreasuryExtended<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;

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
		/// Collateral auction created.
		NewCollateralAuction {
			auction_id: AuctionId,
			collateral_type: CurrencyId,
			collateral_amount: Balance,
			target_bid_price: Balance,
		},
		/// Active auction cancelled.
		CancelAuction { auction_id: AuctionId },
		/// Collateral auction dealt.
		CollateralAuctionDealt {
			auction_id: AuctionId,
			collateral_type: CurrencyId,
			collateral_amount: Balance,
			winner: T::AccountId,
			payment_amount: Balance,
		},
		/// Dex take collateral auction.
		DEXTakeCollateralAuction {
			auction_id: AuctionId,
			collateral_type: CurrencyId,
			collateral_amount: Balance,
			supply_collateral_amount: Balance,
			target_stable_amount: Balance,
		},
		/// Collateral auction aborted.
		CollateralAuctionAborted {
			auction_id: AuctionId,
			collateral_type: CurrencyId,
			collateral_amount: Balance,
			target_stable_amount: Balance,
			refund_recipient: T::AccountId,
		},
	}

	/// Mapping from auction id to collateral auction info
	///
	/// CollateralAuctions: map AuctionId => Option<CollateralAuctionItem>
	#[pallet::storage]
	#[pallet::getter(fn collateral_auctions)]
	pub type CollateralAuctions<T: Config> =
		StorageMap<_, Twox64Concat, AuctionId, CollateralAuctionItem<T::AccountId, BlockNumberFor<T>>, OptionQuery>;

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
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// Start offchain worker in order to submit unsigned tx to cancel
		/// active auction after system shutdown.
		fn offchain_worker(now: BlockNumberFor<T>) {
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
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::cancel_collateral_auction())]
		pub fn cancel(origin: OriginFor<T>, id: AuctionId) -> DispatchResult {
			ensure_none(origin)?;
			ensure!(T::EmergencyShutdown::is_shutdown(), Error::<T>::MustAfterShutdown);
			<Self as AuctionManager<T::AccountId>>::cancel_auction(id)?;
			Self::deposit_event(Event::CancelAuction { auction_id: id });
			Ok(())
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;
		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			if let Call::cancel { id: auction_id } = call {
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
		let call = Call::<T>::cancel { id: auction_id };
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
		let mut lock = StorageLock::<'_, Time>::with_deadline(OFFCHAIN_WORKER_LOCK, lock_expiration);
		let mut guard = lock.try_lock().map_err(|_| OffchainErr::OffchainLock)?;

		let mut to_be_continue = StorageValueRef::persistent(OFFCHAIN_WORKER_DATA);

		// get to_be_continue record,
		// if it exsits, iterator map storage start with previous key
		let start_key = to_be_continue.get::<Vec<u8>>().unwrap_or_default();

		// get the max iterationns config
		let max_iterations = StorageValueRef::persistent(OFFCHAIN_WORKER_MAX_ITERATIONS)
			.get::<u32>()
			.unwrap_or(Some(DEFAULT_MAX_ITERATIONS))
			.unwrap_or(DEFAULT_MAX_ITERATIONS);

		log::debug!(
			target: "auction-manager",
			"offchain worker: max iterations is {:?}",
			max_iterations
		);

		// start iterations to cancel collateral auctions
		let mut iterator = match start_key {
			Some(key) => <CollateralAuctions<T>>::iter_from(key),
			None => <CollateralAuctions<T>>::iter(),
		};

		let mut iteration_count = 0;
		let mut finished = true;

		#[allow(clippy::while_let_on_iterator)]
		while let Some((collateral_auction_id, _)) = iterator.next() {
			iteration_count += 1;

			if let (Some(collateral_auction), Some((_, last_bid_price))) = (
				Self::collateral_auctions(collateral_auction_id),
				Self::get_last_bid(collateral_auction_id),
			) {
				// if collateral auction has already been in reverse stage,
				// should skip it.
				if collateral_auction.in_reverse_stage(last_bid_price) {
					if iteration_count == max_iterations {
						finished = false;
						break;
					}
					continue;
				}
			}
			Self::submit_cancel_auction_tx(collateral_auction_id);

			if iteration_count == max_iterations {
				finished = false;
				break;
			}
			guard.extend_lock().map_err(|_| OffchainErr::OffchainLock)?;
		}

		if finished {
			to_be_continue.clear();
		} else {
			to_be_continue.set(&iterator.last_raw_key());
		}

		// Consume the guard but **do not** unlock the underlying lock.
		guard.forget();

		Ok(())
	}

	fn cancel_collateral_auction(
		id: AuctionId,
		collateral_auction: CollateralAuctionItem<T::AccountId, BlockNumberFor<T>>,
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
		let settle_price =
			T::PriceSource::get_relative_price(T::GetStableCurrencyId::get(), collateral_auction.currency_id)
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

	fn get_minimum_increment_size(now: BlockNumberFor<T>, start_block: BlockNumberFor<T>) -> Rate {
		if now >= start_block + T::AuctionDurationSoftCap::get() {
			// double the minimum increment size when reach soft cap
			T::MinimumIncrementSize::get().saturating_mul(Rate::saturating_from_integer(2))
		} else {
			T::MinimumIncrementSize::get()
		}
	}

	fn get_auction_time_to_close(now: BlockNumberFor<T>, start_block: BlockNumberFor<T>) -> BlockNumberFor<T> {
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
		now: BlockNumberFor<T>,
		id: AuctionId,
		new_bid: (T::AccountId, Balance),
		last_bid: Option<(T::AccountId, Balance)>,
	) -> sp_std::result::Result<BlockNumberFor<T>, DispatchError> {
		let (new_bidder, new_bid_price) = new_bid;
		ensure!(!new_bid_price.is_zero(), Error::<T>::InvalidBidPrice);

		<CollateralAuctions<T>>::try_mutate_exists(
			id,
			|collateral_auction| -> sp_std::result::Result<BlockNumberFor<T>, DispatchError> {
				let collateral_auction = collateral_auction.as_mut().ok_or(Error::<T>::AuctionNotExists)?;
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
		collateral_auction: CollateralAuctionItem<T::AccountId, BlockNumberFor<T>>,
		last_bid: Option<(T::AccountId, Balance)>,
	) {
		let (last_bidder, bid_price) = if let Some((bidder, bid_price)) = last_bid.clone() {
			(Some(bidder), bid_price)
		} else {
			(None, Zero::zero())
		};

		let swap_limit = if collateral_auction.always_forward() {
			SwapLimit::ExactSupply(collateral_auction.amount, bid_price)
		} else {
			SwapLimit::ExactTarget(collateral_auction.amount, collateral_auction.target)
		};

		// if DEX give a price no less than the last_bidder for swap target
		if let Ok((actual_supply_amount, actual_target_amount)) =
			T::CDPTreasury::swap_collateral_to_stable(collateral_auction.currency_id, swap_limit, true)
		{
			Self::try_refund_collateral(
				collateral_auction.currency_id,
				&collateral_auction.refund_recipient,
				collateral_auction.amount.saturating_sub(actual_supply_amount),
			);
			Self::try_refund_bid(&collateral_auction, last_bid);

			// Note: for StableAsset, the swap of cdp treasury is always on `ExactSupply`
			// regardless of this swap_limit params. There will be excess stablecoins that
			// need to be returned to the refund_recipient from cdp treasury account.
			if let SwapLimit::ExactTarget(_, target_limit) = swap_limit {
				if actual_target_amount > target_limit {
					let _ = T::CDPTreasury::withdraw_surplus(
						&collateral_auction.refund_recipient,
						actual_target_amount.saturating_sub(target_limit),
					);
				}
			}

			Self::deposit_event(Event::DEXTakeCollateralAuction {
				auction_id,
				collateral_type: collateral_auction.currency_id,
				collateral_amount: collateral_auction.amount,
				supply_collateral_amount: actual_supply_amount,
				target_stable_amount: actual_target_amount,
			});
		} else if last_bidder.is_some() && bid_price >= collateral_auction.target {
			// if these's bid which is gte target, auction should dealt by the last bidder.
			let winner = last_bidder.expect("ensured last bidder not empty; qed");

			Self::try_refund_collateral(collateral_auction.currency_id, &winner, collateral_auction.amount);
			let payment_amount = collateral_auction.payment_amount(bid_price);

			Self::deposit_event(Event::CollateralAuctionDealt {
				auction_id,
				collateral_type: collateral_auction.currency_id,
				collateral_amount: collateral_auction.amount,
				winner,
				payment_amount,
			});
		} else {
			// abort this collateral auction, these collateral can be reprocessed by cdp treausry.
			Self::try_refund_bid(&collateral_auction, last_bid);

			Self::deposit_event(Event::CollateralAuctionAborted {
				auction_id,
				collateral_type: collateral_auction.currency_id,
				collateral_amount: collateral_auction.amount,
				target_stable_amount: collateral_auction.target,
				refund_recipient: collateral_auction.refund_recipient.clone(),
			});
		}

		// decrement recipient account reference
		frame_system::Pallet::<T>::dec_consumers(&collateral_auction.refund_recipient);

		// update auction records
		TotalCollateralInAuction::<T>::mutate(collateral_auction.currency_id, |balance| {
			*balance = balance.saturating_sub(collateral_auction.amount)
		});
		TotalTargetInAuction::<T>::mutate(|balance| *balance = balance.saturating_sub(collateral_auction.target));
	}

	// Refund stable to the last_bidder.
	fn try_refund_bid(
		collateral_auction: &CollateralAuctionItem<T::AccountId, BlockNumberFor<T>>,
		last_bid: Option<(T::AccountId, Balance)>,
	) {
		if let Some((bidder, bid_price)) = last_bid {
			// If failed, just the bid did not get the stable. It can be fixed by treasury council.
			let res = T::CDPTreasury::issue_debit(&bidder, collateral_auction.payment_amount(bid_price), false);
			if let Err(e) = res {
				log::warn!(
					target: "auction-manager",
					"issue_debit: failed to issue stable {:?} to {:?}: {:?}. \
					This is unexpected but should be safe",
					collateral_auction.payment_amount(bid_price), bidder, e
				);
				debug_assert!(false);
			}
		}
	}

	// Refund collateral to the refund_recipient.
	fn try_refund_collateral(collateral_type: CurrencyId, refund_recipient: &T::AccountId, refund_collateral: Balance) {
		if !refund_collateral.is_zero() {
			// If failed, just the refund_recipient did not get the refund collateral. It can be fixed by
			// treasury council.
			let res = T::CDPTreasury::withdraw_collateral(refund_recipient, collateral_type, refund_collateral);
			if let Err(e) = res {
				log::warn!(
					target: "auction-manager",
					"withdraw_collateral: failed to withdraw {:?} {:?} from CDP treasury to {:?}: {:?}. \
					This is unexpected but should be safe",
					refund_collateral, collateral_type, refund_recipient, e
				);
				debug_assert!(false);
			}
		}
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

impl<T: Config> AuctionHandler<T::AccountId, Balance, BlockNumberFor<T>, AuctionId> for Pallet<T> {
	fn on_new_bid(
		now: BlockNumberFor<T>,
		id: AuctionId,
		new_bid: (T::AccountId, Balance),
		last_bid: Option<(T::AccountId, Balance)>,
	) -> OnNewBidResult<BlockNumberFor<T>> {
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
		// use start_time + AuctionDurationSoftCap as the initial end-time of collateral auction.
		let end_time = start_time.saturating_add(T::AuctionDurationSoftCap::get());
		let auction_id = T::Auction::new_auction(start_time, Some(end_time))?;

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

		Self::deposit_event(Event::NewCollateralAuction {
			auction_id,
			collateral_type: currency_id,
			collateral_amount: amount,
			target_bid_price: target,
		});
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
