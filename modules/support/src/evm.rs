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

use parity_scale_codec::{Decode, Encode};
use primitives::currency::AssetIds;
use primitives::{
	evm::{CallInfo, EvmAddress},
	Balance, CurrencyId,
};
use sp_core::H160;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, MaybeSerializeDeserialize},
	DispatchError, DispatchResult, RuntimeDebug,
};
use sp_std::{
	cmp::{Eq, PartialEq},
	prelude::*,
};

/// Return true if the call of EVM precompile contract is allowed.
pub trait PrecompileCallerFilter {
	fn is_allowed(caller: H160) -> bool;
}

/// Return true if the EVM precompile is paused.
pub trait PrecompilePauseFilter {
	fn is_paused(address: H160) -> bool;
}

/// An abstraction of EVM for EVMBridge
pub trait EVM<AccountId> {
	type Balance: AtLeast32BitUnsigned + Copy + MaybeSerializeDeserialize + Default;

	fn execute(
		context: InvokeContext,
		input: Vec<u8>,
		value: Self::Balance,
		gas_limit: u64,
		storage_limit: u32,
		mode: ExecutionMode,
	) -> Result<CallInfo, sp_runtime::DispatchError>;

	/// Get the real origin account and charge storage rent from the origin.
	fn get_origin() -> Option<AccountId>;
	/// Set the EVM origin
	fn set_origin(origin: AccountId);
	/// Kill the EVM origin
	fn kill_origin();
	/// Push new EVM origin in xcm
	fn push_xcm_origin(origin: AccountId);
	/// Pop EVM origin in xcm
	fn pop_xcm_origin();
	/// Kill the EVM origin in xcm
	fn kill_xcm_origin();
	/// Get the real origin account or xcm origin and charge storage rent from the origin.
	fn get_real_or_xcm_origin() -> Option<AccountId>;
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug)]
pub enum ExecutionMode {
	Execute,
	/// Discard any state changes
	View,
	/// Also discard any state changes and use estimate gas mode for evm config
	EstimateGas,
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug)]
pub struct InvokeContext {
	pub contract: EvmAddress,
	/// similar to msg.sender
	pub sender: EvmAddress,
	/// similar to tx.origin
	pub origin: EvmAddress,
}

/// An abstraction of EVMBridge
pub trait EVMBridge<AccountId, Balance> {
	/// Execute ERC20.name() to read token name from ERC20 contract
	fn name(context: InvokeContext) -> Result<Vec<u8>, DispatchError>;
	/// Execute ERC20.symbol() to read token symbol from ERC20 contract
	fn symbol(context: InvokeContext) -> Result<Vec<u8>, DispatchError>;
	/// Execute ERC20.decimals() to read token decimals from ERC20 contract
	fn decimals(context: InvokeContext) -> Result<u8, DispatchError>;
	/// Execute ERC20.totalSupply() to read total supply from ERC20 contract
	fn total_supply(context: InvokeContext) -> Result<Balance, DispatchError>;
	/// Execute ERC20.balanceOf(address) to read balance of address from ERC20
	/// contract
	fn balance_of(context: InvokeContext, address: EvmAddress) -> Result<Balance, DispatchError>;
	/// Execute ERC20.transfer(address, uint256) to transfer value to `to`
	fn transfer(context: InvokeContext, to: EvmAddress, value: Balance) -> DispatchResult;
	/// Get the real origin account and charge storage rent from the origin.
	fn get_origin() -> Option<AccountId>;
	/// Set the EVM origin
	fn set_origin(origin: AccountId);
	/// Kill the EVM origin
	fn kill_origin();
	/// Push new EVM origin in xcm
	fn push_xcm_origin(origin: AccountId);
	/// Pop EVM origin in xcm
	fn pop_xcm_origin();
	/// Kill the EVM origin in xcm
	fn kill_xcm_origin();
	/// Get the real origin account or xcm origin and charge storage rent from the origin.
	fn get_real_or_xcm_origin() -> Option<AccountId>;
}

