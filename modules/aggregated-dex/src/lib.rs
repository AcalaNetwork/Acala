// This file is part of Acala.

// Copyright (C) 2022 Acala Foundation.
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

//! # Aggregated DEX Module

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::type_complexity)]

use codec::Decode;
use frame_support::{pallet_prelude::*, transactional, PalletId};
use frame_system::{
	offchain::{SendTransactionTypes, SubmitTransaction},
	pallet_prelude::*,
};
use nutsfinance_stable_asset::traits::StableAsset as StableAssetT;
use orml_utilities::OffchainErr;
use primitives::{Balance, CurrencyId, TradingPair};
use sp_runtime::offchain::storage_lock::StorageLockGuard;
use sp_runtime::{
	offchain::{
		storage::StorageValueRef,
		storage_lock::{StorageLock, Time},
		Duration,
	},
	traits::{AccountIdConversion, Convert, Zero},
};
use sp_std::{collections::btree_map::BTreeMap, marker::PhantomData, prelude::*, vec::Vec};
use support::{AggregatedSwapPath, DEXManager, RebasedStableAssetError, Swap, SwapLimit};

mod mock;
mod tests;
pub mod weights;

pub use module::*;
use module_dex::TradingPairStatuses;
pub use weights::WeightInfo;

pub const OFFCHAIN_WORKER_DATA: &[u8] = b"acala/dex-bot/data/";
pub const OFFCHAIN_WORKER_LOCK: &[u8] = b"acala/dex-bot/lock/";
pub const OFFCHAIN_WORKER_MAX_ITERATIONS: &[u8] = b"acala/dex-bot/max-iterations/";
pub const LOCK_DURATION: u64 = 100;
pub const DEFAULT_MAX_ITERATIONS: u32 = 100;

