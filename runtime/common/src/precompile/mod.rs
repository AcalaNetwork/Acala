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

//! The precompiles for EVM, includes standard Ethereum precompiles, and more:
//! - MultiCurrency at address `H160::from_low_u64_be(1024)`.

#![allow(clippy::upper_case_acronyms)]

pub mod mock;
mod tests;
mod weights;

use frame_support::log;
use hex_literal::hex;
use module_evm::{
	precompiles::{
		Blake2F, Bn128Add, Bn128Mul, Bn128Pairing, ECRecover, ECRecoverPublicKey, Identity, IstanbulModexp, Modexp,
		Precompile, Ripemd160, Sha256, Sha3FIPS256, Sha3FIPS512,
	},
	runner::state::{PrecompileFailure, PrecompileResult, PrecompileSet},
	Context, ExitRevert,
};
use module_support::PrecompileCallerFilter as PrecompileCallerFilterT;
use sp_core::H160;
use sp_std::{collections::btree_set::BTreeSet, marker::PhantomData};

pub mod dex;
pub mod evm;
pub mod evm_accounts;
pub mod homa;
pub mod honzon;
pub mod incentives;
pub mod input;
pub mod multicurrency;
pub mod nft;
pub mod oracle;
pub mod schedule;
pub mod stable_asset;

use crate::SystemContractsFilter;
pub use dex::DEXPrecompile;
pub use evm::EVMPrecompile;
pub use evm_accounts::EVMAccountsPrecompile;
pub use homa::HomaPrecompile;
pub use honzon::HonzonPrecompile;
pub use incentives::IncentivesPrecompile;
pub use multicurrency::MultiCurrencyPrecompile;
pub use nft::NFTPrecompile;
pub use oracle::OraclePrecompile;
pub use schedule::SchedulePrecompile;
pub use stable_asset::StableAssetPrecompile;

pub const ECRECOVER: H160 = H160(hex!("0000000000000000000000000000000000000001"));
pub const SHA256: H160 = H160(hex!("0000000000000000000000000000000000000002"));
pub const RIPEMD: H160 = H160(hex!("0000000000000000000000000000000000000003"));
pub const IDENTITY: H160 = H160(hex!("0000000000000000000000000000000000000004"));
pub const MODEXP: H160 = H160(hex!("0000000000000000000000000000000000000005"));
pub const BN_ADD: H160 = H160(hex!("0000000000000000000000000000000000000006"));
pub const BN_MUL: H160 = H160(hex!("0000000000000000000000000000000000000007"));
pub const BN_PAIRING: H160 = H160(hex!("0000000000000000000000000000000000000008"));
pub const BLAKE2F: H160 = H160(hex!("0000000000000000000000000000000000000009"));

pub const ETH_PRECOMPILE_END: H160 = BLAKE2F;

pub const ECRECOVER_PUBLICKEY: H160 = H160(hex!("0000000000000000000000000000000000000080"));
pub const SHA3_256: H160 = H160(hex!("0000000000000000000000000000000000000081"));
pub const SHA3_512: H160 = H160(hex!("0000000000000000000000000000000000000082"));

pub const MULTI_CURRENCY: H160 = H160(hex!("0000000000000000000000000000000000000400"));
pub const NFT: H160 = H160(hex!("0000000000000000000000000000000000000401"));
pub const EVM: H160 = H160(hex!("0000000000000000000000000000000000000402"));
pub const ORACLE: H160 = H160(hex!("0000000000000000000000000000000000000403"));
pub const SCHEDULER: H160 = H160(hex!("0000000000000000000000000000000000000404"));
pub const DEX: H160 = H160(hex!("0000000000000000000000000000000000000405"));
pub const STABLE_ASSET: H160 = H160(hex!("0000000000000000000000000000000000000406"));
pub const HOMA: H160 = H160(hex!("0000000000000000000000000000000000000407"));
pub const EVM_ACCOUNTS: H160 = H160(hex!("0000000000000000000000000000000000000408"));
pub const HONZON: H160 = H160(hex!("0000000000000000000000000000000000000409"));
pub const INCENTIVES: H160 = H160(hex!("000000000000000000000000000000000000040a"));

