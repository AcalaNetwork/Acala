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

use crate::{
	currency::{CurrencyId, CurrencyIdType, DexShareType},
	Balance, BlockNumber, Nonce,
};
use core::ops::Range;
use hex_literal::hex;
pub use module_evm_utility::{
	ethereum::{AccessListItem, Log, TransactionAction},
	evm::ExitReason,
};
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::{H160, H256, U256};
use sp_runtime::{traits::Zero, RuntimeDebug, SaturatedConversion};
use sp_std::vec::Vec;

/// Evm Address.
pub type EvmAddress = sp_core::H160;

/// mandala 595
pub const CHAIN_ID_MANDALA: u64 = 595u64;
/// karura testnet 596
pub const CHAIN_ID_KARURA_TESTNET: u64 = 596u64;
/// acala testnet 597
pub const CHAIN_ID_ACALA_TESTNET: u64 = 597u64;
/// karura mainnet 686
pub const CHAIN_ID_KARURA_MAINNET: u64 = 686u64;
/// acala mainnet 787
pub const CHAIN_ID_ACALA_MAINNET: u64 = 787u64;

// GAS MASK
const GAS_MASK: u64 = 100_000u64;
// STORAGE MASK
const STORAGE_MASK: u64 = 100u64;
// GAS LIMIT CHUNK
const GAS_LIMIT_CHUNK: u64 = 30_000u64;
// MAX GAS_LIMIT CC, log2(BLOCK_STORAGE_LIMIT)
pub const MAX_GAS_LIMIT_CC: u32 = 22u32;