pub type SwapPath = AggregatedSwapPath<CurrencyId>;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config + SendTransactionTypes<Call<Self>> + module_dex::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// DEX
		type DEX: DEXManager<Self::AccountId, Balance, CurrencyId>;

		/// Taiga
		type StableAsset: StableAssetT<
			AssetId = CurrencyId,
			AtLeast64BitUnsigned = Balance,
			Balance = Balance,
			AccountId = Self::AccountId,
			BlockNumber = Self::BlockNumber,
		>;

		/// Origin represented Governance
		type GovernanceOrigin: EnsureOrigin<<Self as frame_system::Config>::Origin>;

		/// The alternative swap path joint list for DEX swap
		#[pallet::constant]
		type DexSwapJointList: Get<Vec<Vec<CurrencyId>>>;

		/// The limit for length of swap path
		#[pallet::constant]
		type SwapPathLimit: Get<u32>;

		/// Treasury account participate in Rebalance swap.
		#[pallet::constant]
		type TreasuryPallet: Get<PalletId>;

		#[pallet::constant]
		type UnsignedPriority: Get<TransactionPriority>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Cannot swap.
		CannotSwap,
		/// The stable asset pool id of Taiga is invalid.
		InvalidPoolId,
		/// The asset index of stable asset pool is invalid.
		InvalidTokenIndex,
		/// The SwapPath is invalid.
		InvalidSwapPath,
		/// Rebalance swap info is invalid.
		RebalanceSwapInfoInvalid,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Rebalance trading path and balance.
		RebalanceTrading {
			currency_id: CurrencyId,
			supply_amount: Balance,
			target_amount: Balance,
			swap_path: Vec<SwapPath>,
		},
		/// Add rebalance info.
		SetupRebalanceSwapInfo {
			currency_id: CurrencyId,
			supply_amount: Balance,
			threshold: Balance,
		},
	}

	/// The specific swap paths for  AggregatedSwap do aggregated_swap to swap TokenA to TokenB
	///
	/// AggregatedSwapPaths: Map: (token_a: CurrencyId, token_b: CurrencyId) => paths: Vec<SwapPath>
	#[pallet::storage]
	#[pallet::getter(fn aggregated_swap_paths)]
	pub type AggregatedSwapPaths<T: Config> =
		StorageMap<_, Twox64Concat, (CurrencyId, CurrencyId), BoundedVec<SwapPath, T::SwapPathLimit>, OptionQuery>;

	/// The specific rebalance swap paths doing aggregated_swap from TokenA to TokenA
	///
	/// AggregatedSwapPaths: Map: CurrencyId => paths: Vec<SwapPath>
	#[pallet::storage]
	#[pallet::getter(fn rebalance_swap_paths)]
	pub type RebalanceSwapPaths<T: Config> =
		StorageMap<_, Twox64Concat, CurrencyId, BoundedVec<SwapPath, T::SwapPathLimit>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn rebalance_supply_threshold)]
	pub type RebalanceSupplyThreshold<T: Config> =
		StorageMap<_, Twox64Concat, CurrencyId, (Balance, Balance), OptionQuery>;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn offchain_worker(now: T::BlockNumber) {
			if let Err(e) = Self::_offchain_worker(now) {
				log::info!(
					target: "dex-bot",
					"offchain worker: cannot run at {:?}: {:?}",
					now, e,
				);
			} else {
				log::debug!(
					target: "dex-bot",
					"offchain worker: start at block: {:?} already done!",
					now,
				);
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Swap with aggregated DEX at exact supply amount.
		///
		/// - `paths`: aggregated swap path.
		/// - `supply_amount`: exact supply amount.
		/// - `min_target_amount`: acceptable minimum target amount.
		#[pallet::weight(<T as Config>::WeightInfo::swap_with_exact_supply(
			paths.iter().fold(0, |u, swap_path| match swap_path {
				SwapPath::Dex(v) => u + (v.len() as u32),
				SwapPath::Taiga(_, _, _) => u + 1
			})
		))]
		#[transactional]
		pub fn swap_with_exact_supply(
			origin: OriginFor<T>,
			paths: Vec<SwapPath>,
			#[pallet::compact] supply_amount: Balance,
			#[pallet::compact] min_target_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let paths: BoundedVec<SwapPath, T::SwapPathLimit> =
				paths.try_into().map_err(|_| Error::<T>::InvalidSwapPath)?;
			let _ = Self::do_aggregated_swap(&who, &paths, SwapLimit::ExactSupply(supply_amount, min_target_amount))?;
			Ok(())
		}

		#[pallet::weight(<T as Config>::WeightInfo::swap_with_exact_supply(
			paths.iter().fold(0, |u, swap_path| match swap_path {
				SwapPath::Dex(v) => u + (v.len() as u32),
				SwapPath::Taiga(_, _, _) => u + 1
			})
		))]
		#[transactional]
		pub fn swap_with_exact_target(
			origin: OriginFor<T>,
			paths: Vec<SwapPath>,
			#[pallet::compact] target_amount: Balance,
			#[pallet::compact] max_supply_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let paths: BoundedVec<SwapPath, T::SwapPathLimit> =
				paths.try_into().map_err(|_| Error::<T>::InvalidSwapPath)?;
			let _ = Self::do_aggregated_swap(&who, &paths, SwapLimit::ExactTarget(max_supply_amount, target_amount))?;
			Ok(())
		}

		/// Update the aggregated swap paths for AggregatedSwap to swap TokenA to TokenB.
		///
		/// Requires `GovernanceOrigin`
		///
		/// Parameters:
		/// - `updates`:  Vec<((TokenA, TokenB), Option<Vec<SwapPath>>)>
		#[pallet::weight(<T as Config>::WeightInfo::update_aggregated_swap_paths(updates.len() as u32))]
		#[transactional]
		pub fn update_aggregated_swap_paths(
			origin: OriginFor<T>,
			updates: Vec<((CurrencyId, CurrencyId), Option<Vec<SwapPath>>)>,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			for (key, maybe_paths) in updates {
				if let Some(paths) = maybe_paths {
					let paths: BoundedVec<SwapPath, T::SwapPathLimit> =
						paths.try_into().map_err(|_| Error::<T>::InvalidSwapPath)?;
					let (supply_currency_id, target_currency_id) = Self::check_swap_paths(&paths)?;
					ensure!(
						key == (supply_currency_id, target_currency_id),
						Error::<T>::InvalidSwapPath
					);
					AggregatedSwapPaths::<T>::insert(key, paths);
				} else {
					AggregatedSwapPaths::<T>::remove(key);
				}
			}

			Ok(())
		}

		/// Update the rebalance swap paths for AggregatedSwap to swap TokenA to TokenA.
		///
		/// Requires `GovernanceOrigin`
		///
		/// Parameters:
		/// - `updates`:  Vec<(CurrencyId, Option<Vec<SwapPath>>)>
		#[pallet::weight(<T as Config>::WeightInfo::update_aggregated_swap_paths(updates.len() as u32))]
		#[transactional]
		pub fn update_rebalance_swap_paths(
			origin: OriginFor<T>,
			updates: Vec<(CurrencyId, Option<Vec<SwapPath>>)>,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			for (key, maybe_paths) in updates {
				if let Some(paths) = maybe_paths {
					let paths: BoundedVec<SwapPath, T::SwapPathLimit> =
						paths.try_into().map_err(|_| Error::<T>::InvalidSwapPath)?;
					let (supply_currency_id, target_currency_id) = Self::check_swap_paths(&paths)?;
					ensure!(
						key == supply_currency_id && supply_currency_id == target_currency_id,
						Error::<T>::InvalidSwapPath
					);
					RebalanceSwapPaths::<T>::insert(key, paths);
				} else {
					RebalanceSwapPaths::<T>::remove(key);
				}
			}

			Ok(())
		}

		/// Update the rebalance swap information for specify token.
		///
		/// Parameters:
		/// - `currency_id`: the token used for rebalance swap
		/// - `supply_amount`: the supply amount of `currency_id` used for rebalance swap
		/// - `threshold`: the target amount of `currency_id` used for rebalance swap
		#[pallet::weight(<T as Config>::WeightInfo::set_rebalance_swap_info())]
		#[transactional]
		pub fn set_rebalance_swap_info(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			#[pallet::compact] supply_amount: Balance,
			#[pallet::compact] threshold: Balance,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;
			Self::do_set_rebalance_swap_info(currency_id, supply_amount, threshold)
		}

		/// Force execution rebalance swap by offchain worker.
		///
		/// Parameters:
		/// - `currency_id`: the token used for rebalance swap
		/// - `swap_path`: the aggregated swap path used for rebalance swap
		#[pallet::weight(<T as Config>::WeightInfo::force_rebalance_swap())]
		#[transactional]
		pub fn force_rebalance_swap(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			swap_path: Vec<SwapPath>,
		) -> DispatchResult {
			ensure_none(origin)?;
			Self::do_rebalance_swap(currency_id, swap_path)
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;
		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			if let Call::force_rebalance_swap {
				currency_id,
				swap_path: _,
			} = call
			{
				ValidTransaction::with_tag_prefix("DexBotOffchainWorker")
					.priority(T::UnsignedPriority::get())
					.and_provides((<frame_system::Pallet<T>>::block_number(), currency_id))
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
	fn treasury_account() -> T::AccountId {
		T::TreasuryPallet::get().into_account_truncating()
	}

	fn check_swap_paths(paths: &[SwapPath]) -> sp_std::result::Result<(CurrencyId, CurrencyId), DispatchError> {
		ensure!(!paths.is_empty(), Error::<T>::InvalidSwapPath);
		let mut supply_currency_id: Option<CurrencyId> = None;
		let mut previous_output_currency_id: Option<CurrencyId> = None;

		for path in paths {
			match path {
				SwapPath::Dex(dex_path) => {
					let input_currency_id = dex_path.first().ok_or(Error::<T>::InvalidSwapPath)?;
					let output_currency_id = dex_path.last().ok_or(Error::<T>::InvalidSwapPath)?;
					ensure!(input_currency_id != output_currency_id, Error::<T>::InvalidSwapPath);

					// If there has been a swap before,
					// the currency id of this swap must be the output currency id of the previous swap.
					if let Some(currency_id) = previous_output_currency_id {
						ensure!(currency_id == *input_currency_id, Error::<T>::InvalidSwapPath);
					}

					if supply_currency_id.is_none() {
						supply_currency_id = Some(*input_currency_id);
					}
					previous_output_currency_id = Some(*output_currency_id);
				}
				SwapPath::Taiga(pool_id, supply_asset_index, target_asset_index) => {
					ensure!(supply_asset_index != target_asset_index, Error::<T>::InvalidSwapPath);
					let pool_info = T::StableAsset::pool(*pool_id).ok_or(Error::<T>::InvalidPoolId)?;
					let input_currency_id = pool_info
						.assets
						.get(*supply_asset_index as usize)
						.ok_or(Error::<T>::InvalidTokenIndex)?;
					let output_currency_id = pool_info
						.assets
						.get(*target_asset_index as usize)
						.ok_or(Error::<T>::InvalidTokenIndex)?;

					// If there has been a swap before,
					// the currency id of this swap must be the output currency id of the previous swap.
					if let Some(currency_id) = previous_output_currency_id {
						ensure!(currency_id == *input_currency_id, Error::<T>::InvalidSwapPath);
					}

					if supply_currency_id.is_none() {
						supply_currency_id = Some(*input_currency_id);
					}
					previous_output_currency_id = Some(*output_currency_id);
				}
			}
		}

		ensure!(
			supply_currency_id.is_some() && previous_output_currency_id.is_some(),
			Error::<T>::InvalidSwapPath
		);

		Ok((
			supply_currency_id.expect("already checked; qed"),
			previous_output_currency_id.expect("already checked; qed"),
		))
	}

	fn get_aggregated_swap_amount(paths: &[SwapPath], swap_limit: SwapLimit<Balance>) -> Option<(Balance, Balance)> {
		Self::check_swap_paths(paths).ok()?;

		match swap_limit {
			SwapLimit::ExactSupply(exact_supply_amount, min_target_amount) => {
				let mut output_amount: Balance = exact_supply_amount;

				for path in paths {
					match path {
						SwapPath::Dex(dex_path) => {
							// use the output of the previous swap as input.
							let (_, actual_target) =
								T::DEX::get_swap_amount(dex_path, SwapLimit::ExactSupply(output_amount, Zero::zero()))?;

							output_amount = actual_target;
						}
						SwapPath::Taiga(pool_id, supply_asset_index, target_asset_index) => {
							// use the output of the previous swap as input.
							let (_, actual_output_amount) = T::StableAsset::get_swap_output_amount(
								*pool_id,
								*supply_asset_index,
								*target_asset_index,
								output_amount,
							)
							.map(|result| (result.dx, result.dy))?;

							output_amount = actual_output_amount;
						}
					}
				}

				if output_amount >= min_target_amount {
					return Some((exact_supply_amount, output_amount));
				}
			}
			SwapLimit::ExactTarget(max_supply_amount, exact_target_amount) => {
				let mut input_amount: Balance = exact_target_amount;

				for path in paths.iter().rev() {
					match path {
						SwapPath::Dex(dex_path) => {
							// calculate the supply amount
							let (supply_amount, _) = T::DEX::get_swap_amount(
								dex_path,
								SwapLimit::ExactTarget(Balance::max_value(), input_amount),
							)?;

							input_amount = supply_amount;
						}
						SwapPath::Taiga(pool_id, supply_asset_index, target_asset_index) => {
							// calculate the input amount
							let (actual_input_amount, _) = T::StableAsset::get_swap_input_amount(
								*pool_id,
								*supply_asset_index,
								*target_asset_index,
								input_amount,
							)
							.map(|result| (result.dx, result.dy))?;

							input_amount = actual_input_amount;
						}
					}
				}

				if input_amount <= max_supply_amount {
					// actually swap by `ExactSupply` limit
					return Self::get_aggregated_swap_amount(
						paths,
						SwapLimit::ExactSupply(input_amount, exact_target_amount),
					);
				}
			}
		}

		None
	}

	/// Aggregated swap by DEX and Taiga.
	#[transactional]
	fn do_aggregated_swap(
		who: &T::AccountId,
		paths: &[SwapPath],
		swap_limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		Self::check_swap_paths(paths)?;

		match swap_limit {
			// do swap directly one by one according to the SwapPaths
			SwapLimit::ExactSupply(exact_supply_amount, min_target_amount) => {
				let mut output_amount: Balance = exact_supply_amount;

				for path in paths {
					match path {
						SwapPath::Dex(dex_path) => {
							// use the output of the previous swap as input.
							let (_, actual_target) = T::DEX::swap_with_specific_path(
								who,
								dex_path,
								SwapLimit::ExactSupply(output_amount, Zero::zero()),
							)?;

							output_amount = actual_target;
						}
						SwapPath::Taiga(pool_id, supply_asset_index, target_asset_index) => {
							let pool_info = T::StableAsset::pool(*pool_id).ok_or(Error::<T>::InvalidPoolId)?;
							let asset_length = pool_info.assets.len() as u32;

							// use the output of the previous swap as input.
							let (_, actual_target) = T::StableAsset::swap(
								who,
								*pool_id,
								*supply_asset_index,
								*target_asset_index,
								output_amount,
								Zero::zero(),
								asset_length,
							)?;

							output_amount = actual_target;
						}
					}
				}

				// the result must meet the swap_limit.
				ensure!(output_amount >= min_target_amount, Error::<T>::CannotSwap);

				Ok((exact_supply_amount, output_amount))
			}
			// Calculate the supply amount first, then execute swap with ExactSupply
			SwapLimit::ExactTarget(_max_supply_amount, exact_target_amount) => {
				let (supply_amount, _) =
					Self::get_aggregated_swap_amount(paths, swap_limit).ok_or(Error::<T>::CannotSwap)?;

				// actually swap by `ExactSupply` limit
				Self::do_aggregated_swap(who, paths, SwapLimit::ExactSupply(supply_amount, exact_target_amount))
			}
		}
	}

	fn do_set_rebalance_swap_info(
		currency_id: CurrencyId,
		supply_amount: Balance,
		threshold: Balance,
	) -> DispatchResult {
		ensure!(threshold > supply_amount, Error::<T>::RebalanceSwapInfoInvalid);
		RebalanceSupplyThreshold::<T>::try_mutate(currency_id, |maybe_supply_threshold| -> DispatchResult {
			*maybe_supply_threshold = Some((supply_amount, threshold));
			Ok(())
		})?;
		Self::deposit_event(Event::SetupRebalanceSwapInfo {
			currency_id,
			supply_amount,
			threshold,
		});
		Ok(())
	}

	fn submit_rebalance_swap_tx(currency_id: CurrencyId, swap_path: Vec<SwapPath>) {
		let call = Call::<T>::force_rebalance_swap { currency_id, swap_path };
		if let Err(err) = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()) {
			log::info!(
				target: "dex-bot",
				"offchain worker: submit unsigned swap from currency:{:?}, failed: {:?}",
				currency_id, err,
			);
		}
	}

	pub fn calculate_rebalance_paths(
		// mut _finished: bool,
		// mut iteration_count: u32,
		max_iterations: u32,
		// mut _last_currency_id: Option<CurrencyId>,
		start_key: Option<Vec<u8>>,
		mut guard: Option<&mut StorageLockGuard<Time>>,
		f: impl Fn(CurrencyId, Vec<SwapPath>),
	) -> Result<(bool, Option<CurrencyId>), OffchainErr> {
		let mut finished = true;
		let mut iteration_count = 0;
		let mut last_currency_id: Option<CurrencyId> = None;

		let mut trading_pair_values_map: BTreeMap<CurrencyId, Vec<CurrencyId>> = BTreeMap::new();
		let mut first_currency: Option<CurrencyId> = None;
		TradingPairStatuses::<T>::iter()
			.filter(|(_, status)| status.enabled())
			.for_each(|(pair, _)| {
				trading_pair_values_map
					.entry(pair.first())
					.or_insert_with(Vec::<CurrencyId>::new)
					.push(pair.second());
				if first_currency.is_none() {
					first_currency = Some(pair.first());
				}
			});
		let mut iterator = trading_pair_values_map.iter();

		// processing pure dex rebalance swap
		#[allow(clippy::while_let_on_iterator)]
		'outer: while let Some((currency_id, trading_tokens)) = iterator.next() {
			// BTreeMap don't have `iter_from(key)`, use compare to ignore previous processed token.
			match start_key.clone() {
				Some(key) => {
					let starter_key = CurrencyId::decode(&mut &*key).map_err(|_| OffchainErr::OffchainLock)?;
					if *currency_id < starter_key {
						continue;
					}
				}
				None => {}
			};

			let len = trading_tokens.len();
			if len < 2 {
				continue;
			}
			// update last processing CurrencyId
			last_currency_id = Some(*currency_id);
			for i in 0..(len - 1) {
				for j in (i + 1)..len {
					iteration_count += 1;

					if let Some(pair) = TradingPair::from_currency_ids(trading_tokens[i], trading_tokens[j]) {
						if TradingPairStatuses::<T>::contains_key(&pair) {
							let pair_status = TradingPairStatuses::<T>::get(&pair);
							if pair_status.enabled() {
								let first_path: Vec<CurrencyId> = vec![*currency_id, pair.first(), pair.second()];
								let second_path: Vec<CurrencyId> = vec![pair.second(), *currency_id];
								let swap_path = vec![SwapPath::Dex(first_path), SwapPath::Dex(second_path)];
								// TODO: check swap_amount before submit unsigned tx?
								f(*currency_id, swap_path);
							}
						}
					}

					// inner iterator consider as iterations too.
					if iteration_count == max_iterations {
						finished = false;
						break 'outer;
					}

					// extend offchain worker lock
					if let Some(ref mut guard) = guard {
						guard.extend_lock().map_err(|_| OffchainErr::OffchainLock)?;
					}
				}
			}
		}

		// processing aggregated dex rebalance swap
		if iteration_count < max_iterations {
			for (currency_id, swap_path) in RebalanceSwapPaths::<T>::iter() {
				iteration_count += 1;

				f(currency_id, swap_path.into_inner());

				// inner iterator consider as iterations too.
				if iteration_count == max_iterations {
					finished = false;
					break;
				}

				// extend offchain worker lock
				if let Some(ref mut guard) = guard {
					guard.extend_lock().map_err(|_| OffchainErr::OffchainLock)?;
				}
			}
		}

		Ok((finished, last_currency_id))
	}

	fn _offchain_worker(_now: T::BlockNumber) -> Result<(), OffchainErr> {
		// acquire offchain worker lock
		let lock_expiration = Duration::from_millis(LOCK_DURATION);
		let mut lock = StorageLock::<'_, Time>::with_deadline(OFFCHAIN_WORKER_LOCK, lock_expiration);
		let mut guard = lock.try_lock().map_err(|_| OffchainErr::OffchainLock)?;
		// get the max iterations config
		let max_iterations = StorageValueRef::persistent(OFFCHAIN_WORKER_MAX_ITERATIONS)
			.get::<u32>()
			.unwrap_or(Some(DEFAULT_MAX_ITERATIONS))
			.unwrap_or(DEFAULT_MAX_ITERATIONS);
		let mut to_be_continue = StorageValueRef::persistent(OFFCHAIN_WORKER_DATA);
		let start_key = to_be_continue.get::<Vec<u8>>().unwrap_or_default();

		let (finished, last_currency_id) =
			Self::calculate_rebalance_paths(max_iterations, start_key, Some(&mut guard), |currency_id, swap_path| {
				Self::submit_rebalance_swap_tx(currency_id, swap_path)
			})?;

		// if iteration for map storage finished, clear to be continue record
		// otherwise, update to be continue record
		if finished {
			to_be_continue.clear();
		} else {
			match last_currency_id {
				Some(last_currency_id) => to_be_continue.set(&last_currency_id.encode()),
				None => to_be_continue.clear(),
			}
		}

		// Consume the guard but **do not** unlock the underlying lock.
		guard.forget();

		Ok(())
	}

	/// Rebalance swap on path, the final output should be large than input.
	fn do_rebalance_swap(currency_id: CurrencyId, swap_path: Vec<SwapPath>) -> DispatchResult {
		if let Some((supply_amount, minimum_amount)) = RebalanceSupplyThreshold::<T>::get(&currency_id) {
			let supply = SwapLimit::ExactSupply(supply_amount, 0);
			if let Some((_, target_amount)) = Pallet::<T>::get_aggregated_swap_amount(&swap_path, supply) {
				if target_amount > minimum_amount {
					Pallet::<T>::do_aggregated_swap(&Self::treasury_account(), &swap_path, supply)?;

					Self::deposit_event(Event::RebalanceTrading {
						currency_id,
						supply_amount,
						target_amount,
						swap_path,
					});
				}
			}
		}
		Ok(())
	}
}

