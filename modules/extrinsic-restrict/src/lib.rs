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

//! Extrinsic Restrict module.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{
	log,
	pallet_prelude::*,
	traits::{Contains, IsSubType},
	transactional, PalletId,
};
use frame_system::{ensure_signed, pallet_prelude::*};
use module_support::{Rate, Ratio};
use orml_traits::Change;
use orml_traits::MultiCurrency;
use primitives::{Balance, CurrencyId};
use scale_info::TypeInfo;
use sp_arithmetic::traits::CheckedRem;
use sp_runtime::{
	traits::{
		AccountIdConversion, BlockNumberProvider, Bounded, CheckedDiv, CheckedSub, Convert, DispatchInfoOf, One,
		PostDispatchInfoOf, Saturating, SignedExtension, UniqueSaturatedInto, Zero,
	},
	transaction_validity::{InvalidTransaction, TransactionValidityError},
	ArithmeticError, FixedPointNumber,
};
use sp_std::{cmp::Ordering, convert::From, marker::PhantomData, prelude::*, vec, vec::Vec};
use xcm::latest::prelude::*;
use xcm::VersionedMultiAsset;

pub use module::*;
// pub use weights::WeightInfo;

mod mock;
mod tests;
pub mod weights;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, Default)]
	pub struct XtokensTransferLimit {
		accumulation_limit: Option<Balance>,
		period_accumulation: Balance,
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Origin represented Governance
		type GovernanceOrigin: EnsureOrigin<<Self as frame_system::Config>::Origin>;

		#[pallet::constant]
		type XtokensTransferCumulativeResetPeriod: Get<Self::BlockNumber>;

		type CurrencyIdConvert: Convert<MultiLocation, Option<CurrencyId>>;

		type XtokensTransferWhitelist: Contains<Self::AccountId>;

		// /// Weight information for the extrinsics in this module.
		// type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		///	The mint amount is below the threshold.
		BelowMintThreshold,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The redeemer withdraw expired redemption.
		WithdrawRedemption {
			redeemer: T::AccountId,
			redemption_amount: Balance,
		},
	}

	/// Requests to redeem staked currencies.
	///
	/// RedeemRequests: Map: AccountId => Option<(liquid_amount: Balance, allow_fast_match: bool)>
	#[pallet::storage]
	#[pallet::getter(fn redeem_requests)]
	pub type XtokensTransferLimits<T: Config> =
		StorageMap<_, Twox64Concat, CurrencyId, XtokensTransferLimit, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn hihi)]
	pub type Hihi<T: Config> = StorageMap<_, Twox64Concat, CurrencyId, XtokensTransferLimit, ValueQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(now: T::BlockNumber) -> Weight {
			if now
				.checked_rem(&T::XtokensTransferCumulativeResetPeriod::get())
				.map_or(false, |n| n.is_zero())
			{
				XtokensTransferLimits::<T>::translate(
					|_key, val: XtokensTransferLimit| -> Option<XtokensTransferLimit> {
						Some(XtokensTransferLimit {
							period_accumulation: Zero::zero(),
							..val
						})
					},
				);
			}

			0
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10000)]
		#[transactional]
		pub fn update_xtokens_transfer_limit(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			accumulation_limit_change: Option<Option<Balance>>,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			XtokensTransferLimits::<T>::try_mutate_exists(currency_id, |maybe_limit| -> DispatchResult {
				let mut limit = maybe_limit.take().unwrap_or_default();

				if let Some(change) = accumulation_limit_change {
					limit.accumulation_limit = change;
				}

				*maybe_limit = if limit.accumulation_limit.is_some() {
					Some(limit)
				} else {
					None
				};

				Ok(())
			})?;

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn convert_multi_asset(asset: MultiAsset) -> Option<(CurrencyId, Balance)> {
			if let MultiAsset {
				id: Concrete(location),
				fun: Fungible(amount),
			} = asset
			{
				if let Some(currency_id) = T::CurrencyIdConvert::convert(location) {
					return Some((currency_id, amount));
				}
			}

			None
		}

		pub fn check_xtokens_transfer_pass(who: &T::AccountId, assets: &Vec<(CurrencyId, Balance)>) -> bool {
			if T::XtokensTransferWhitelist::contains(who) {
				return true;
			}

			for (currency_id, amount) in assets {
				match XtokensTransferLimits::<T>::get(currency_id) {
					Some(XtokensTransferLimit {
						accumulation_limit: Some(cap),
						period_accumulation,
					}) => {
						if period_accumulation.saturating_add(*amount) > period_accumulation {
							return false;
						}
					}
					_ => {}
				}
			}

			true
		}

		pub fn accumulate_xtokens_transfer_assets(who: &T::AccountId, assets: &Vec<(CurrencyId, Balance)>) {
			if !T::XtokensTransferWhitelist::contains(who) {
				for (currency_id, amount) in assets {
					XtokensTransferLimits::<T>::mutate_exists(currency_id, |maybe_limit| {
						if let Some(limit) = maybe_limit.as_mut() {
							limit.period_accumulation = limit.period_accumulation.saturating_add(*amount);
						}
					});
				}
			}
		}
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct XtokensTransferRestrict<T: Config + Send + Sync>(PhantomData<T>);

impl<T: Config + Send + Sync> sp_std::fmt::Debug for XtokensTransferRestrict<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "XtokensTransferRestrict")
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T: Config + Send + Sync + orml_xtokens::Config<Balance = Balance, CurrencyId = CurrencyId>> SignedExtension
	for XtokensTransferRestrict<T>
