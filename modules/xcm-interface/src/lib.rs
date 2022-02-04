// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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

//! Xcm Interface module.
//!
//! This module interfaces Acala native modules with the Relaychain / parachains via the use of XCM.
//! Functions in this module will create XCM messages that performs the requested functions and
//! send the messages out to the intended destination.
//!
//! This module hides away XCM layer from native modules via the use of traits.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{log, pallet_prelude::*, transactional, weights::Weight};
use frame_system::pallet_prelude::*;
use module_support::{CallBuilder, HomaSubAccountXcm};
use orml_traits::XcmTransfer;
use primitives::{Balance, CurrencyId, EraIndex};
use scale_info::TypeInfo;
use sp_runtime::traits::Convert;
use sp_std::{convert::From, prelude::*, vec, vec::Vec};
use xcm::latest::prelude::*;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, TypeInfo)]
	pub enum XcmInterfaceOperation {
		// XTokens
		XtokensTransfer,
		// Homa
		HomaWithdrawUnbonded,
		HomaBondExtra,
		HomaUnbond,
	}

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_xcm::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Origin represented Governance
		type UpdateOrigin: EnsureOrigin<<Self as frame_system::Config>::Origin>;

		/// The currency id of the Staking asset
		#[pallet::constant]
		type StakingCurrencyId: Get<CurrencyId>;

		/// The account of parachain on the relaychain.
		#[pallet::constant]
		type ParachainAccount: Get<Self::AccountId>;

		/// Unbonding slashing spans for unbonding on the relaychain.
		#[pallet::constant]
		type RelayChainUnbondingSlashingSpans: Get<EraIndex>;

		/// The convert for convert sovereign subacocunt index to the MultiLocation where the
		/// staking currencies are sent to.
		type SovereignSubAccountLocationConvert: Convert<u16, MultiLocation>;

		/// The Call builder for communicating with RelayChain via XCM messaging.
		type RelayChainCallBuilder: CallBuilder<AccountId = Self::AccountId, Balance = Balance>;

		/// The interface to Cross-chain transfer.
		type XcmTransfer: XcmTransfer<Self::AccountId, Balance, CurrencyId>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The xcm operation have failed
		XcmFailed,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Xcm dest weight has been updated. \[xcm_operation, new_xcm_dest_weight\]
		XcmDestWeightUpdated(XcmInterfaceOperation, Weight),
		/// Xcm dest weight has been updated. \[xcm_operation, new_xcm_dest_weight\]
		XcmFeeUpdated(XcmInterfaceOperation, Balance),
	}

	/// The dest weight limit and fee for execution XCM msg sended by XcmInterface. Must be
	/// sufficient, otherwise the execution of XCM msg on relaychain will fail.
	///
	/// XcmDestWeightAndFee: map: XcmInterfaceOperation => (Weight, Balance)
	#[pallet::storage]
	#[pallet::getter(fn xcm_dest_weight_and_fee)]
	pub type XcmDestWeightAndFee<T: Config> =
		StorageMap<_, Twox64Concat, XcmInterfaceOperation, (Weight, Balance), ValueQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets the xcm_dest_weight and fee for XCM operation of XcmInterface.
		///
		/// Parameters:
		/// - `updates`: vec of tuple: (XcmInterfaceOperation, WeightChange, FeeChange).
		#[pallet::weight(10_000_000)]
		#[transactional]
		pub fn update_xcm_dest_weight_and_fee(
			origin: OriginFor<T>,
			updates: Vec<(XcmInterfaceOperation, Option<Weight>, Option<Balance>)>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			for (operation, weight_change, fee_change) in updates {
				XcmDestWeightAndFee::<T>::mutate(operation, |(weight, fee)| {
					if let Some(new_weight) = weight_change {
						*weight = new_weight;
						Self::deposit_event(Event::<T>::XcmDestWeightUpdated(operation, new_weight));
					}
					if let Some(new_fee) = fee_change {
						*fee = new_fee;
						Self::deposit_event(Event::<T>::XcmFeeUpdated(operation, new_fee));
					}
				});
			}

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {}

	impl<T: Config> HomaSubAccountXcm<T::AccountId, Balance> for Pallet<T> {
		/// Cross-chain transfer staking currency to sub account on relaychain.
		fn transfer_staking_to_sub_account(
			sender: &T::AccountId,
			sub_account_index: u16,
			amount: Balance,
		) -> DispatchResult {
			T::XcmTransfer::transfer(
				sender.clone(),
				T::StakingCurrencyId::get(),
				amount,
				T::SovereignSubAccountLocationConvert::convert(sub_account_index),
				Self::xcm_dest_weight_and_fee(XcmInterfaceOperation::XtokensTransfer).0,
			)
		}

		/// Send XCM message to the relaychain for sub account to withdraw_unbonded staking currency
		/// and send it back.
		fn withdraw_unbonded_from_sub_account(sub_account_index: u16, amount: Balance) -> DispatchResult {
			let (xcm_dest_weight, xcm_fee) = Self::xcm_dest_weight_and_fee(XcmInterfaceOperation::HomaWithdrawUnbonded);
			let xcm_message = T::RelayChainCallBuilder::finalize_call_into_xcm_message(
				T::RelayChainCallBuilder::utility_as_derivative_call(
					T::RelayChainCallBuilder::utility_batch_call(vec![
						T::RelayChainCallBuilder::staking_withdraw_unbonded(T::RelayChainUnbondingSlashingSpans::get()),
						T::RelayChainCallBuilder::balances_transfer_keep_alive(T::ParachainAccount::get(), amount),
					]),
					sub_account_index,
				),
				xcm_fee,
				xcm_dest_weight,
			);
			let result = pallet_xcm::Pallet::<T>::send_xcm(Here, Parent, xcm_message);
			log::debug!(
				target: "xcm-interface",
				"subaccount {:?} send XCM to withdraw unbonded {:?}, result: {:?}",
				sub_account_index, amount, result
			);

			ensure!(result.is_ok(), Error::<T>::XcmFailed);
			Ok(())
		}

		/// Send XCM message to the relaychain for sub account to bond extra.
		fn bond_extra_on_sub_account(sub_account_index: u16, amount: Balance) -> DispatchResult {
			let (xcm_dest_weight, xcm_fee) = Self::xcm_dest_weight_and_fee(XcmInterfaceOperation::HomaBondExtra);
			let xcm_message = T::RelayChainCallBuilder::finalize_call_into_xcm_message(
				T::RelayChainCallBuilder::utility_as_derivative_call(
					T::RelayChainCallBuilder::staking_bond_extra(amount),
					sub_account_index,
				),
				xcm_fee,
				xcm_dest_weight,
			);
			let result = pallet_xcm::Pallet::<T>::send_xcm(Here, Parent, xcm_message);
			log::debug!(
				target: "xcm-interface",
				"subaccount {:?} send XCM to bond {:?}, result: {:?}",
				sub_account_index, amount, result,
			);

			ensure!(result.is_ok(), Error::<T>::XcmFailed);
			Ok(())
		}

		/// Send XCM message to the relaychain for sub account to unbond.
		fn unbond_on_sub_account(sub_account_index: u16, amount: Balance) -> DispatchResult {
			let (xcm_dest_weight, xcm_fee) = Self::xcm_dest_weight_and_fee(XcmInterfaceOperation::HomaUnbond);
			let xcm_message = T::RelayChainCallBuilder::finalize_call_into_xcm_message(
				T::RelayChainCallBuilder::utility_as_derivative_call(
					T::RelayChainCallBuilder::staking_unbond(amount),
					sub_account_index,
				),
				xcm_fee,
				xcm_dest_weight,
			);
			let result = pallet_xcm::Pallet::<T>::send_xcm(Here, Parent, xcm_message);
			log::debug!(
				target: "xcm-interface",
				"subaccount {:?} send XCM to unbond {:?}, result: {:?}",
				sub_account_index, amount, result
			);

			ensure!(result.is_ok(), Error::<T>::XcmFailed);
			Ok(())
		}

		/// The fee of cross-chain transfer is deducted from the recipient.
		fn get_xcm_transfer_fee() -> Balance {
			Self::xcm_dest_weight_and_fee(XcmInterfaceOperation::XtokensTransfer).1
		}
	}
}