/// Swap by Acala DEX which has specific joints.
pub struct DexSwap<T>(PhantomData<T>);
impl<T: Config> Swap<T::AccountId, Balance, CurrencyId> for DexSwap<T> {
	fn get_swap_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> Option<(Balance, Balance)> {
		T::DEX::get_best_price_swap_path(
			supply_currency_id,
			target_currency_id,
			limit,
			T::DexSwapJointList::get(),
		)
		.map(|(_, supply_amount, target_amount)| (supply_amount, target_amount))
	}

	fn swap(
		who: &T::AccountId,
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		let path = T::DEX::get_best_price_swap_path(
			supply_currency_id,
			target_currency_id,
			limit,
			T::DexSwapJointList::get(),
		)
		.ok_or(Error::<T>::CannotSwap)?
		.0;

		T::DEX::swap_with_specific_path(who, &path, limit)
	}

	fn swap_by_path(
		who: &T::AccountId,
		swap_path: &[CurrencyId],
		limit: SwapLimit<Balance>,
	) -> Result<(Balance, Balance), DispatchError> {
		T::DEX::swap_with_specific_path(who, swap_path, limit)
	}

	// DexSwap do not support swap by aggregated path.
	fn swap_by_aggregated_path(
		_who: &T::AccountId,
		_swap_path: &[SwapPath],
		_limit: SwapLimit<Balance>,
	) -> Result<(Balance, Balance), DispatchError> {
		Err(Error::<T>::CannotSwap.into())
	}
}