pub fn target_gas_limit(target_gas: Option<u64>) -> Option<u64> {
	target_gas.map(|x| x.saturating_div(10).saturating_mul(9)) // 90%
}

pub struct AllPrecompiles<R> {
	active: BTreeSet<H160>,
	_marker: PhantomData<R>,
}

impl<R> AllPrecompiles<R>
where
	R: module_evm::Config,
{
	pub fn acala() -> Self {
		Self {
			active: BTreeSet::from([
				ECRECOVER,
				SHA256,
				RIPEMD,
				IDENTITY,
				MODEXP,
				BN_ADD,
				BN_MUL,
				BN_PAIRING,
				BLAKE2F,
				// Non-standard precompile starts with 128
				ECRECOVER_PUBLICKEY,
				SHA3_256,
				SHA3_512,
				// Acala precompile
				MULTI_CURRENCY,
				// NFT,
				EVM,
				ORACLE,
				// SCHEDULER,
				DEX,
				// STABLE_ASSET,
				// HOMA,
				EVM_ACCOUNTS,
				/* HONZON
				 * INCENTIVES */
			]),
			_marker: Default::default(),
		}
	}

	pub fn karura() -> Self {
		Self {
			active: BTreeSet::from([
				ECRECOVER,
				SHA256,
				RIPEMD,
				IDENTITY,
				MODEXP,
				BN_ADD,
				BN_MUL,
				BN_PAIRING,
				BLAKE2F,
				// Non-standard precompile starts with 128
				ECRECOVER_PUBLICKEY,
				SHA3_256,
				SHA3_512,
				// Acala precompile
				MULTI_CURRENCY,
				// NFT,
				EVM,
				ORACLE,
				// SCHEDULER,
				DEX,
				// STABLE_ASSET,
				// HOMA,
				EVM_ACCOUNTS,
				/* HONZON
				 * INCENTIVES */
			]),
			_marker: Default::default(),
		}
	}

	pub fn mandala() -> Self {
		Self {
			active: BTreeSet::from([
				ECRECOVER,
				SHA256,
				RIPEMD,
				IDENTITY,
				MODEXP,
				BN_ADD,
				BN_MUL,
				BN_PAIRING,
				BLAKE2F,
				// Non-standard precompile starts with 128
				ECRECOVER_PUBLICKEY,
				SHA3_256,
				SHA3_512,
				// Acala precompile
				MULTI_CURRENCY,
				NFT,
				EVM,
				ORACLE,
				SCHEDULER,
				DEX,
				STABLE_ASSET,
				HOMA,
				EVM_ACCOUNTS,
				HONZON,
				INCENTIVES,
			]),
			_marker: Default::default(),
		}
	}
}

