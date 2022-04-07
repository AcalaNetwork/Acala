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

#![allow(clippy::type_complexity)]
use crate::{AddressMapping, CurrencyId, Erc20InfoMapping};
use codec::Encode;
use nutsfinance_stable_asset::traits::StableAsset;
use nutsfinance_stable_asset::{PoolTokenIndex, RedeemProportionResult, StableAssetPoolId, StableAssetPoolInfo};
use primitives::{
	currency::TokenInfo,
	evm::{EvmAddress, H160_POSITION_TOKEN},
};
use sp_core::{crypto::AccountId32, H160};
use sp_io::hashing::blake2_256;
use sp_runtime::DispatchResult;
use sp_std::vec::Vec;

pub struct MockAddressMapping;

impl AddressMapping<AccountId32> for MockAddressMapping {
	fn get_account_id(address: &H160) -> AccountId32 {
		let mut data = [0u8; 32];
		data[0..4].copy_from_slice(b"evm:");
		data[4..24].copy_from_slice(&address[..]);
		AccountId32::from(data)
	}

	fn get_evm_address(account_id: &AccountId32) -> Option<H160> {
		let data: [u8; 32] = account_id.clone().into();
		if data.starts_with(b"evm:") {
			Some(H160::from_slice(&data[4..24]))
		} else {
			None
		}
	}

	fn get_default_evm_address(account_id: &AccountId32) -> H160 {
		let slice: &[u8] = account_id.as_ref();
		H160::from_slice(&slice[0..20])
	}

	fn get_or_create_evm_address(account_id: &AccountId32) -> H160 {
		Self::get_evm_address(account_id).unwrap_or({
			let payload = (b"evm:", account_id);
			H160::from_slice(&payload.using_encoded(blake2_256)[0..20])
		})
	}

	fn is_linked(account_id: &AccountId32, evm: &H160) -> bool {
		Self::get_or_create_evm_address(account_id) == *evm
	}
}

pub struct MockErc20InfoMapping;

impl Erc20InfoMapping for MockErc20InfoMapping {
	fn name(currency_id: CurrencyId) -> Option<Vec<u8>> {
		currency_id.name().map(|v| v.as_bytes().to_vec())
	}

	fn symbol(currency_id: CurrencyId) -> Option<Vec<u8>> {
		currency_id.symbol().map(|v| v.as_bytes().to_vec())
	}

	fn decimals(currency_id: CurrencyId) -> Option<u8> {
		currency_id.decimals()
	}

	fn encode_evm_address(v: CurrencyId) -> Option<EvmAddress> {
		EvmAddress::try_from(v).ok()
	}

	fn decode_evm_address(v: EvmAddress) -> Option<CurrencyId> {
		let token = v.as_bytes()[H160_POSITION_TOKEN]
			.try_into()
			.map(CurrencyId::Token)
			.ok()?;
		EvmAddress::try_from(token)
			.map(|addr| if addr == v { Some(token) } else { None })
			.ok()?
	}
}

pub struct MockStableAsset<CurrencyId, Balance, AccountId, BlockNumber> {
	_value1: CurrencyId,
	_value2: Balance,
	_value3: AccountId,
	_value4: BlockNumber,
}
impl<CurrencyId, Balance, AccountId, BlockNumber> StableAsset
	for MockStableAsset<CurrencyId, Balance, AccountId, BlockNumber>
{
	type AssetId = CurrencyId;
	type AtLeast64BitUnsigned = Balance;
	type Balance = Balance;
	type AccountId = AccountId;
	type BlockNumber = BlockNumber;

	fn pool_count() -> StableAssetPoolId {
		unimplemented!()
	}

	fn pool(
		_id: StableAssetPoolId,
	) -> Option<StableAssetPoolInfo<Self::AssetId, Self::Balance, Self::Balance, Self::AccountId, Self::BlockNumber>> {
		unimplemented!()
	}

	fn create_pool(
		_pool_asset: Self::AssetId,
		_assets: Vec<Self::AssetId>,
		_precisions: Vec<Self::Balance>,
		_mint_fee: Self::Balance,
		_swap_fee: Self::Balance,
		_redeem_fee: Self::Balance,
		_initial_a: Self::Balance,
		_fee_recipient: Self::AccountId,
		_yield_recipient: Self::AccountId,
		_precision: Self::Balance,
	) -> DispatchResult {
		unimplemented!()
	}

	fn mint(
		_who: &Self::AccountId,
		_pool_id: StableAssetPoolId,
		_amounts: Vec<Self::Balance>,
		_min_mint_amount: Self::Balance,
	) -> DispatchResult {
		unimplemented!()
	}

	fn swap(
		_who: &Self::AccountId,
		_pool_id: StableAssetPoolId,
		_i: PoolTokenIndex,
		_j: PoolTokenIndex,
		_dx: Self::Balance,
		_min_dy: Self::Balance,
		_asset_length: u32,
	) -> DispatchResult {
		unimplemented!()
	}

	fn redeem_proportion(
		_who: &Self::AccountId,
		_pool_id: StableAssetPoolId,
		_amount: Self::Balance,
		_min_redeem_amounts: Vec<Self::Balance>,
	) -> DispatchResult {
		unimplemented!()
	}

	fn redeem_single(
		_who: &Self::AccountId,
		_pool_id: StableAssetPoolId,
		_amount: Self::Balance,
		_i: PoolTokenIndex,
		_min_redeem_amount: Self::Balance,
		_asset_length: u32,
	) -> DispatchResult {
		unimplemented!()
	}

	fn redeem_multi(
		_who: &Self::AccountId,
		_pool_id: StableAssetPoolId,
		_amounts: Vec<Self::Balance>,
		_max_redeem_amount: Self::Balance,
	) -> DispatchResult {
		unimplemented!()
	}

	fn collect_fee(
		_pool_id: StableAssetPoolId,
		_pool_info: &mut StableAssetPoolInfo<
			Self::AssetId,
			Self::Balance,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	) -> DispatchResult {
		unimplemented!()
	}

	fn update_balance(
		_pool_id: StableAssetPoolId,
		_pool_info: &mut StableAssetPoolInfo<
			Self::AssetId,
			Self::Balance,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	) -> DispatchResult {
		unimplemented!()
	}

	fn collect_yield(
		_pool_id: StableAssetPoolId,
		_pool_info: &mut StableAssetPoolInfo<
			Self::AssetId,
			Self::Balance,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	) -> DispatchResult {
		unimplemented!()
	}

	fn modify_a(_pool_id: StableAssetPoolId, _a: Self::Balance, _future_a_block: Self::BlockNumber) -> DispatchResult {
		unimplemented!()
	}

	fn get_collect_yield_amount(
		_pool_info: &StableAssetPoolInfo<
			Self::AssetId,
			Self::Balance,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	) -> Option<StableAssetPoolInfo<Self::AssetId, Self::Balance, Self::Balance, Self::AccountId, Self::BlockNumber>> {
		unimplemented!()
	}

	fn get_balance_update_amount(
		_pool_info: &StableAssetPoolInfo<
			Self::AssetId,
			Self::Balance,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	) -> Option<StableAssetPoolInfo<Self::AssetId, Self::Balance, Self::Balance, Self::AccountId, Self::BlockNumber>> {
		unimplemented!()
	}

	fn get_redeem_proportion_amount(
		_pool_info: &StableAssetPoolInfo<
			Self::AssetId,
			Self::Balance,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
		_amount_bal: Self::Balance,
	) -> Option<RedeemProportionResult<Self::Balance>> {
		unimplemented!()
	}
}