/// Swap by Taiga pool.
pub struct TaigaSwap<T>(PhantomData<T>);
impl<T: Config> Swap<T::AccountId, Balance, CurrencyId> for TaigaSwap<T> {
	// !!! Note: if ths limit is `ExactTarget` and the `max_supply_amount` will cause overflow in
	// StableAsset, will return `None`. Because the `get_best_route` of StableAsset treats it as the
	// actual input amount. However, it will fail so will not cause loss. Maybe need to modiry
	// StableAsset impl to avoid this risk.
	fn get_swap_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> Option<(Balance, Balance)> {
		match limit {
			SwapLimit::ExactSupply(supply_amount, min_target_amount) => {
				let (pool_id, input_index, output_index, _) =
					T::StableAsset::get_best_route(supply_currency_id, target_currency_id, supply_amount)?;

				if let Some((input_amount, output_amount)) =
					T::StableAsset::get_swap_output_amount(pool_id, input_index, output_index, supply_amount)
						.map(|result| (result.dx, result.dy))
				{
					if output_amount >= min_target_amount {
						return Some((input_amount, output_amount));
					}
				}
			}
			SwapLimit::ExactTarget(max_supply_amount, target_amount) => {
				let (pool_id, input_index, output_index, _) =
					T::StableAsset::get_best_route(supply_currency_id, target_currency_id, max_supply_amount)?;

				if let Some((input_amount, _)) =
					T::StableAsset::get_swap_input_amount(pool_id, input_index, output_index, target_amount)
						.map(|result| (result.dx, result.dy))
				{
					if !input_amount.is_zero() && input_amount <= max_supply_amount {
						// actually swap by `ExactSupply` limit
						return Self::get_swap_amount(
							supply_currency_id,
							target_currency_id,
							SwapLimit::ExactSupply(input_amount, target_amount),
						);
					}
				}
			}
		};

		None
	}

