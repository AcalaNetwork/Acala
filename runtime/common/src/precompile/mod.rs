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

//! The precompiles for EVM, includes standard Ethereum precompiles, and more:
//! - MultiCurrency at address `H160::from_low_u64_be(1024)`.

#![allow(clippy::upper_case_acronyms)]

pub mod mock;
mod tests;
mod weights;

use hex_literal::hex;
use module_evm::{
	precompiles::{
		Blake2F, Bn128Add, Bn128Mul, Bn128Pairing, ECRecover, ECRecoverPublicKey, Identity, IstanbulModexp, Modexp,
		Precompile, Ripemd160, Sha256, Sha3FIPS256, Sha3FIPS512,
	},
	ExitRevert, IsPrecompileResult, PrecompileFailure, PrecompileHandle, PrecompileResult, PrecompileSet,
};
use module_support::{PrecompileCallerFilter, PrecompilePauseFilter};
use sp_core::H160;
use sp_runtime::traits::Zero;
use sp_std::{collections::btree_set::BTreeSet, marker::PhantomData};

pub mod dex;
pub mod earning;
pub mod evm;
pub mod evm_accounts;
pub mod homa;
pub mod honzon;
pub mod incentives;
pub mod input;
pub mod liquid_crowdloan;
pub mod multicurrency;
pub mod nft;
pub mod oracle;
pub mod schedule;
pub mod stable_asset;
pub mod xtokens;

use crate::SystemContractsFilter;
pub use dex::DEXPrecompile;
pub use earning::EarningPrecompile;
pub use evm::EVMPrecompile;
pub use evm_accounts::EVMAccountsPrecompile;
pub use homa::HomaPrecompile;
pub use honzon::HonzonPrecompile;
pub use incentives::IncentivesPrecompile;
pub use liquid_crowdloan::LiquidCrowdloanPrecompile;
pub use multicurrency::MultiCurrencyPrecompile;
pub use nft::NFTPrecompile;
pub use oracle::OraclePrecompile;
pub use schedule::SchedulePrecompile;
pub use stable_asset::StableAssetPrecompile;
pub use xtokens::XtokensPrecompile;

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
pub const XTOKENS: H160 = H160(hex!("000000000000000000000000000000000000040b"));
pub const LIQUID_CROWDLOAN: H160 = H160(hex!("000000000000000000000000000000000000040c"));
pub const EARNING: H160 = H160(hex!("000000000000000000000000000000000000040d"));

pub struct AllPrecompiles<R, F, E> {
	set: BTreeSet<H160>,
	_marker: PhantomData<(R, F, E)>,
}

impl<R, F, E> AllPrecompiles<R, F, E>
where
	R: module_evm::Config,
	E: PrecompileSet,
{
	pub fn acala() -> Self {
		Self {
			set: BTreeSet::from([
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
				STABLE_ASSET,
				HOMA,
				EVM_ACCOUNTS,
				HONZON,
				INCENTIVES,
				XTOKENS,
				LIQUID_CROWDLOAN,
				EARNING,
			]),
			_marker: Default::default(),
		}
	}

	pub fn karura() -> Self {
		Self {
			set: BTreeSet::from([
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
				STABLE_ASSET,
				HOMA,
				EVM_ACCOUNTS,
				HONZON,
				INCENTIVES,
				XTOKENS,
				// LIQUID_CROWDLOAN,
				EARNING,
			]),
			_marker: Default::default(),
		}
	}

	pub fn mandala() -> Self {
		Self {
			set: BTreeSet::from([
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
				XTOKENS,
				// LIQUID_CROWDLOAN,
				EARNING,
			]),
			_marker: Default::default(),
		}
	}
}