#[derive(Clone, Eq, PartialEq, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
/// External input from the transaction.
pub struct Vicinity {
	/// Current transaction gas price.
	pub gas_price: U256,
	/// Origin of the transaction.
	pub origin: EvmAddress,
	/// Environmental coinbase.
	pub block_coinbase: Option<EvmAddress>,
	/// Environmental block gas limit. Used only for testing
	pub block_gas_limit: Option<U256>,
	/// Environmental block difficulty. Used only for testing
	pub block_difficulty: Option<U256>,
	/// Environmental base fee per gas.
	pub block_base_fee_per_gas: Option<U256>,
	/// Environmental randomness.
	pub block_randomness: Option<H256>,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct ExecutionInfo<T> {
	pub exit_reason: ExitReason,
	pub value: T,
	pub used_gas: U256,
	pub used_storage: i32,
	pub logs: Vec<Log>,
}

pub type CallInfo = ExecutionInfo<Vec<u8>>;
pub type CreateInfo = ExecutionInfo<H160>;

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct BlockLimits {
	/// Max gas limit
	pub max_gas_limit: u64,
	/// Max storage limit
	pub max_storage_limit: u32,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct EstimateResourcesRequest {
	/// From
	pub from: Option<H160>,
	/// To
	pub to: Option<H160>,
	/// Gas Limit
	pub gas_limit: Option<u64>,
	/// Storage Limit
	pub storage_limit: Option<u32>,
	/// Value
	pub value: Option<Balance>,
	/// Data
	pub data: Option<Vec<u8>>,
	/// AccessList
	pub access_list: Option<Vec<AccessListItem>>,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct EthereumTransactionMessage {
	pub chain_id: u64,
	pub genesis: H256,
	pub nonce: Nonce,
	pub tip: Balance,
	pub gas_price: u64,
	pub gas_limit: u64,
	pub storage_limit: u32,
	pub action: TransactionAction,
	pub value: Balance,
	pub input: Vec<u8>,
	pub valid_until: BlockNumber,
	pub access_list: Vec<AccessListItem>,
}

/// Ethereum precompiles
/// 0 - 0x0000000000000000000000000000000000000400
/// Acala precompiles
/// 0x0000000000000000000000000000000000000400 - 0x0000000000000000000000000000000000000800
pub const PRECOMPILE_ADDRESS_START: EvmAddress = H160(hex!("0000000000000000000000000000000000000400"));
/// Predeployed system contracts (except Mirrored ERC20)
/// 0x0000000000000000000000000000000000000800 - 0x0000000000000000000000000000000000001000
pub const PREDEPLOY_ADDRESS_START: EvmAddress = H160(hex!("0000000000000000000000000000000000000800"));
pub const MIRRORED_TOKENS_ADDRESS_START: EvmAddress = H160(hex!("0000000000000000000100000000000000000000"));
pub const MIRRORED_NFT_ADDRESS_START: u64 = 0x2000000;
/// ERC20 Holding Account used for transfer ERC20 token
pub const ERC20_HOLDING_ACCOUNT: EvmAddress = H160(hex_literal::hex!("000000000000000000ff00000000000000000000"));
/// System contract address prefix
pub const SYSTEM_CONTRACT_ADDRESS_PREFIX: [u8; 9] = [0u8; 9];

#[rustfmt::skip]
/// CurrencyId to H160([u8; 20]) bit encoding rule.
///
/// Type occupies 1 byte, and data occupies 4 bytes(less than 4 bytes, right justified).
///
/// 0x0000000000000000000000000000000000000000
///    0 1 2 3 4 5 6 7 8 910111213141516171819 index
///   ^^^^^^^^^^^^^^^^^^                       System contract address prefix
///                     ^^                     CurrencyId Type: 1-Token 2-DexShare 3-StableAsset
///                                                             4-LiquidCrowdloan
///                                                             5-ForeignAsset(ignore Erc20, without the prefix of system contracts)
///                                                             FF-Erc20 Holding Account
///                                         ^^ CurrencyId Type is 1-Token, Token
///                                   ^^^^^^^^ CurrencyId Type is 1-Token, NFT
///                       ^^                   CurrencyId Type is 2-DexShare, DexShare Left Type:
///                                                             0-Token 1-Erc20 2-LiquidCrowdloan 3-ForeignAsset 4-StableAsset
///                         ^^^^^^^^           CurrencyId Type is 2-DexShare, DexShare left field
///                                 ^^         CurrencyId Type is 2-DexShare, DexShare Right Type:
///                                                             the same as DexShare Left Type
///                                   ^^^^^^^^ CurrencyId Type is 2-DexShare, DexShare right field
///                                   ^^^^^^^^ CurrencyId Type is 3-StableAsset, StableAssetPoolId
///                                   ^^^^^^^^ CurrencyId Type is 4-LiquidCrowdloan, Lease
///                                       ^^^^ CurrencyId Type is 5-ForeignAsset, ForeignAssetId

/// Check if the given `address` is a system contract.
///
/// It's system contract if the address starts with SYSTEM_CONTRACT_ADDRESS_PREFIX.
pub fn is_system_contract(address: &EvmAddress) -> bool {
	address.as_bytes().starts_with(&SYSTEM_CONTRACT_ADDRESS_PREFIX)
}

pub const H160_POSITION_CURRENCY_ID_TYPE: usize = 9;
pub const H160_POSITION_TOKEN: usize = 19;
pub const H160_POSITION_TOKEN_NFT: Range<usize> = 16..20;
pub const H160_POSITION_DEXSHARE_LEFT_TYPE: usize = 10;
pub const H160_POSITION_DEXSHARE_LEFT_FIELD: Range<usize> = 11..15;
pub const H160_POSITION_DEXSHARE_RIGHT_TYPE: usize = 15;
pub const H160_POSITION_DEXSHARE_RIGHT_FIELD: Range<usize> = 16..20;
pub const H160_POSITION_STABLE_ASSET: Range<usize> = 16..20;
pub const H160_POSITION_LIQUID_CROADLOAN: Range<usize> = 16..20;
pub const H160_POSITION_FOREIGN_ASSET: Range<usize> = 18..20;

/// Generate the EvmAddress from CurrencyId so that evm contracts can call the erc20 contract.
/// NOTE: Can not be used directly, need to check the erc20 is mapped.
impl TryFrom<CurrencyId> for EvmAddress {
	type Error = ();

	fn try_from(val: CurrencyId) -> Result<Self, Self::Error> {
		let mut address = [0u8; 20];
		match val {
			CurrencyId::Token(token) => {
				address[H160_POSITION_CURRENCY_ID_TYPE] = CurrencyIdType::Token.into();
				address[H160_POSITION_TOKEN] = token.into();
			}
			CurrencyId::DexShare(left, right) => {
				let left_field: u32 = left.into();
				let right_field: u32 = right.into();
				address[H160_POSITION_CURRENCY_ID_TYPE] = CurrencyIdType::DexShare.into();
				address[H160_POSITION_DEXSHARE_LEFT_TYPE] = Into::<DexShareType>::into(left).into();
				address[H160_POSITION_DEXSHARE_LEFT_FIELD].copy_from_slice(&left_field.to_be_bytes());
				address[H160_POSITION_DEXSHARE_RIGHT_TYPE] = Into::<DexShareType>::into(right).into();
				address[H160_POSITION_DEXSHARE_RIGHT_FIELD].copy_from_slice(&right_field.to_be_bytes());
			}
			CurrencyId::Erc20(erc20) => {
				address[..].copy_from_slice(erc20.as_bytes());
			}
			CurrencyId::StableAssetPoolToken(stable_asset_id) => {
				address[H160_POSITION_CURRENCY_ID_TYPE] = CurrencyIdType::StableAsset.into();
				address[H160_POSITION_STABLE_ASSET].copy_from_slice(&stable_asset_id.to_be_bytes());
			}
			CurrencyId::LiquidCrowdloan(lease) => {
				address[H160_POSITION_CURRENCY_ID_TYPE] = CurrencyIdType::LiquidCrowdloan.into();
				address[H160_POSITION_LIQUID_CROADLOAN].copy_from_slice(&lease.to_be_bytes());
			}
			CurrencyId::ForeignAsset(foreign_asset_id) => {
				address[H160_POSITION_CURRENCY_ID_TYPE] = CurrencyIdType::ForeignAsset.into();
				address[H160_POSITION_FOREIGN_ASSET].copy_from_slice(&foreign_asset_id.to_be_bytes());
			}
		};

		Ok(EvmAddress::from_slice(&address))
	}
}

pub fn decode_gas_price(gas_price: u64, gas_limit: u64, tx_fee_per_gas: u128) -> Option<(u128, u32)> {
	// ensure gas_price >= 100 Gwei
	if u128::from(gas_price) < tx_fee_per_gas {
		return None;
	}

	let mut tip: u128 = 0;
	let mut actual_gas_price = gas_price;
	const TEN_GWEI: u64 = 10_000_000_000u64;

	// tip = 10% * tip_number
	let tip_number = gas_price.checked_div(TEN_GWEI)?.checked_sub(10)?;
	if !tip_number.is_zero() {
		actual_gas_price = gas_price.checked_sub(tip_number.checked_mul(TEN_GWEI)?)?;
		tip = actual_gas_price
			.checked_mul(gas_limit)?
			.checked_mul(tip_number)?
			.checked_div(10)? // percentage
			.checked_div(1_000_000)? // ACA decimal is 12, ETH decimal is 18
			.into();
	}

	// valid_until max is u32::MAX.
	let valid_until: u32 = Into::<u128>::into(actual_gas_price)
		.checked_sub(tx_fee_per_gas)?
		.saturated_into();

	Some((tip, valid_until))
}

pub fn decode_gas_limit(gas_limit: u64) -> (u64, u32) {
	let gas_and_storage: u64 = gas_limit.checked_rem(GAS_MASK).expect("constant never failed; qed");
	let actual_gas_limit: u64 = gas_and_storage
		.checked_div(STORAGE_MASK)
		.expect("constant never failed; qed")
		.saturating_mul(GAS_LIMIT_CHUNK);
	let storage_limit_number: u32 = gas_and_storage
		.checked_rem(STORAGE_MASK)
		.expect("constant never failed; qed")
		.try_into()
		.expect("STORAGE_MASK is 100, the result maximum is 99; qed");

	let actual_storage_limit = if storage_limit_number.is_zero() {
		Default::default()
	} else if storage_limit_number > MAX_GAS_LIMIT_CC {
		2u32.saturating_pow(MAX_GAS_LIMIT_CC)
	} else {
		2u32.saturating_pow(storage_limit_number)
	};

	(actual_gas_limit, actual_storage_limit)
}

#[cfg(not(feature = "evm-tests"))]
mod convert {
	use sp_runtime::traits::{CheckedDiv, Saturating, Zero};

	/// Convert decimal between native(12) and EVM(18) and therefore the 1_000_000 conversion.
	const DECIMALS_VALUE: u32 = 1_000_000u32;

	/// Convert decimal from native(KAR/ACA 12) to EVM(18).
	pub fn convert_decimals_to_evm<B: Zero + Saturating + From<u32>>(b: B) -> B {
		if b.is_zero() {
			return b;
		}
		b.saturating_mul(DECIMALS_VALUE.into())
	}

	/// Convert decimal from EVM(18) to native(KAR/ACA 12).
	pub fn convert_decimals_from_evm<B: Zero + Saturating + CheckedDiv + PartialEq + Copy + From<u32>>(
		b: B,
	) -> Option<B> {
		if b.is_zero() {
			return Some(b);
		}
		let res = b
			.checked_div(&Into::<B>::into(DECIMALS_VALUE))
			.expect("divisor is non-zero; qed");

		if res.saturating_mul(DECIMALS_VALUE.into()) == b {
			Some(res)
		} else {
			None
		}
	}
}

#[cfg(feature = "evm-tests")]
mod convert {
	pub fn convert_decimals_to_evm<B>(b: B) -> B {
		b
	}

	pub fn convert_decimals_from_evm<B>(b: B) -> Option<B> {
		Some(b)
	}
}

pub use convert::*;

#[cfg(feature = "tracing")]
pub mod tracing {
	use module_evm_utility::evm::Opcode;
	use parity_scale_codec::{Decode, Encode};
	use scale_info::TypeInfo;
	use sp_core::{H160, H256, U256};
	use sp_runtime::RuntimeDebug;
	use sp_std::vec::Vec;

	#[cfg(feature = "std")]
	use serde::{Deserialize, Serialize};

	#[derive(Clone, Eq, PartialEq, Default, Encode, Decode, RuntimeDebug, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub enum CallType {
		#[default]
		CALL,
		CALLCODE,
		STATICCALL,
		DELEGATECALL,
		CREATE,
		SUICIDE,
	}

	impl From<Opcode> for CallType {
		fn from(op: Opcode) -> Self {
			match op {
				Opcode::CALLCODE => CallType::CALLCODE,
				Opcode::DELEGATECALL => CallType::DELEGATECALL,
				Opcode::STATICCALL => CallType::STATICCALL,
				Opcode::CREATE | Opcode::CREATE2 => CallType::CREATE,
				Opcode::SUICIDE => CallType::SUICIDE,
				_ => CallType::CALL,
			}
		}
	}

	impl sp_std::fmt::Display for CallType {
		fn fmt(&self, f: &mut sp_std::fmt::Formatter<'_>) -> sp_std::fmt::Result {
			match self {
				CallType::CALL => write!(f, "CALL"),
				CallType::CALLCODE => write!(f, "CALLCODE"),
				CallType::STATICCALL => write!(f, "STATICCALL"),
				CallType::DELEGATECALL => write!(f, "DELEGATECALL"),
				CallType::CREATE => write!(f, "CREATE"),
				CallType::SUICIDE => write!(f, "SUICIDE"),
			}
		}
	}

	#[cfg(feature = "std")]
	mod maybe_hex {
		use serde::{Deserialize, Deserializer, Serializer};
		pub fn serialize<S: Serializer>(data: &Option<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error> {
			if let Some(data) = data {
				sp_core::bytes::serialize(data.as_slice(), serializer)
			} else {
				serializer.serialize_none()
			}
		}

		pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error> {
			use serde::de::Error;
			match Option::deserialize(deserializer) {
				Ok(Some(data)) => sp_core::bytes::from_hex(data).map_err(Error::custom).map(Some),
				Ok(None) => Ok(None),
				Err(e) => Err(e),
			}
		}
	}

	#[derive(Clone, Eq, PartialEq, Default, Encode, Decode, RuntimeDebug, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
	pub struct CallTrace {
		#[cfg_attr(feature = "std", serde(rename = "type"))]
		pub call_type: CallType,
		pub from: H160,
		pub to: H160,
		#[cfg_attr(feature = "std", serde(with = "sp_core::bytes"))]
		pub input: Vec<u8>,
		pub value: U256,
		// gas limit
		#[codec(compact)]
		pub gas: u64,
		#[codec(compact)]
		pub gas_used: u64,
		#[cfg_attr(feature = "std", serde(with = "maybe_hex"))]
		// value returned from EVM, if any
		pub output: Option<Vec<u8>>,
		#[cfg_attr(feature = "std", serde(with = "maybe_hex"))]
		// evm error, if any
		pub error: Option<Vec<u8>>,
		#[cfg_attr(feature = "std", serde(with = "maybe_hex"))]
		// revert reason, if any
		pub revert_reason: Option<Vec<u8>>,
		// depth of the call
		#[codec(compact)]
		pub depth: u32,
		// List of logs
		pub logs: Vec<LogTrace>,
		// List of sub-calls
		pub calls: Vec<CallTrace>,
	}

	#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
	pub enum LogTrace {
		Log {
			address: H160,
			topics: Vec<H256>,
			#[cfg_attr(feature = "std", serde(with = "sp_core::bytes"))]
			data: Vec<u8>,
		},
		SLoad {
			address: H160,
			index: H256,
			value: H256,
		},
		SStore {
			address: H160,
			index: H256,
			value: H256,
		},
	}

	#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
	pub struct Step {
		pub op: Opcode,
		#[codec(compact)]
		pub pc: u32,
		#[codec(compact)]
		pub depth: u32,
		#[codec(compact)]
		pub gas: u64,
		// 32 bytes stack items without leading zeros
		pub stack: Vec<Vec<u8>>,
		// Chunks of memory 32 bytes each without leading zeros except the last one which is untouched
		// Recreate the memory by joining the chunks. Each chunk (except the last one) should be 32 bytes
		pub memory: Option<Vec<Vec<u8>>>,
	}

	#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub enum TraceOutcome {
		Calls(Vec<CallTrace>),
		Steps(Vec<Step>),
	}

	#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub enum TracerConfig {
		CallTracer,
		OpcodeTracer(OpcodeConfig),
	}

	#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub struct OpcodeConfig {
		// Tracing opcodes is very expensive, so we need to limit the number of opcodes to trace.
		// Each trace call will have a maximum of `page_size` opcodes. If the number of opcodes
		// is equal to `page_size` then another trace call will be needed to get the next page of opcodes.
		pub page: u32,
		// Number of opcodes to trace in a single page.
		pub page_size: u32,
		pub disable_stack: bool,
		pub enable_memory: bool,
	}
}