	#[transactional]
	fn swap(
		who: &T::AccountId,
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		let (supply_amount, min_target_amount) = match limit {
			SwapLimit::ExactSupply(supply_amount, min_target_amount) => (supply_amount, min_target_amount),
			SwapLimit::ExactTarget(_, target_amount) => {
				let (supply_amount, _) = Self::get_swap_amount(supply_currency_id, target_currency_id, limit)
					.ok_or(Error::<T>::CannotSwap)?;
				(supply_amount, target_amount)
			}
		};

		let (pool_id, input_index, output_index, _) =
			T::StableAsset::get_best_route(supply_currency_id, target_currency_id, supply_amount)
				.ok_or(Error::<T>::CannotSwap)?;
		let pool_info = T::StableAsset::pool(pool_id).ok_or(Error::<T>::InvalidPoolId)?;
		let asset_length = pool_info.assets.len() as u32;

		let (actual_supply, actual_target) = T::StableAsset::swap(
			who,
			pool_id,
			input_index,
			output_index,
			supply_amount,
			min_target_amount,
			asset_length,
		)?;

		ensure!(actual_target >= min_target_amount, Error::<T>::CannotSwap);
		Ok((actual_supply, actual_target))
	}

	// TaigaSwap do not support direct dex swap.
	fn swap_by_path(
		_who: &T::AccountId,
		_swap_path: &[CurrencyId],
		_limit: SwapLimit<Balance>,
	) -> Result<(Balance, Balance), DispatchError> {
		Err(Error::<T>::CannotSwap.into())
	}