impl<R, PausedPrecompile, E> PrecompileSet for AllPrecompiles<R, PausedPrecompile, E>
where
	R: module_evm::Config,
	E: PrecompileSet + Default,
	PausedPrecompile: PrecompilePauseFilter,
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
	XtokensPrecompile<R>: Precompile,
	EarningPrecompile<R>: Precompile,
{
	fn execute(&self, handle: &mut impl PrecompileHandle) -> Option<PrecompileResult> {
		let context = handle.context();
		let address = handle.code_address();

		if let IsPrecompileResult::Answer {
			is_precompile: false, ..
		} = self.is_precompile(address, u64::zero())
		{
			return None;
		}

		// ensure precompile is not paused
		if PausedPrecompile::is_paused(address) {
			log::debug!(target: "evm", "Precompile {:?} is paused", address);
			return Some(Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "precompile is paused".into(),
			}));
		}

		// Filter known precompile addresses except Ethereum officials
		if address > ETH_PRECOMPILE_END && context.address != address {
			return Some(Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "cannot be called with DELEGATECALL or CALLCODE".into(),
			}));
		}

		log::trace!(target: "evm", "Precompile begin, address: {:?}, input: {:?}, context: {:?}", address, handle.input(), context);

		// https://github.com/ethereum/go-ethereum/blob/9357280fce5c5d57111d690a336cca5f89e34da6/core/vm/contracts.go#L83
		let result = if address == ECRECOVER {
			Some(ECRecover::execute(handle))
		} else if address == SHA256 {
			Some(Sha256::execute(handle))
		} else if address == RIPEMD {
			Some(Ripemd160::execute(handle))
		} else if address == IDENTITY {
			Some(Identity::execute(handle))
		} else if address == MODEXP {
			if R::config().increase_state_access_gas {
				Some(Modexp::execute(handle))
			} else {
				Some(IstanbulModexp::execute(handle))
			}
		} else if address == BN_ADD {
			Some(Bn128Add::execute(handle))
		} else if address == BN_MUL {
			Some(Bn128Mul::execute(handle))
		} else if address == BN_PAIRING {
			Some(Bn128Pairing::execute(handle))
		} else if address == BLAKE2F {
			Some(Blake2F::execute(handle))
		}
		// Non-standard precompile starts with 128
		else if address == ECRECOVER_PUBLICKEY {
			Some(ECRecoverPublicKey::execute(handle))
		} else if address == SHA3_256 {
			Some(Sha3FIPS256::execute(handle))
		} else if address == SHA3_512 {
			Some(Sha3FIPS512::execute(handle))
		}
		// Acala precompile
		else {
			if !SystemContractsFilter::is_allowed(context.caller) {
				log::debug!(target: "evm", "Precompile no permission: {:?}", context.caller);
				return Some(Err(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "NoPermission".into(),
				}));
			}

			if !module_evm::Pallet::<R>::is_contract(&context.caller) {
				log::debug!(target: "evm", "Caller is not a system contract: {:?}", context.caller);
				return Some(Err(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Caller is not a system contract".into(),
				}));
			}

			if address == MULTI_CURRENCY {
				Some(MultiCurrencyPrecompile::<R>::execute(handle))
			} else if address == NFT {
				Some(NFTPrecompile::<R>::execute(handle))
			} else if address == EVM {
				Some(EVMPrecompile::<R>::execute(handle))
			} else if address == ORACLE {
				Some(OraclePrecompile::<R>::execute(handle))
			} else if address == SCHEDULER {
				Some(SchedulePrecompile::<R>::execute(handle))
			} else if address == DEX {
				Some(DEXPrecompile::<R>::execute(handle))
			} else if address == STABLE_ASSET {
				Some(StableAssetPrecompile::<R>::execute(handle))
			} else if address == HOMA {
				Some(HomaPrecompile::<R>::execute(handle))
			} else if address == EVM_ACCOUNTS {
				Some(EVMAccountsPrecompile::<R>::execute(handle))
			} else if address == HONZON {
				Some(HonzonPrecompile::<R>::execute(handle))
			} else if address == INCENTIVES {
				Some(IncentivesPrecompile::<R>::execute(handle))
			} else if address == XTOKENS {
				Some(XtokensPrecompile::<R>::execute(handle))
			} else if address == EARNING {
				Some(EarningPrecompile::<R>::execute(handle))
			} else {
				E::execute(&Default::default(), handle)
			}
		};

		log::trace!(target: "evm", "Precompile end, address: {:?}, input: {:?}, context: {:?}, result: {:?}", address, handle.input(), handle.context(), result);
		if let Some(Err(PrecompileFailure::Revert { ref output, .. })) = result {
			log::debug!(target: "evm", "Precompile failed: {:?}", core::str::from_utf8(output));
		};
		result
	}

	fn is_precompile(&self, address: H160, _remaining_gas: u64) -> IsPrecompileResult {
		let is_precompile = {
			self.set.contains(&address)
				|| match E::is_precompile(&Default::default(), address, u64::zero()) {
					IsPrecompileResult::Answer { is_precompile, .. } => is_precompile,
					_ => false,
				}
		};

		IsPrecompileResult::Answer {
			is_precompile,
			extra_cost: 0,
		}
	}
}

pub struct AcalaPrecompiles<R>(sp_std::marker::PhantomData<R>);

impl<R> Default for AcalaPrecompiles<R> {
	fn default() -> Self {
		Self(sp_std::marker::PhantomData)
	}
}

impl<R> PrecompileSet for AcalaPrecompiles<R>
where
	LiquidCrowdloanPrecompile<R>: Precompile,
{
	fn execute(&self, handle: &mut impl PrecompileHandle) -> Option<PrecompileResult> {
		let address = handle.code_address();
		if address == LIQUID_CROWDLOAN {
			Some(LiquidCrowdloanPrecompile::execute(handle))
		} else {
			None
		}
	}

	fn is_precompile(&self, address: H160, _remaining_gas: u64) -> IsPrecompileResult {
		IsPrecompileResult::Answer {
			is_precompile: address == LIQUID_CROWDLOAN,
			extra_cost: 0,
		}
	}
}

#[test]
fn ensure_precompile_address_start() {
	use primitives::evm::PRECOMPILE_ADDRESS_START;
	assert_eq!(PRECOMPILE_ADDRESS_START, MULTI_CURRENCY);
}
