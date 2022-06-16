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

use super::{
	input::{Input, InputT, Output},
	target_gas_limit,
};
use frame_support::{
	log,
	traits::tokens::nonfungibles::{Inspect, Transfer},
};
use module_evm::{
	precompiles::Precompile,
	runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult},
	Context, ExitError, ExitRevert, ExitSucceed,
};
use module_support::AddressMapping;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use orml_traits::InspectExtended;
use primitives::nft::NFTBalance;
use sp_core::H160;
use sp_runtime::RuntimeDebug;
use sp_std::{marker::PhantomData, prelude::*};

/// The `NFT` impl precompile.
///
/// `input` data starts with `action`.
///
/// Actions:
/// - Query balance. Rest `input` bytes: `account_id`.
/// - Query owner. Rest `input` bytes: `class_id`, `token_id`.
/// - Transfer. Rest `input`bytes: `from`, `to`, `class_id`, `token_id`.
pub struct NFTPrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	QueryBalance = "balanceOf(address)",
	QueryOwner = "ownerOf(uint256,uint256)",
	Transfer = "transfer(address,address,uint256,uint256)",
}

impl<Runtime> Precompile for NFTPrecompile<Runtime>
where
	Runtime: module_evm::Config + module_prices::Config + module_nft::Config,
	module_nft::Pallet<Runtime>: InspectExtended<Runtime::AccountId, Balance = NFTBalance>
		+ Inspect<Runtime::AccountId, ItemId = u64, CollectionId = u32>
		+ Transfer<Runtime::AccountId>,
{
	fn execute(input: &[u8], target_gas: Option<u64>, _context: &Context, _is_static: bool) -> PrecompileResult {
		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(
			input,
			target_gas_limit(target_gas),
		);

		let gas_cost = Pricer::<Runtime>::cost(&input)?;

		if let Some(gas_limit) = target_gas {
			if gas_limit < gas_cost {
				return Err(PrecompileFailure::Error {
					exit_status: ExitError::OutOfGas,
				});
			}
		}

		let action = input.action()?;

		match action {
			Action::QueryBalance => {
				let who = input.account_id_at(1)?;

				log::debug!(target: "evm", "nft: query_balance who: {:?}", who);

				let balance = module_nft::Pallet::<Runtime>::balance(&who);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: Output::encode_uint(balance),
					logs: Default::default(),
				})
			}
			Action::QueryOwner => {
				let class_id = input.u32_at(1)?;
				let token_id = input.u64_at(2)?;

				log::debug!(target: "evm", "nft: query_owner class_id: {:?}, token_id: {:?}", class_id, token_id);

				let owner: H160 = if let Some(o) = module_nft::Pallet::<Runtime>::owner(&class_id, &token_id) {
					Runtime::AddressMapping::get_evm_address(&o)
						.unwrap_or_else(|| Runtime::AddressMapping::get_default_evm_address(&o))
				} else {
					Default::default()
				};

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: Output::encode_address(owner),
					logs: Default::default(),
				})
			}
			Action::Transfer => {
				let from = input.account_id_at(1)?;
				let to = input.account_id_at(2)?;

				let class_id = input.u32_at(3)?;
				let token_id = input.u64_at(4)?;

				log::debug!(target: "evm", "nft: transfer from: {:?}, to: {:?}, class_id: {:?}, token_id: {:?}", from, to, class_id, token_id);

				<module_nft::Pallet<Runtime> as Transfer<Runtime::AccountId>>::transfer(&class_id, &token_id, &to)
					.map_err(|e| PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Into::<&str>::into(e).as_bytes().to_vec(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: vec![],
					logs: Default::default(),
				})
			}
		}
	}
}

pub struct Pricer<R>(PhantomData<R>);

impl<Runtime> Pricer<Runtime>
where
	Runtime: module_evm::Config + module_prices::Config + module_nft::Config,
{
	pub const BASE_COST: u64 = 200;

	fn cost(
		input: &Input<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>,
	) -> Result<u64, PrecompileFailure> {
		let _action = input.action()?;
		// TODO: gas cost
		Ok(Self::BASE_COST)
	}
}
