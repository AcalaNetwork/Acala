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

//! Xcm Interface module.
//!
//! This module interfaces Acala native modules with the Relaychain / parachains via the use of XCM.
//! Functions in this module will create XCM messages that performs the requested functions and
//! send the messages out to the intended destination.
//!
//! This module hides away XCM layer from native modules via the use of traits.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{pallet_prelude::*, traits::Get};
use frame_system::pallet_prelude::*;
use module_support::{relaychain::CallBuilder, HomaSubAccountXcm};
use orml_traits::XcmTransfer;
use primitives::{Balance, CurrencyId, EraIndex};
use scale_info::TypeInfo;
use sp_runtime::traits::Convert;
use sp_std::{convert::From, prelude::*, vec, vec::Vec};
use xcm::{prelude::*, v3::Weight as XcmWeight};

mod mocks;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[derive(Encode, Decode, Eq, PartialEq, Clone, RuntimeDebug, TypeInfo)]
	pub enum XcmInterfaceOperation {
		// XTokens
		XtokensTransfer,
		// Homa
		HomaWithdrawUnbonded,
		HomaBondExtra,
		HomaUnbond,
		// Parachain fee with location info
		ParachainFee(Box<Location>),
		// `XcmPallet::reserve_transfer_assets` call via proxy account
		ProxyReserveTransferAssets,
		HomaNominate,
	}

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_xcm::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Origin represented Governance
		type UpdateOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;

		/// The currency id of the Staking asset
		#[pallet::constant]
		type StakingCurrencyId: Get<CurrencyId>;

		/// The account of parachain on the relaychain.
		#[pallet::constant]
		type ParachainAccount: Get<Self::AccountId>;

		/// Unbonding slashing spans for unbonding on the relaychain.
		#[pallet::constant]
		type RelayChainUnbondingSlashingSpans: Get<EraIndex>;

		/// The convert for convert sovereign subacocunt index to the Location where the
		/// staking currencies are sent to.
		type SovereignSubAccountLocationConvert: Convert<u16, Location>;

		/// The Call builder for communicating with RelayChain via XCM messaging.
		type RelayChainCallBuilder: CallBuilder<RelayChainAccountId = Self::AccountId, Balance = Balance>;

		/// The interface to Cross-chain transfer.
		type XcmTransfer: XcmTransfer<Self::AccountId, Balance, CurrencyId>;

		/// Self parachain location.
		#[pallet::constant]
		type SelfLocation: Get<Location>;

		/// Convert AccountId to Location to build XCM message.
		type AccountIdToLocation: Convert<Self::AccountId, Location>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The xcm operation have failed
		XcmFailed,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Xcm dest weight has been updated.
		XcmDestWeightUpdated {
			xcm_operation: XcmInterfaceOperation,
			new_xcm_dest_weight: XcmWeight,
		},
		/// Xcm dest weight has been updated.
		XcmFeeUpdated {
			xcm_operation: XcmInterfaceOperation,
			new_xcm_dest_weight: Balance,
		},
	}

	/// The dest weight limit and fee for execution XCM msg sended by XcmInterface. Must be
	/// sufficient, otherwise the execution of XCM msg on relaychain will fail.
	///
	/// XcmDestWeightAndFee: map: XcmInterfaceOperation => (Weight, Balance)
	#[pallet::storage]
	#[pallet::getter(fn xcm_dest_weight_and_fee)]
	pub type XcmDestWeightAndFee<T: Config> =
		StorageMap<_, Twox64Concat, XcmInterfaceOperation, (XcmWeight, Balance), ValueQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets the xcm_dest_weight and fee for XCM operation of XcmInterface.
		///
		/// Parameters:
		/// - `updates`: vec of tuple: (XcmInterfaceOperation, WeightChange, FeeChange).
		#[pallet::call_index(0)]
		#[pallet::weight(frame_support::weights::Weight::from_parts(10_000_000, 0))]
		pub fn update_xcm_dest_weight_and_fee(
			origin: OriginFor<T>,
			updates: Vec<(XcmInterfaceOperation, Option<XcmWeight>, Option<Balance>)>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			for (operation, weight_change, fee_change) in updates {
				XcmDestWeightAndFee::<T>::mutate(&operation, |(weight, fee)| {
					if let Some(new_weight) = weight_change {
						*weight = new_weight;
						Self::deposit_event(Event::<T>::XcmDestWeightUpdated {
							xcm_operation: operation.clone(),
							new_xcm_dest_weight: new_weight,
						});
					}
					if let Some(new_fee) = fee_change {
						*fee = new_fee;
						Self::deposit_event(Event::<T>::XcmFeeUpdated {
							xcm_operation: operation.clone(),
							new_xcm_dest_weight: new_fee,
						});
					}
				});
			}

			Ok(())
		}
	}

	impl<T: Config> HomaSubAccountXcm<T::AccountId, Balance> for Pallet<T> {
		type RelayChainAccountId = T::AccountId;

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
				WeightLimit::Limited(Self::xcm_dest_weight_and_fee(XcmInterfaceOperation::XtokensTransfer).0),
			)
			.map(|_| ())
		}

		/// Send XCM message to the relaychain for sub account to withdraw_unbonded staking currency
		/// and send it back.
		fn withdraw_unbonded_from_sub_account(sub_account_index: u16, amount: Balance) -> DispatchResult {
			let (xcm_dest_weight, xcm_fee) = Self::xcm_dest_weight_and_fee(XcmInterfaceOperation::HomaWithdrawUnbonded);

			// TODO: config xcm_dest_weight and fee for withdraw_unbonded and transfer seperately.
			// Temperarily use double fee.
			let xcm_message = T::RelayChainCallBuilder::finalize_multiple_calls_into_xcm_message(
				vec![
					(
						T::RelayChainCallBuilder::utility_as_derivative_call(
							T::RelayChainCallBuilder::staking_withdraw_unbonded(
								T::RelayChainUnbondingSlashingSpans::get(),
							),
							sub_account_index,
						),
						xcm_dest_weight,
					),
					(
						T::RelayChainCallBuilder::utility_as_derivative_call(
							T::RelayChainCallBuilder::balances_transfer_keep_alive(T::ParachainAccount::get(), amount),
							sub_account_index,
						),
						xcm_dest_weight,
					),
				],
				xcm_fee.saturating_mul(2),
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

		/// Send XCM message to the relaychain for sub account to nominate.
		fn nominate_on_sub_account(sub_account_index: u16, targets: Vec<Self::RelayChainAccountId>) -> DispatchResult {
			let (xcm_dest_weight, xcm_fee) = Self::xcm_dest_weight_and_fee(XcmInterfaceOperation::HomaNominate);
			let xcm_message = T::RelayChainCallBuilder::finalize_call_into_xcm_message(
				T::RelayChainCallBuilder::utility_as_derivative_call(
					T::RelayChainCallBuilder::staking_nominate(targets.clone()),
					sub_account_index,
				),
				xcm_fee,
				xcm_dest_weight,
			);
			let result = pallet_xcm::Pallet::<T>::send_xcm(Here, Parent, xcm_message);
			log::debug!(
				target: "xcm-interface",
				"subaccount {:?} send XCM to nominate {:?}, result: {:?}",
				sub_account_index, targets, result
			);

			ensure!(result.is_ok(), Error::<T>::XcmFailed);
			Ok(())
		}

		/// The fee of cross-chain transfer is deducted from the recipient.
		fn get_xcm_transfer_fee() -> Balance {
			Self::xcm_dest_weight_and_fee(XcmInterfaceOperation::XtokensTransfer).1
		}

		/// The fee of parachain transfer.
		fn get_parachain_fee(location: Location) -> Balance {
			Self::xcm_dest_weight_and_fee(XcmInterfaceOperation::ParachainFee(Box::new(location))).1
		}
	}
}