	// TaigaSwap do not support swap by aggregated path.
	fn swap_by_aggregated_path(
		_who: &T::AccountId,
		_swap_path: &[SwapPath],
		_limit: SwapLimit<Balance>,
	) -> Result<(Balance, Balance), DispatchError> {
		Err(Error::<T>::CannotSwap.into())
	}
}

/// Choose DEX or Taiga to fully execute the swap by which price is better.
pub struct EitherDexOrTaigaSwap<T>(PhantomData<T>);

struct DexOrTaigaSwapParams {
	dex_result: Option<(Balance, Balance)>,
	taiga_result: Option<(Balance, Balance)>,
	swap_amount: Option<(Balance, Balance)>,
}

impl<T: Config> EitherDexOrTaigaSwap<T> {
	fn get_swap_params(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> DexOrTaigaSwapParams {
		let dex_result = DexSwap::<T>::get_swap_amount(supply_currency_id, target_currency_id, limit);
		let taiga_result = TaigaSwap::<T>::get_swap_amount(supply_currency_id, target_currency_id, limit);
		let swap_amount =
			if let (Some((dex_supply, dex_target)), Some((taiga_supply, taiga_target))) = (dex_result, taiga_result) {
				match limit {
					SwapLimit::ExactSupply(_, _) => {
						if taiga_target > dex_target {
							taiga_result
						} else {
							dex_result
						}
					}
					SwapLimit::ExactTarget(_, _) => {
						if taiga_supply < dex_supply {
							taiga_result
						} else {
							dex_result
						}
					}
				}
			} else {
				dex_result.or(taiga_result)
			};

		DexOrTaigaSwapParams {
			dex_result,
			taiga_result,
			swap_amount,
		}
	}
}

impl<T: Config> Swap<T::AccountId, Balance, CurrencyId> for EitherDexOrTaigaSwap<T> {
	fn get_swap_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> Option<(Balance, Balance)> {
		Self::get_swap_params(supply_currency_id, target_currency_id, limit).swap_amount
	}

