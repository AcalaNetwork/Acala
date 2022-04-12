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

use codec::{Decode, Encode};
use frame_support::traits::Get;
use scale_info::TypeInfo;
use sp_core::H160;
use sp_runtime::{DispatchError, RuntimeDebug};
use sp_std::{cmp::PartialEq, prelude::*};

#[derive(RuntimeDebug, Encode, Decode, Clone, Copy, PartialEq, TypeInfo)]
pub enum SwapLimit<Balance> {
	/// use exact amount supply amount to swap. (exact_supply_amount, minimum_target_amount)
	ExactSupply(Balance, Balance),
	/// swap to get exact amount target. (maximum_supply_amount, exact_target_amount)
	ExactTarget(Balance, Balance),
}

pub trait DEXManager<AccountId, Balance, CurrencyId> {
	fn get_liquidity_pool(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> (Balance, Balance);

	fn get_liquidity_token_address(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> Option<H160>;

	fn get_swap_amount(path: &[CurrencyId], limit: SwapLimit<Balance>) -> Option<(Balance, Balance)>;

	fn get_best_price_swap_path(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
		alternative_path_joint_list: Vec<Vec<CurrencyId>>,
	) -> Option<(Vec<CurrencyId>, Balance, Balance)>;

	fn swap_with_specific_path(
		who: &AccountId,
		path: &[CurrencyId],
		limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError>;

	fn add_liquidity(
		who: &AccountId,
		currency_id_a: CurrencyId,
		currency_id_b: CurrencyId,
		max_amount_a: Balance,
		max_amount_b: Balance,
		min_share_increment: Balance,
		stake_increment_share: bool,
	) -> sp_std::result::Result<(Balance, Balance, Balance), DispatchError>;

	fn remove_liquidity(
		who: &AccountId,
		currency_id_a: CurrencyId,
		currency_id_b: CurrencyId,
		remove_share: Balance,
		min_withdrawn_a: Balance,
		min_withdrawn_b: Balance,
		by_unstake: bool,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError>;
}

pub trait Swap<AccountId, Balance, CurrencyId> {
	fn get_swap_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> Option<(Balance, Balance)>;

	fn swap(
		who: &AccountId,
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError>;
}

#[derive(Eq, PartialEq, RuntimeDebug)]
pub enum SwapError {
	CannotSwap,
}

impl Into<DispatchError> for SwapError {
	fn into(self) -> DispatchError {
		DispatchError::Other("Cannot swap")
	}
}

pub struct SpecificJointsSwap<Dex, Joints>(sp_std::marker::PhantomData<(Dex, Joints)>);

impl<AccountId, Balance, CurrencyId, Dex, Joints> Swap<AccountId, Balance, CurrencyId>
	for SpecificJointsSwap<Dex, Joints>
where
	Dex: DEXManager<AccountId, Balance, CurrencyId>,
	Joints: Get<Vec<Vec<CurrencyId>>>,
	Balance: Clone,
{
	fn get_swap_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> Option<(Balance, Balance)> {
		<Dex as DEXManager<AccountId, Balance, CurrencyId>>::get_best_price_swap_path(
			supply_currency_id,
			target_currency_id,
			limit,
			Joints::get(),
		)
		.map(|(_, supply_amount, target_amount)| (supply_amount, target_amount))
	}

	fn swap(
		who: &AccountId,
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		let path = <Dex as DEXManager<AccountId, Balance, CurrencyId>>::get_best_price_swap_path(
			supply_currency_id,
			target_currency_id,
			limit.clone(),
			Joints::get(),
		)
		.ok_or(Into::<DispatchError>::into(SwapError::CannotSwap))?
		.0;

		<Dex as DEXManager<AccountId, Balance, CurrencyId>>::swap_with_specific_path(who, &path, limit)
	}
}

#[macro_export]
macro_rules! create_aggregated_swap {
	($name:ident, $account_id:ty, $balance:ty, $currency_id:ty, [$( $provider:ty ),*]) => {
		pub struct $name;
		impl $crate::Swap<$account_id, $balance, $currency_id> for $name {
			fn get_swap_amount(
				supply_currency_id: $currency_id,
				target_currency_id: $currency_id,
				limit: $crate::SwapLimit<$balance>,
			) -> Option<($balance, $balance)> {
				let mut maybe_best = None;

				$(
					if let Some((supply_amount, target_amount)) = <$provider as $crate::Swap<$account_id, $balance, $currency_id>>::get_swap_amount(
						supply_currency_id,
						target_currency_id,
						limit,
					) {
						if let Some((previous_supply, previous_target)) = maybe_best {
							if supply_amount > previous_supply || target_amount < previous_target {
							// do nothing
							} else {
								maybe_best = Some((supply_amount, target_amount))
							}
						} else {
							maybe_best = Some((supply_amount, target_amount))
						}
					}
				)*

				maybe_best
			}

			fn swap(
				who: &$account_id,
				supply_currency_id: $currency_id,
				target_currency_id: $currency_id,
				limit: $crate::SwapLimit<$balance>,
			) -> sp_std::result::Result<($balance, $balance), sp_runtime::DispatchError> {
				let mut maybe_best: Option<(usize, $balance, $balance)> = None;
				let mut i: usize = 0;
				$(
					if let Some((supply_amount, target_amount)) = <$provider as $crate::Swap<$account_id, $balance, $currency_id>>::get_swap_amount(
						supply_currency_id,
						target_currency_id,
						limit,
					) {
						if let Some((_, previous_supply, previous_target)) = maybe_best {
							if supply_amount > previous_supply || target_amount < previous_target {
							// do nothing
							} else {
								maybe_best = Some((i, supply_amount, target_amount))
							}
						} else {
							maybe_best = Some((i, supply_amount, target_amount))
						}
					}

					i += 1;
				)*

				if let Some((best_index, _, _)) = maybe_best {
					let mut j = 0;
					$(
						if j == best_index {
							let response = <$provider as $crate::Swap<$account_id, $balance, $currency_id>>::swap(
								who,
								supply_currency_id,
								target_currency_id,
								limit,
							);

							return response;
						}

						j += 1;
					)*
				}

				Err(Into::<sp_runtime::DispatchError>::into($crate::SwapError::CannotSwap))
			}
    	}
	}
}

#[cfg(feature = "std")]
impl<AccountId, CurrencyId, Balance> DEXManager<AccountId, Balance, CurrencyId> for ()
where
	Balance: Default,
{
	fn get_liquidity_pool(_currency_id_a: CurrencyId, _currency_id_b: CurrencyId) -> (Balance, Balance) {
		Default::default()
	}

	fn get_liquidity_token_address(_currency_id_a: CurrencyId, _currency_id_b: CurrencyId) -> Option<H160> {
		Some(Default::default())
	}

	fn get_swap_amount(_path: &[CurrencyId], _limit: SwapLimit<Balance>) -> Option<(Balance, Balance)> {
		Some(Default::default())
	}

	fn get_best_price_swap_path(
		_supply_currency_id: CurrencyId,
		_target_currency_id: CurrencyId,
		_limit: SwapLimit<Balance>,
		_alternative_path_joint_list: Vec<Vec<CurrencyId>>,
	) -> Option<(Vec<CurrencyId>, Balance, Balance)> {
		Some(Default::default())
	}

	fn swap_with_specific_path(
		_who: &AccountId,
		_path: &[CurrencyId],
		_limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		Ok(Default::default())
	}

	fn add_liquidity(
		_who: &AccountId,
		_currency_id_a: CurrencyId,
		_currency_id_b: CurrencyId,
		_max_amount_a: Balance,
		_max_amount_b: Balance,
		_min_share_increment: Balance,
		_stake_increment_share: bool,
	) -> sp_std::result::Result<(Balance, Balance, Balance), DispatchError> {
		Ok(Default::default())
	}

	fn remove_liquidity(
		_who: &AccountId,
		_currency_id_a: CurrencyId,
		_currency_id_b: CurrencyId,
		_remove_share: Balance,
		_min_withdrawn_a: Balance,
		_min_withdrawn_b: Balance,
		_by_unstake: bool,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		Ok(Default::default())
	}
}
