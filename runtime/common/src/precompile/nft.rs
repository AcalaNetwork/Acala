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

use crate::precompile::PrecompileOutput;
use frame_support::log;
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use module_support::{AddressMapping as AddressMappingT, Erc20InfoMapping as Erc20InfoMappingT};
use sp_core::H160;
use sp_runtime::RuntimeDebug;
use sp_std::{borrow::Cow, fmt::Debug, marker::PhantomData, prelude::*, result};

use orml_traits::NFT as NFTT;

use super::input::{Input, InputT, Output};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use primitives::NFTBalance;

/// The `NFT` impl precompile.
///
/// `input` data starts with `action`.
///
/// Actions:
/// - Query balance. Rest `input` bytes: `account_id`.
/// - Query owner. Rest `input` bytes: `class_id`, `token_id`.
/// - Transfer. Rest `input`bytes: `from`, `to`, `class_id`, `token_id`.
pub struct NFTPrecompile<AccountId, AddressMapping, Erc20InfoMapping, NFT>(
	PhantomData<(AccountId, AddressMapping, Erc20InfoMapping, NFT)>,
);

#[module_evm_utiltity_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	QueryBalance = "balanceOf(address)",
	QueryOwner = "ownerOf(uint256,uint256)",
	Transfer = "transfer(address,address,uint256,uint256)",
}

impl<AccountId, AddressMapping, Erc20InfoMapping, NFT> Precompile
	for NFTPrecompile<AccountId, AddressMapping, Erc20InfoMapping, NFT>
where
	AccountId: Clone + Debug,
	AddressMapping: AddressMappingT<AccountId>,
	Erc20InfoMapping: Erc20InfoMappingT,
	NFT: NFTT<AccountId, Balance = NFTBalance, ClassId = u32, TokenId = u64>,
{
	fn execute(
		input: &[u8],
		_target_gas: Option<u64>,
		_context: &Context,
	) -> result::Result<PrecompileOutput, ExitError> {
		let input = Input::<Action, AccountId, AddressMapping, Erc20InfoMapping>::new(input);

		let action = input.action()?;

		match action {
			Action::QueryBalance => {
				let who = input.account_id_at(1)?;

				log::debug!(target: "evm", "nft: query_balance who: {:?}", who);

				let balance = NFT::balance(&who);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: Output::default().encode_u128(balance),
					logs: Default::default(),
				})
			}
			Action::QueryOwner => {
				let class_id = input.u32_at(1)?;
				let token_id = input.u64_at(2)?;

				log::debug!(target: "evm", "nft: query_owner class_id: {:?}, token_id: {:?}", class_id, token_id);

				let owner: H160 = if let Some(o) = NFT::owner((class_id, token_id)) {
					AddressMapping::get_evm_address(&o).unwrap_or_else(|| AddressMapping::get_default_evm_address(&o))
				} else {
					Default::default()
				};

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: Output::default().encode_address(&owner),
					logs: Default::default(),
				})
			}
			Action::Transfer => {
				let from = input.account_id_at(1)?;
				let to = input.account_id_at(2)?;

				let class_id = input.u32_at(3)?;
				let token_id = input.u64_at(4)?;

				log::debug!(target: "evm", "nft: transfer from: {:?}, to: {:?}, class_id: {:?}, token_id: {:?}", from, to, class_id, token_id);

				NFT::transfer(&from, &to, (class_id, token_id))
					.map_err(|e| ExitError::Other(Cow::Borrowed(e.into())))?;

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
