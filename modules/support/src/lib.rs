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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::from_over_into)]
#![allow(clippy::type_complexity)]

use frame_support::pallet_prelude::{DispatchClass, Pays, Weight};
use primitives::{task::TaskResult, Balance, CurrencyId, Multiplier, ReserveIdentifier};
use sp_runtime::{
	traits::CheckedDiv, transaction_validity::TransactionValidityError, DispatchError, DispatchResult, FixedU128,
};
use sp_std::{prelude::*, result::Result, vec};
use xcm::prelude::*;

pub mod bounded;
pub mod dex;
pub mod earning;
pub mod evm;
pub mod homa;
pub mod honzon;
pub mod incentives;
pub mod mocks;
pub mod relaychain;
pub mod stable_asset;

pub use crate::bounded::*;
pub use crate::dex::*;
pub use crate::earning::*;
pub use crate::evm::*;
pub use crate::homa::*;
pub use crate::honzon::*;
pub use crate::incentives::*;
pub use crate::stable_asset::*;

pub type Price = FixedU128;
pub type ExchangeRate = FixedU128;
pub type Ratio = FixedU128;
pub type Rate = FixedU128;

/// Implement this StoredMap to replace https://github.com/paritytech/substrate/blob/569aae5341ea0c1d10426fa1ec13a36c0b64393b/frame/system/src/lib.rs#L1679
/// NOTE: If use module-evm, need regards existed `frame_system::Account` also exists
/// `pallet_balances::Account`, even if it's AccountData is default. (This kind of account is
/// usually created by inc_provider), so that `repatriate_reserved` can transfer reserved balance to
/// contract account, which is created by `inc_provider`.
pub struct SystemAccountStore<T>(sp_std::marker::PhantomData<T>);
impl<T: frame_system::Config> frame_support::traits::StoredMap<T::AccountId, T::AccountData> for SystemAccountStore<T> {
	fn get(k: &T::AccountId) -> T::AccountData {
		frame_system::Account::<T>::get(k).data
	}

	fn try_mutate_exists<R, E: From<DispatchError>>(
		k: &T::AccountId,
		f: impl FnOnce(&mut Option<T::AccountData>) -> Result<R, E>,
	) -> Result<R, E> {
		let account = frame_system::Account::<T>::get(k);
		let is_default = account.data == T::AccountData::default();

		// if System Account exists, act its Balances Account also exists.
		let mut some_data = if is_default && !frame_system::Pallet::<T>::account_exists(k) {
			None
		} else {
			Some(account.data)
		};

		let result = f(&mut some_data)?;
		if frame_system::Pallet::<T>::providers(k) > 0 || frame_system::Pallet::<T>::sufficients(k) > 0 {
			frame_system::Account::<T>::mutate(k, |a| a.data = some_data.unwrap_or_default());
		} else {
			frame_system::Account::<T>::remove(k)
		}
		Ok(result)
	}
}

pub trait PriceProvider<CurrencyId> {
	fn get_price(currency_id: CurrencyId) -> Option<Price>;
	fn get_relative_price(base: CurrencyId, quote: CurrencyId) -> Option<Price> {
		if let (Some(base_price), Some(quote_price)) = (Self::get_price(base), Self::get_price(quote)) {
			base_price.checked_div(&quote_price)
		} else {
			None
		}
	}
}

pub trait DEXPriceProvider<CurrencyId> {
	fn get_relative_price(base: CurrencyId, quote: CurrencyId) -> Option<ExchangeRate>;
}

pub trait LockablePrice<CurrencyId> {
	fn lock_price(currency_id: CurrencyId) -> DispatchResult;
	fn unlock_price(currency_id: CurrencyId) -> DispatchResult;
}

pub trait ExchangeRateProvider {
	fn get_exchange_rate() -> ExchangeRate;
}

pub trait TransactionPayment<AccountId, Balance, NegativeImbalance> {
	fn reserve_fee(who: &AccountId, fee: Balance, named: Option<ReserveIdentifier>) -> Result<Balance, DispatchError>;
	fn unreserve_fee(who: &AccountId, fee: Balance, named: Option<ReserveIdentifier>) -> Balance;
	fn unreserve_and_charge_fee(
		who: &AccountId,
		weight: Weight,
	) -> Result<(Balance, NegativeImbalance), TransactionValidityError>;
	fn refund_fee(who: &AccountId, weight: Weight, payed: NegativeImbalance) -> Result<(), TransactionValidityError>;
	fn charge_fee(
		who: &AccountId,
		len: u32,
		weight: Weight,
		tip: Balance,
		pays_fee: Pays,
		class: DispatchClass,
	) -> Result<(), TransactionValidityError>;
	fn weight_to_fee(weight: Weight) -> Balance;
	fn apply_multiplier_to_fee(fee: Balance, multiplier: Option<Multiplier>) -> Balance;
}

/// Dispatchable tasks
pub trait DispatchableTask {
	fn dispatch(self, weight: Weight) -> TaskResult;
}

/// Idle scheduler trait
pub trait IdleScheduler<Index, Task> {
	fn schedule(task: Task) -> Result<Index, DispatchError>;
	fn dispatch(id: Index, weight: Weight) -> Weight;
}

#[cfg(feature = "std")]
impl DispatchableTask for () {
	fn dispatch(self, _weight: Weight) -> TaskResult {
		unimplemented!()
	}
}

#[cfg(feature = "std")]
impl<Index, Task> IdleScheduler<Index, Task> for () {
	fn schedule(_task: Task) -> Result<Index, DispatchError> {
		unimplemented!()
	}
	fn dispatch(_id: Index, _weight: Weight) -> Weight {
		unimplemented!()
	}
}

#[impl_trait_for_tuples::impl_for_tuples(30)]
pub trait OnNewEra<EraIndex> {
	fn on_new_era(era: EraIndex);
}

pub trait NomineesProvider<AccountId> {
	fn nominees() -> Vec<AccountId>;
	fn nominees_in_groups(group_index_list: Vec<u16>) -> Vec<(u16, Vec<AccountId>)>;
}

impl<AccountId> NomineesProvider<AccountId> for () {
	fn nominees() -> Vec<AccountId> {
		vec![]
	}

	fn nominees_in_groups(_: Vec<u16>) -> Vec<(u16, Vec<AccountId>)> {
		vec![]
	}
}

pub trait LiquidateCollateral<AccountId> {
	fn liquidate(
		who: &AccountId,
		currency_id: CurrencyId,
		amount: Balance,
		target_stable_amount: Balance,
	) -> DispatchResult;
}

#[impl_trait_for_tuples::impl_for_tuples(30)]
impl<AccountId> LiquidateCollateral<AccountId> for Tuple {
	fn liquidate(
		who: &AccountId,
		currency_id: CurrencyId,
		amount: Balance,
		target_stable_amount: Balance,
	) -> DispatchResult {
		let mut last_error = None;
		for_tuples!( #(
			match Tuple::liquidate(who, currency_id, amount, target_stable_amount) {
				Ok(_) => return Ok(()),
				Err(e) => { last_error = Some(e) }
			}
		)* );
		let last_error = last_error.unwrap_or(DispatchError::Other("No liquidation impl."));
		Err(last_error)
	}
}

pub trait BuyWeightRate {
	fn calculate_rate(location: Location) -> Option<Ratio>;
}