	fn swap(
		who: &T::AccountId,
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		let DexOrTaigaSwapParams {
			dex_result,
			taiga_result,
			swap_amount,
		} = Self::get_swap_params(supply_currency_id, target_currency_id, limit);

		if swap_amount.is_some() {
			if dex_result == swap_amount {
				return DexSwap::<T>::swap(who, supply_currency_id, target_currency_id, limit);
			} else if taiga_result == swap_amount {
				return TaigaSwap::<T>::swap(who, supply_currency_id, target_currency_id, limit);
			}
		}

		Err(Error::<T>::CannotSwap.into())
	}

	fn swap_by_path(
		who: &T::AccountId,
		swap_path: &[CurrencyId],
		limit: SwapLimit<Balance>,
	) -> Result<(Balance, Balance), DispatchError> {
		DexSwap::<T>::swap_by_path(who, swap_path, limit)
	}

	// Both DexSwap and TaigaSwap do not support swap by aggregated path.
	fn swap_by_aggregated_path(
		_who: &T::AccountId,
		_swap_path: &[SwapPath],
		_limit: SwapLimit<Balance>,
	) -> Result<(Balance, Balance), DispatchError> {
		Err(Error::<T>::CannotSwap.into())
	}
}

/// Choose the best price to execute swap:
/// 1. fully execute the swap by DEX
/// 2. fully execute the swap by Taiga
/// 3. aggregated swap by DEX and Taiga
pub struct AggregatedSwap<T>(PhantomData<T>);

struct AggregatedSwapParams {
	dex_result: Option<(Balance, Balance)>,
	taiga_result: Option<(Balance, Balance)>,
	aggregated_result: Option<(Balance, Balance)>,
	swap_amount: Option<(Balance, Balance)>,
}

impl<T: Config> AggregatedSwap<T> {
	fn get_swap_params(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> AggregatedSwapParams {
		let mut swap_amount: Option<(Balance, Balance)> = None;

		let dex_result = DexSwap::<T>::get_swap_amount(supply_currency_id, target_currency_id, limit);
		let taiga_result = TaigaSwap::<T>::get_swap_amount(supply_currency_id, target_currency_id, limit);
		let aggregated_result = Pallet::<T>::aggregated_swap_paths((supply_currency_id, target_currency_id))
			.and_then(|paths| Pallet::<T>::get_aggregated_swap_amount(&paths, limit));

		for result in sp_std::vec![dex_result, taiga_result, aggregated_result].iter() {
			if let Some((supply_amount, target_amount)) = *result {
				if let Some((candidate_supply_amount, candidate_target_amount)) = swap_amount {
					match limit {
						SwapLimit::ExactSupply(_, _) => {
							if target_amount > candidate_target_amount {
								swap_amount = *result;
							}
						}
						SwapLimit::ExactTarget(_, _) => {
							if supply_amount < candidate_supply_amount {
								swap_amount = *result;
							}
						}
					}
				} else {
					swap_amount = *result;
				}
			}
		}

		AggregatedSwapParams {
			dex_result,
			taiga_result,
			aggregated_result,
			swap_amount,
		}
	}
}

impl<T: Config> Swap<T::AccountId, Balance, CurrencyId> for AggregatedSwap<T> {
	fn get_swap_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> Option<(Balance, Balance)> {
		Self::get_swap_params(supply_currency_id, target_currency_id, limit).swap_amount
	}