impl<R> PrecompileSet for AllPrecompiles<R>
where
	R: module_evm::Config,
	MultiCurrencyPrecompile<R>: Precompile,
	NFTPrecompile<R>: Precompile,
	EVMPrecompile<R>: Precompile,
	EVMAccountsPrecompile<R>: Precompile,
	OraclePrecompile<R>: Precompile,
	DEXPrecompile<R>: Precompile,
	StableAssetPrecompile<R>: Precompile,
	SchedulePrecompile<R>: Precompile,
	HomaPrecompile<R>: Precompile,
	HonzonPrecompile<R>: Precompile,
	IncentivesPrecompile<R>: Precompile,
{
	fn execute(
		&self,
		address: H160,
		input: &[u8],
		target_gas: Option<u64>,
		context: &Context,
		is_static: bool,
	) -> Option<PrecompileResult> {
		if !self.is_precompile(address) {
			return None;
		}

		// Filter known precompile addresses except Ethereum officials
		if address > ETH_PRECOMPILE_END && context.address != address {
			return Some(Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "cannot be called with DELEGATECALL or CALLCODE".into(),
				cost: target_gas.unwrap_or_default(),
			}));
		}

		log::trace!(target: "evm", "Precompile begin, address: {:?}, input: {:?}, target_gas: {:?}, context: {:?}", address, input, target_gas, context);

		// https://github.com/ethereum/go-ethereum/blob/9357280fce5c5d57111d690a336cca5f89e34da6/core/vm/contracts.go#L83
		let result = if address == ECRECOVER {
			Some(ECRecover::execute(input, target_gas, context, is_static))
		} else if address == SHA256 {
			Some(Sha256::execute(input, target_gas, context, is_static))
		} else if address == RIPEMD {
			Some(Ripemd160::execute(input, target_gas, context, is_static))
		} else if address == IDENTITY {
			Some(Identity::execute(input, target_gas, context, is_static))
		} else if address == MODEXP {
			if R::config().increase_state_access_gas {
				Some(Modexp::execute(input, target_gas, context, is_static))
			} else {
				Some(IstanbulModexp::execute(input, target_gas, context, is_static))
			}
		} else if address == BN_ADD {
			Some(Bn128Add::execute(input, target_gas, context, is_static))
		} else if address == BN_MUL {
			Some(Bn128Mul::execute(input, target_gas, context, is_static))
		} else if address == BN_PAIRING {
			Some(Bn128Pairing::execute(input, target_gas, context, is_static))
		} else if address == BLAKE2F {
			Some(Blake2F::execute(input, target_gas, context, is_static))
		}
		// Non-standard precompile starts with 128
		else if address == ECRECOVER_PUBLICKEY {
			Some(ECRecoverPublicKey::execute(input, target_gas, context, is_static))
		} else if address == SHA3_256 {
			Some(Sha3FIPS256::execute(input, target_gas, context, is_static))
		} else if address == SHA3_512 {
			Some(Sha3FIPS512::execute(input, target_gas, context, is_static))
		}
		// Acala precompile
		else {
			if !SystemContractsFilter::is_allowed(context.caller) {
				log::debug!(target: "evm", "Precompile no permission: {:?}", context.caller);
				return Some(Err(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "NoPermission".into(),
					cost: target_gas.unwrap_or_default(),
				}));
			}

			if !module_evm::Pallet::<R>::is_contract(&context.caller) {
				log::debug!(target: "evm", "Caller is not a system contract: {:?}", context.caller);
				return Some(Err(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Caller is not a system contract".into(),
					cost: target_gas.unwrap_or_default(),
				}));
			}

			if address == MULTI_CURRENCY {
				Some(MultiCurrencyPrecompile::<R>::execute(
					input, target_gas, context, is_static,
				))
			} else if address == NFT {
				Some(NFTPrecompile::<R>::execute(input, target_gas, context, is_static))
			} else if address == EVM {
				Some(EVMPrecompile::<R>::execute(input, target_gas, context, is_static))
			} else if address == ORACLE {
				Some(OraclePrecompile::<R>::execute(input, target_gas, context, is_static))
			} else if address == SCHEDULER {
				Some(SchedulePrecompile::<R>::execute(input, target_gas, context, is_static))
			} else if address == DEX {
				Some(DEXPrecompile::<R>::execute(input, target_gas, context, is_static))
			} else if address == STABLE_ASSET {
				Some(StableAssetPrecompile::<R>::execute(
					input, target_gas, context, is_static,
				))
			} else if address == HOMA {
				Some(HomaPrecompile::<R>::execute(input, target_gas, context, is_static))
			} else if address == EVM_ACCOUNTS {
				Some(EVMAccountsPrecompile::<R>::execute(
					input, target_gas, context, is_static,
				))
			} else if address == HONZON {
				Some(HonzonPrecompile::<R>::execute(input, target_gas, context, is_static))
			} else if address == INCENTIVES {
				Some(IncentivesPrecompile::<R>::execute(
					input, target_gas, context, is_static,
				))
			} else {
				None
			}
		};

		log::trace!(target: "evm", "Precompile end, address: {:?}, input: {:?}, target_gas: {:?}, context: {:?}, result: {:?}", address, input, target_gas, context, result);
		if let Some(Err(PrecompileFailure::Revert { ref output, .. })) = result {
			log::debug!(target: "evm", "Precompile failed: {:?}", core::str::from_utf8(output));
		};
		result
	}

	fn is_precompile(&self, address: H160) -> bool {
		self.active.contains(&address)
	}
}

#[test]
fn ensure_precompile_address_start() {
	use primitives::evm::PRECOMPILE_ADDRESS_START;
	assert_eq!(PRECOMPILE_ADDRESS_START, MULTI_CURRENCY);
}