where
	T::Call: IsSubType<orml_xtokens::Call<T>>,
{
	const IDENTIFIER: &'static str = "XtokensTransferRestrict";
	type AccountId = T::AccountId;
	type Call = T::Call;
	type AdditionalSigned = ();
	type Pre = (Self::AccountId, Vec<(CurrencyId, Balance)>);

	fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> {
		Ok(())
	}

	fn validate(
		&self,
		_who: &Self::AccountId,
		_call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> TransactionValidity {
		Ok(ValidTransaction::default())
	}

	fn pre_dispatch(
		self,
		who: &Self::AccountId,
		call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		let mut xtokens_transfer_assets: Vec<(CurrencyId, Balance)> = vec![];

		// todo: derive call

		match call.is_sub_type() {
			Some(orml_xtokens::Call::transfer {
				currency_id, amount, ..
			}) => {
				xtokens_transfer_assets.push((*currency_id, *amount));
			}
			Some(orml_xtokens::Call::transfer_multiasset { asset, .. }) => {
				if let Ok(asset) = (*asset.clone()).try_into() {
					if let Some(asset) = Pallet::<T>::convert_multi_asset(asset) {
						xtokens_transfer_assets.push(asset)
					}
				}
			}
			Some(orml_xtokens::Call::transfer_with_fee {
				currency_id, amount, ..
			}) => {
				xtokens_transfer_assets.push((*currency_id, *amount));
			}
			Some(orml_xtokens::Call::transfer_multiasset_with_fee { asset, .. }) => {
				if let Ok(asset) = (*asset.clone()).try_into() {
					if let Some(asset) = Pallet::<T>::convert_multi_asset(asset) {
						xtokens_transfer_assets.push(asset)
					}
				}
			}
			Some(orml_xtokens::Call::transfer_multicurrencies { currencies, .. }) => {
				let assets: Vec<(CurrencyId, Balance)> = currencies.to_vec();
				xtokens_transfer_assets.extend(assets);
			}
			Some(orml_xtokens::Call::transfer_multiassets { assets, .. }) => {
				if let Ok(asset_list) = TryInto::<MultiAssets>::try_into(*assets.clone()) {
					for asset in asset_list.drain() {
						if let Some(v) = Pallet::<T>::convert_multi_asset(asset) {
							xtokens_transfer_assets.push(v)
						}
					}
				}
			}
			_ => {}
		};

		// todo: merge items with the same currency id

		ensure!(
			Pallet::<T>::check_xtokens_transfer_pass(who, &xtokens_transfer_assets),
			InvalidTransaction::Custom(11)
		);

		Ok((who.clone(), xtokens_transfer_assets))
	}

	fn post_dispatch(
		pre: Option<Self::Pre>,
		_info: &DispatchInfoOf<Self::Call>,
		_post_info: &PostDispatchInfoOf<Self::Call>,
		_len: usize,
		result: &DispatchResult,
	) -> Result<(), TransactionValidityError> {
		if result.is_ok() {
			if let Some((who, xtokens_transfer_assets)) = pre {
				Pallet::<T>::accumulate_xtokens_transfer_assets(&who, &xtokens_transfer_assets);
			}
		}

		Ok(())
	}
}

pub struct XtokensTransferFilter<T>(PhantomData<T>);
impl<T: Config> Contains<T::Call> for XtokensTransferFilter<T> {
	fn contains(call: &T::Call) -> bool {
		true
	}
}
