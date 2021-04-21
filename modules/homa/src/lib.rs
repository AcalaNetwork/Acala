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

//! # Homa Module
//!
//! ## Overview
//!
//! The user entrance of Homa protocol. User can inject DOT into the staking
//! pool and get LDOT, which is the redemption voucher for DOT owned by the
//! staking pool. The staking pool will staking these DOT to get staking
//! rewards. Holders of LDOT can choose different ways to redeem DOT.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{pallet_prelude::*, transactional};
use frame_system::pallet_prelude::*;
use primitives::{Balance, EraIndex};
use sp_runtime::RuntimeDebug;
use support::HomaProtocol;

pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

/// Redemption modes:
/// 1. Immediately: User will immediately get back DOT from the free pool,
/// which is a liquid pool operated by staking pool, but they have to pay
/// extra fee. 2. Target: User can claim the unclaimed unbonding DOT of
/// specific era, after the remaining unbinding period has passed, users can
/// get back the DOT. 3. WaitForUnbonding: User request unbond, the staking
/// pool will process unbonding in the next era, and user needs to wait for
/// the complete unbonding era which determined by Polkadot.
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq)]
pub enum RedeemStrategy {
	Immediately,
	Target(EraIndex),
	WaitForUnbonding,
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The core of Homa protocol.
		type Homa: HomaProtocol<Self::AccountId, Balance, EraIndex>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Inject DOT to staking pool and mint LDOT in a certain exchange rate
		/// decided by staking pool.
		///
		/// - `amount`: the DOT amount to inject into staking pool.
		#[pallet::weight(<T as Config>::WeightInfo::mint())]
		#[transactional]
		pub fn mint(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			T::Homa::mint(&who, amount)?;
			Ok(().into())
		}

		/// Burn LDOT and redeem DOT from staking pool.
		///
		/// - `amount`: the LDOT amount to redeem.
		/// - `strategy`: redemption mode.
		#[pallet::weight(match *strategy {
			RedeemStrategy::Immediately => <T as Config>::WeightInfo::redeem_immediately(),
			RedeemStrategy::Target(_) => <T as Config>::WeightInfo::redeem_by_claim_unbonding(),
			RedeemStrategy::WaitForUnbonding => <T as Config>::WeightInfo::redeem_wait_for_unbonding(),
		})]
		#[transactional]
		pub fn redeem(
			origin: OriginFor<T>,
			#[pallet::compact] amount: Balance,
			strategy: RedeemStrategy,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			match strategy {
				RedeemStrategy::Immediately => {
					T::Homa::redeem_by_free_unbonded(&who, amount)?;
				}
				RedeemStrategy::Target(target_era) => {
					T::Homa::redeem_by_claim_unbonding(&who, amount, target_era)?;
				}
				RedeemStrategy::WaitForUnbonding => {
					T::Homa::redeem_by_unbond(&who, amount)?;
				}
			}
			Ok(().into())
		}

		/// Get back those DOT that have been unbonded.
		#[pallet::weight(<T as Config>::WeightInfo::withdraw_redemption())]
		#[transactional]
		pub fn withdraw_redemption(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			T::Homa::withdraw_redemption(&who)?;
			Ok(().into())
		}
	}
}