#[cfg(feature = "std")]
impl<AccountId, Balance: Default> EVMBridge<AccountId, Balance> for () {
	fn name(_context: InvokeContext) -> Result<Vec<u8>, DispatchError> {
		Err(DispatchError::Other("unimplemented evm bridge"))
	}
	fn symbol(_context: InvokeContext) -> Result<Vec<u8>, DispatchError> {
		Err(DispatchError::Other("unimplemented evm bridge"))
	}
	fn decimals(_context: InvokeContext) -> Result<u8, DispatchError> {
		Err(DispatchError::Other("unimplemented evm bridge"))
	}
	fn total_supply(_context: InvokeContext) -> Result<Balance, DispatchError> {
		Err(DispatchError::Other("unimplemented evm bridge"))
	}
	fn balance_of(_context: InvokeContext, _address: EvmAddress) -> Result<Balance, DispatchError> {
		Err(DispatchError::Other("unimplemented evm bridge"))
	}
	fn transfer(_context: InvokeContext, _to: EvmAddress, _value: Balance) -> DispatchResult {
		Err(DispatchError::Other("unimplemented evm bridge"))
	}
	fn get_origin() -> Option<AccountId> {
		None
	}
	fn set_origin(_origin: AccountId) {}
	fn kill_origin() {}
	fn push_xcm_origin(_origin: AccountId) {}
	fn pop_xcm_origin() {}
	fn kill_xcm_origin() {}
	fn get_real_or_xcm_origin() -> Option<AccountId> {
		None
	}
}

/// EVM bridge for collateral liquidation.
pub trait LiquidationEvmBridge {
	/// Execute liquidation. Sufficient repayment is expected to be transferred to `repay_dest`,
	/// if not received or below `min_repayment`, the liquidation would be seen as failed.
	fn liquidate(
		context: InvokeContext,
		collateral: EvmAddress,
		repay_dest: EvmAddress,
		amount: Balance,
		min_repayment: Balance,
	) -> DispatchResult;
	/// Called on sufficient repayment received and collateral transferred to liquidation contract.
	fn on_collateral_transfer(context: InvokeContext, collateral: EvmAddress, amount: Balance);
	/// Called on insufficient repayment received and repayment refunded to liquidation contract.
	fn on_repayment_refund(context: InvokeContext, collateral: EvmAddress, repayment: Balance);
}
impl LiquidationEvmBridge for () {
	fn liquidate(
		_context: InvokeContext,
		_collateral: EvmAddress,
		_repay_dest: EvmAddress,
		_amount: Balance,
		_min_repayment: Balance,
	) -> DispatchResult {
		Err(DispatchError::Other("unimplemented evm bridge"))
	}
	fn on_collateral_transfer(_context: InvokeContext, _collateral: EvmAddress, _amount: Balance) {}
	fn on_repayment_refund(_context: InvokeContext, _collateral: EvmAddress, _repayment: Balance) {}
}

/// An abstraction of EVMManager
pub trait EVMManager<AccountId, Balance> {
	/// Query the constants `NewContractExtraBytes` value from evm module.
	fn query_new_contract_extra_bytes() -> u32;
	/// Query the constants `StorageDepositPerByte` value from evm module.
	fn query_storage_deposit_per_byte() -> Balance;
	/// Query the maintainer address from the ERC20 contract.
	fn query_maintainer(contract: &H160) -> Result<H160, DispatchError>;
	/// Query the constants `DeveloperDeposit` value from evm module.
	fn query_developer_deposit() -> Balance;
	/// Query the constants `PublicationFee` value from evm module.
	fn query_publication_fee() -> Balance;
	/// Transfer the maintainer of the contract address.
	fn transfer_maintainer(from: AccountId, contract: H160, new_maintainer: H160) -> DispatchResult;
	/// Publish contract
	fn publish_contract_precompile(who: AccountId, contract: H160) -> DispatchResult;
	/// Query the developer status of an account
	fn query_developer_status(who: &AccountId) -> bool;
	/// Enable developer mode
	fn enable_account_contract_development(who: &AccountId) -> DispatchResult;
	/// Disable developer mode
	fn disable_account_contract_development(who: &AccountId) -> DispatchResult;
}

/// An abstraction of EVMAccountsManager
pub trait EVMAccountsManager<AccountId> {
	/// Returns the AccountId used to generate the given EvmAddress.
	fn get_account_id(address: &EvmAddress) -> AccountId;
	/// Returns the EvmAddress associated with a given AccountId or the underlying EvmAddress of the
	/// AccountId.
	fn get_evm_address(account_id: &AccountId) -> Option<EvmAddress>;
	/// Claim account mapping between AccountId and a generated EvmAddress based off of the
	/// AccountId.
	fn claim_default_evm_address(account_id: &AccountId) -> Result<EvmAddress, DispatchError>;
}

