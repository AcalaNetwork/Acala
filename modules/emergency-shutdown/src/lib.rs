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

//! # Emergency Shutdown Module
//!
//! ## Overview
//!
//! When a black swan occurs such as price plunge or fatal bug, the highest
//! priority is to minimize user losses as much as possible. When the decision
//! to shutdown system is made, emergency shutdown module needs to trigger all
//! related module to halt, and start a series of operations including close
//! some user entry, freeze feed prices, run offchain worker to settle
//! CDPs has debit, cancel all active auctions module, when debits and gaps are
//! settled, the stable currency holder are allowed to refund a basket of
//! remaining collateral assets.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{pallet_prelude::*, transactional};
use frame_system::{ensure_signed, pallet_prelude::*};
use primitives::{Balance, CurrencyId};
use sp_runtime::{traits::Zero, FixedPointNumber};
use sp_std::prelude::*;
use support::{AuctionManager, CDPTreasury, EmergencyShutdown, PriceProvider, Ratio};

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config + loans::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The list of valid collateral currency types
		#[pallet::constant]
		type CollateralCurrencyIds: Get<Vec<CurrencyId>>;

		/// Price source to freeze currencies' price
		type PriceSource: PriceProvider<CurrencyId>;

		/// CDP treasury to escrow collateral assets after settlement
		type CDPTreasury: CDPTreasury<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;

		/// Check the auction cancellation to decide whether to open the final
		/// redemption
		type AuctionManagerHandler: AuctionManager<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;

		/// The origin which may trigger emergency shutdown. Root can always do
		/// this.
		type ShutdownOrigin: EnsureOrigin<Self::Origin>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// System has already been shutdown
		AlreadyShutdown,
		/// Must after system shutdown
		MustAfterShutdown,
		/// Final redemption is still not opened
		CanNotRefund,
		/// Exist potential surplus, means settlement has not been completed
		ExistPotentialSurplus,
		/// Exist unhandled debit, means settlement has not been completed
		ExistUnhandledDebit,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// Emergency shutdown occurs. \[block_number\]
		Shutdown(T::BlockNumber),
		/// The final redemption opened. \[block_number\]
		OpenRefund(T::BlockNumber),
		/// Refund info. \[caller, stable_coin_amount, refund_list\]
		Refund(T::AccountId, Balance, Vec<(CurrencyId, Balance)>),
	}

	/// Emergency shutdown flag
	///
	/// IsShutdown: bool
	#[pallet::storage]
	#[pallet::getter(fn is_shutdown)]
	pub type IsShutdown<T: Config> = StorageValue<_, bool, ValueQuery>;

	/// Open final redemption flag
	///
	/// CanRefund: bool
	#[pallet::storage]
	#[pallet::getter(fn can_refund)]
	pub type CanRefund<T: Config> = StorageValue<_, bool, ValueQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Start emergency shutdown
		///
		/// The dispatch origin of this call must be `ShutdownOrigin`.
		#[pallet::weight((T::WeightInfo::emergency_shutdown(T::CollateralCurrencyIds::get().len() as u32), DispatchClass::Operational))]
		#[transactional]
		pub fn emergency_shutdown(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			T::ShutdownOrigin::ensure_origin(origin)?;
			ensure!(!Self::is_shutdown(), Error::<T>::AlreadyShutdown);

			// get all collateral types
			let collateral_currency_ids = T::CollateralCurrencyIds::get();

			// lock price for every collateral
			for currency_id in collateral_currency_ids {
				<T as Config>::PriceSource::lock_price(currency_id);
			}

			IsShutdown::<T>::put(true);
			Self::deposit_event(Event::Shutdown(<frame_system::Pallet<T>>::block_number()));
			Ok(().into())
		}

		/// Open final redemption if settlement is completed.
		///
		/// The dispatch origin of this call must be `ShutdownOrigin`.
		#[pallet::weight((T::WeightInfo::open_collateral_refund(), DispatchClass::Operational))]
		#[transactional]
		pub fn open_collateral_refund(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			T::ShutdownOrigin::ensure_origin(origin)?;
			ensure!(Self::is_shutdown(), Error::<T>::MustAfterShutdown); // must after shutdown

			// Ensure all debits of CDPs have been settled, and all collateral auction has
			// been done or canceled. Settle all collaterals type CDPs which have debit,
			// cancel all collateral auctions in forward stage and wait for all collateral
			// auctions in reverse stage to be ended.
			let collateral_currency_ids = T::CollateralCurrencyIds::get();
			for currency_id in collateral_currency_ids {
				// there's no collateral auction
				ensure!(
					<T as Config>::AuctionManagerHandler::get_total_collateral_in_auction(currency_id).is_zero(),
					Error::<T>::ExistPotentialSurplus,
				);
				// there's on debit in CDP
				ensure!(
					<loans::Pallet<T>>::total_positions(currency_id).debit.is_zero(),
					Error::<T>::ExistUnhandledDebit,
				);
			}

			// Open refund stage
			CanRefund::<T>::put(true);
			Self::deposit_event(Event::OpenRefund(<frame_system::Pallet<T>>::block_number()));
			Ok(().into())
		}

		/// Refund a basket of remaining collateral assets to caller
		///
		/// - `amount`: stable currency amount used to refund.
		#[pallet::weight(T::WeightInfo::refund_collaterals(T::CollateralCurrencyIds::get().len() as u32))]
		#[transactional]
		pub fn refund_collaterals(
			origin: OriginFor<T>,
			#[pallet::compact] amount: Balance,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(Self::can_refund(), Error::<T>::CanNotRefund);

			let refund_ratio: Ratio = <T as Config>::CDPTreasury::get_debit_proportion(amount);
			let collateral_currency_ids = T::CollateralCurrencyIds::get();

			// burn caller's stable currency by CDP treasury
			<T as Config>::CDPTreasury::burn_debit(&who, amount)?;

			let mut refund_assets: Vec<(CurrencyId, Balance)> = vec![];
			// refund collaterals to caller by CDP treasury
			for currency_id in collateral_currency_ids {
				let refund_amount =
					refund_ratio.saturating_mul_int(<T as Config>::CDPTreasury::get_total_collaterals(currency_id));

				if !refund_amount.is_zero() {
					<T as Config>::CDPTreasury::withdraw_collateral(&who, currency_id, refund_amount)?;
					refund_assets.push((currency_id, refund_amount));
				}
			}

			Self::deposit_event(Event::Refund(who, amount, refund_assets));
			Ok(().into())
		}
	}
}

impl<T: Config> EmergencyShutdown for Pallet<T> {
	fn is_shutdown() -> bool {
		Self::is_shutdown()
	}
}