	fn swap(
		who: &T::AccountId,
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		let AggregatedSwapParams {
			dex_result,
			taiga_result,
			aggregated_result,
			swap_amount,
		} = Self::get_swap_params(supply_currency_id, target_currency_id, limit);

		if swap_amount.is_some() {
			if dex_result == swap_amount {
				return DexSwap::<T>::swap(who, supply_currency_id, target_currency_id, limit);
			} else if taiga_result == swap_amount {
				return TaigaSwap::<T>::swap(who, supply_currency_id, target_currency_id, limit);
			} else if aggregated_result == swap_amount {
				let aggregated_swap_paths =
					Pallet::<T>::aggregated_swap_paths((supply_currency_id, target_currency_id))
						.ok_or(Error::<T>::CannotSwap)?;
				return Pallet::<T>::do_aggregated_swap(who, &aggregated_swap_paths, limit);
			}
		}

		Err(Error::<T>::CannotSwap.into())
	}

	// AggregatedSwap support swap by aggregated path.
	fn swap_by_aggregated_path(
		who: &T::AccountId,
		swap_path: &[SwapPath],
		limit: SwapLimit<Balance>,
	) -> Result<(Balance, Balance), DispatchError> {
		Pallet::<T>::do_aggregated_swap(who, swap_path, limit)
	}
}

pub struct RebasedStableAssetErrorConvertor<T>(PhantomData<T>);
impl<T: Config> Convert<RebasedStableAssetError, DispatchError> for RebasedStableAssetErrorConvertor<T> {
	fn convert(e: RebasedStableAssetError) -> DispatchError {
		match e {
			RebasedStableAssetError::InvalidPoolId => Error::<T>::InvalidPoolId.into(),
			RebasedStableAssetError::InvalidTokenIndex => Error::<T>::InvalidTokenIndex.into(),
		}
	}
}