/// A mapping between `AccountId` and `EvmAddress`.
pub trait AddressMapping<AccountId> {
	/// Returns the AccountId used go generate the given EvmAddress.
	fn get_account_id(evm: &EvmAddress) -> AccountId;
	/// Returns the EvmAddress associated with a given AccountId or the
	/// underlying EvmAddress of the AccountId.
	/// Returns None if there is no EvmAddress associated with the AccountId
	/// and there is no underlying EvmAddress in the AccountId.
	fn get_evm_address(account_id: &AccountId) -> Option<EvmAddress>;
	/// Returns the EVM address associated with an account ID and generates an
	/// account mapping if no association exists.
	fn get_or_create_evm_address(account_id: &AccountId) -> EvmAddress;
	/// Returns the default EVM address associated with an account ID.
	fn get_default_evm_address(account_id: &AccountId) -> EvmAddress;
	/// Returns true if a given AccountId is associated with a given EvmAddress
	/// and false if is not.
	fn is_linked(account_id: &AccountId, evm: &EvmAddress) -> bool;
}

/// A mapping between AssetId and AssetMetadata.
pub trait AssetIdMapping<ForeignAssetId, Location, AssetMetadata> {
	/// Returns the AssetMetadata associated with a given `AssetIds`.
	fn get_asset_metadata(asset_ids: AssetIds) -> Option<AssetMetadata>;
	/// Returns the MultiLocation associated with a given ForeignAssetId.
	fn get_location(foreign_asset_id: ForeignAssetId) -> Option<Location>;
	/// Returns the CurrencyId associated with a given MultiLocation.
	fn get_currency_id(location: Location) -> Option<CurrencyId>;
}

/// A mapping between u32 and Erc20 address.
/// provide a way to encode/decode for CurrencyId;
pub trait Erc20InfoMapping {
	/// Returns the name associated with a given CurrencyId.
	/// If CurrencyId is CurrencyId::DexShare and contain DexShare::Erc20,
	/// the EvmAddress must have been mapped.
	fn name(currency_id: CurrencyId) -> Option<Vec<u8>>;
	/// Returns the symbol associated with a given CurrencyId.
	/// If CurrencyId is CurrencyId::DexShare and contain DexShare::Erc20,
	/// the EvmAddress must have been mapped.
	fn symbol(currency_id: CurrencyId) -> Option<Vec<u8>>;
	/// Returns the decimals associated with a given CurrencyId.
	/// If CurrencyId is CurrencyId::DexShare and contain DexShare::Erc20,
	/// the EvmAddress must have been mapped.
	fn decimals(currency_id: CurrencyId) -> Option<u8>;
	/// Encode the CurrencyId to EvmAddress.
	/// If is CurrencyId::DexShare and contain DexShare::Erc20,
	/// will use the u32 to get the DexShare::Erc20 from the mapping.
	fn encode_evm_address(v: CurrencyId) -> Option<EvmAddress>;
	/// Decode the CurrencyId from EvmAddress.
	/// If is CurrencyId::DexShare and contain DexShare::Erc20,
	/// will use the u32 to get the DexShare::Erc20 from the mapping.
	fn decode_evm_address(v: EvmAddress) -> Option<CurrencyId>;
}

#[cfg(feature = "std")]
impl Erc20InfoMapping for () {
	fn name(_currency_id: CurrencyId) -> Option<Vec<u8>> {
		None
	}

	fn symbol(_currency_id: CurrencyId) -> Option<Vec<u8>> {
		None
	}

	fn decimals(_currency_id: CurrencyId) -> Option<u8> {
		None
	}

	fn encode_evm_address(_v: CurrencyId) -> Option<EvmAddress> {
		None
	}

	fn decode_evm_address(_v: EvmAddress) -> Option<CurrencyId> {
		None
	}
}

pub mod limits {
	pub struct Limit {
		pub gas: u64,
		pub storage: u32,
	}

	impl Limit {
		pub const fn new(gas: u64, storage: u32) -> Self {
			Self { gas, storage }
		}
	}

	pub mod erc20 {
		use super::*;

		pub const NAME: Limit = Limit::new(100_000, 0);
		pub const SYMBOL: Limit = Limit::new(100_000, 0);
		pub const DECIMALS: Limit = Limit::new(100_000, 0);
		pub const TOTAL_SUPPLY: Limit = Limit::new(100_000, 0);
		pub const BALANCE_OF: Limit = Limit::new(100_000, 0);
		pub const TRANSFER: Limit = Limit::new(200_000, 960);
	}

	pub mod liquidation {
		use super::*;

		pub const LIQUIDATE: Limit = Limit::new(200_000, 1_000);
		pub const ON_COLLATERAL_TRANSFER: Limit = Limit::new(200_000, 1_000);
		pub const ON_REPAYMENT_REFUND: Limit = Limit::new(200_000, 1_000);
	}
}
