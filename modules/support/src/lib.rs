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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::upper_case_acronyms)]

use codec::FullCodec;
use frame_support::pallet_prelude::{DispatchClass, Pays, Weight};
use primitives::{task::TaskResult, CurrencyId};
use sp_runtime::{
	traits::CheckedDiv, transaction_validity::TransactionValidityError, DispatchError, DispatchResult, FixedU128,
};
use sp_std::prelude::*;

use xcm::latest::prelude::*;

pub mod dex;
pub mod evm;
pub mod honzon;
pub mod mocks;

pub use crate::dex::*;
pub use crate::evm::*;
pub use crate::honzon::*;

pub type Price = FixedU128;
pub type ExchangeRate = FixedU128;
pub type Ratio = FixedU128;
pub type Rate = FixedU128;

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

pub trait DEXIncentives<AccountId, CurrencyId, Balance> {
	fn do_deposit_dex_share(who: &AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult;
	fn do_withdraw_dex_share(who: &AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult;
}

#[cfg(feature = "std")]
impl<AccountId, CurrencyId, Balance> DEXIncentives<AccountId, CurrencyId, Balance> for () {
	fn do_deposit_dex_share(_: &AccountId, _: CurrencyId, _: Balance) -> DispatchResult {
		Ok(())
	}

	fn do_withdraw_dex_share(_: &AccountId, _: CurrencyId, _: Balance) -> DispatchResult {
		Ok(())
	}
}

pub trait TransactionPayment<AccountId, Balance, NegativeImbalance> {
	fn reserve_fee(who: &AccountId, weight: Weight) -> Result<Balance, DispatchError>;
	fn unreserve_fee(who: &AccountId, fee: Balance);
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
}

#[cfg(feature = "std")]
use frame_support::traits::Imbalance;
#[cfg(feature = "std")]
impl<AccountId, Balance: Default + Copy, NegativeImbalance: Imbalance<Balance>>
	TransactionPayment<AccountId, Balance, NegativeImbalance> for ()
{
	fn reserve_fee(_who: &AccountId, _weight: Weight) -> Result<Balance, DispatchError> {
		Ok(Default::default())
	}

	fn unreserve_fee(_who: &AccountId, _fee: Balance) {}

	fn unreserve_and_charge_fee(
		_who: &AccountId,
		_weight: Weight,
	) -> Result<(Balance, NegativeImbalance), TransactionValidityError> {
		Ok((Default::default(), Imbalance::zero()))
	}

	fn refund_fee(
		_who: &AccountId,
		_weight: Weight,
		_payed: NegativeImbalance,
	) -> Result<(), TransactionValidityError> {
		Ok(())
	}

	fn charge_fee(
		_who: &AccountId,
		_len: u32,
		_weight: Weight,
		_tip: Balance,
		_pays_fee: Pays,
		_class: DispatchClass,
	) -> Result<(), TransactionValidityError> {
		Ok(())
	}
}

/// Used to interface with the Compound's Cash module
pub trait CompoundCashTrait<Balance, Moment> {
	fn set_future_yield(next_cash_yield: Balance, yield_index: u128, timestamp_effective: Moment) -> DispatchResult;
}

pub trait CallBuilder {
	type AccountId: FullCodec;
	type Balance: FullCodec;
	type RelayChainCall: FullCodec;

	/// Execute multiple calls in a batch.
	/// Param:
	/// - calls: List of calls to be executed
	fn utility_batch_call(calls: Vec<Self::RelayChainCall>) -> Self::RelayChainCall;

	/// Execute a call, replacing the `Origin` with a sub-account.
	///  params:
	/// - call: The call to be executed. Can be nested with `utility_batch_call`
	/// - index: The index of sub-account to be used as the new origin.
	fn utility_as_derivative_call(call: Self::RelayChainCall, index: u16) -> Self::RelayChainCall;

	/// Bond extra on relay-chain.
	///  params:
	/// - amount: The amount of staking currency to bond.
	fn staking_bond_extra(amount: Self::Balance) -> Self::RelayChainCall;

	/// Unbond on relay-chain.
	///  params:
	/// - amount: The amount of staking currency to unbond.
	fn staking_unbond(amount: Self::Balance) -> Self::RelayChainCall;

	/// Withdraw unbonded staking on the relay-chain.
	///  params:
	/// - num_slashing_spans: The number of slashing spans to withdraw from.
	fn staking_withdraw_unbonded(num_slashing_spans: u32) -> Self::RelayChainCall;

	/// Transfer Staking currency to another account, disallowing "death".
	///  params:
	/// - to: The destination for the transfer
	/// - amount: The amount of staking currency to be transferred.
	fn balances_transfer_keep_alive(to: Self::AccountId, amount: Self::Balance) -> Self::RelayChainCall;

	/// Wrap the final calls into the Xcm format.
	///  params:
	/// - call: The call to be executed
	/// - extra_fee: Extra fee (in staking currency) used for buy the `weight` and `debt`.
	/// - weight: the weight limit used for XCM.
	/// - debt: the weight limit used to process the `call`.
	fn finalize_call_into_xcm_message(call: Self::RelayChainCall, extra_fee: Self::Balance, weight: Weight) -> Xcm<()>;
}

/// Dispatchable tasks
pub trait DispatchableTask {
	fn dispatch(self, weight: Weight) -> TaskResult;
}

/// Idle scheduler trait
pub trait IdleScheduler<Task> {
	fn schedule(task: Task) -> DispatchResult;
}

#[cfg(feature = "std")]
impl DispatchableTask for () {
	fn dispatch(self, _weight: Weight) -> TaskResult {
		unimplemented!()
	}
}

#[cfg(feature = "std")]
impl<Task> IdleScheduler<Task> for () {
	fn schedule(_task: Task) -> DispatchResult {
		unimplemented!()
	}
}

#[impl_trait_for_tuples::impl_for_tuples(30)]
pub trait OnNewEra<EraIndex> {
	fn on_new_era(era: EraIndex);
}

pub trait NomineesProvider<AccountId> {
	fn nominees() -> Vec<AccountId>;
}

pub trait HomaSubAccountXcm<AccountId, Balance> {
	/// Cross-chain transfer staking currency to sub account on relaychain.
	fn transfer_staking_to_sub_account(sender: &AccountId, sub_account_index: u16, amount: Balance) -> DispatchResult;
	/// Send XCM message to the relaychain for sub account to withdraw_unbonded staking currency and
	/// send it back.
	fn withdraw_unbonded_from_sub_account(sub_account_index: u16, amount: Balance) -> DispatchResult;
	/// Send XCM message to the relaychain for sub account to bond extra.
	fn bond_extra_on_sub_account(sub_account_index: u16, amount: Balance) -> DispatchResult;
	/// Send XCM message to the relaychain for sub account to unbond.
	fn unbond_on_sub_account(sub_account_index: u16, amount: Balance) -> DispatchResult;
	/// The fee of cross-chain transfer is deducted from the recipient.
	fn get_xcm_transfer_fee() -> Balance;
	/// The fee of parachain
	fn get_parachain_fee(location: MultiLocation) -> Balance;
}
