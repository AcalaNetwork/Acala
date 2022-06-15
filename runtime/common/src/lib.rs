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

//! Common runtime code for Acala, Karura and Mandala.

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	parameter_types,
	traits::{Contains, EnsureOneOf, Get},
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, WEIGHT_PER_MILLIS},
		DispatchClass, Weight,
	},
	RuntimeDebug,
};
use frame_system::{limits, EnsureRoot};
use module_evm::GenesisAccount;
use orml_traits::GetByKey;
use primitives::{evm::is_system_contract, Balance, CurrencyId, Nonce};
use scale_info::TypeInfo;
use sp_core::{Bytes, H160};
use sp_runtime::{traits::Convert, transaction_validity::TransactionPriority, Perbill};
use sp_std::{collections::btree_map::BTreeMap, marker::PhantomData, prelude::*};
use static_assertions::const_assert;

pub use check_nonce::CheckNonce;
pub use module_support::{ExchangeRate, PrecompileCallerFilter, Price, Rate, Ratio};
pub use precompile::{
	AllPrecompiles, DEXPrecompile, EVMPrecompile, MultiCurrencyPrecompile, NFTPrecompile, OraclePrecompile,
	SchedulePrecompile, StableAssetPrecompile,
};
pub use primitives::{
	currency::{TokenInfo, ACA, AUSD, BNC, DOT, KAR, KBTC, KINT, KSM, KUSD, LCDOT, LDOT, LKSM, PHA, RENBTC, VSKSM},
	AccountId,
};
pub use xcm_impl::{native_currency_location, AcalaDropAssets, FixedRateOfAsset};

#[cfg(feature = "std")]
use sp_core::bytes::from_hex;
#[cfg(feature = "std")]
use std::str::FromStr;

pub mod bench;
pub mod check_nonce;
pub mod precompile;
pub mod xcm_impl;

mod gas_to_weight_ratio;
#[cfg(test)]
mod mock;

pub type TimeStampedPrice = orml_oracle::TimestampedValue<Price, primitives::Moment>;

// Priority of unsigned transactions
parameter_types! {
	// Operational = final_fee * OperationalFeeMultiplier / TipPerWeightStep * max_tx_per_block + (tip + 1) / TipPerWeightStep * max_tx_per_block
	// final_fee_min = base_fee + len_fee + adjusted_weight_fee + tip
	// priority_min = final_fee * OperationalFeeMultiplier / TipPerWeightStep * max_tx_per_block + (tip + 1) / TipPerWeightStep * max_tx_per_block
	//              = final_fee_min * OperationalFeeMultiplier / TipPerWeightStep
	// Ensure Inherent -> Operational tx -> Unsigned tx -> Signed normal tx
	// Ensure `max_normal_priority < MinOperationalPriority / 2`
	pub TipPerWeightStep: Balance = cent(KAR); // 0.01 KAR/ACA
	pub MaxTipsOfPriority: Balance = 10_000 * dollar(KAR); // 10_000 KAR/ACA
	pub const OperationalFeeMultiplier: u64 = 100_000_000_000_000u64;
	// MinOperationalPriority = final_fee_min * OperationalFeeMultiplier / TipPerWeightStep
	// 1_500_000_000u128 from https://github.com/AcalaNetwork/Acala/blob/bda4d430cbecebf8720d700b976875d0d805ceca/runtime/integration-tests/src/runtime.rs#L275
	MinOperationalPriority: TransactionPriority = (1_500_000_000u128 * OperationalFeeMultiplier::get() as u128 / TipPerWeightStep::get())
		.try_into()
		.expect("Check that there is no overflow here");
	pub CdpEngineUnsignedPriority: TransactionPriority = MinOperationalPriority::get() - 1000;
	pub AuctionManagerUnsignedPriority: TransactionPriority = MinOperationalPriority::get() - 2000;
	pub RenvmBridgeUnsignedPriority: TransactionPriority = MinOperationalPriority::get() - 3000;
}

/// The call is allowed only if caller is a system contract.
pub struct SystemContractsFilter;
impl PrecompileCallerFilter for SystemContractsFilter {
	fn is_allowed(caller: H160) -> bool {
		is_system_contract(caller)
	}
}

/// Convert gas to weight
pub struct GasToWeight;
impl Convert<u64, Weight> for GasToWeight {
	fn convert(gas: u64) -> Weight {
		gas.saturating_mul(gas_to_weight_ratio::RATIO)
	}
}

pub struct ExistentialDepositsTimesOneHundred<NativeCurrencyId, NativeED, OtherEDs>(
	PhantomData<(NativeCurrencyId, NativeED, OtherEDs)>,
);
impl<NativeCurrencyId: Get<CurrencyId>, NativeED: Get<Balance>, OtherEDs: GetByKey<CurrencyId, Balance>>
	GetByKey<CurrencyId, Balance> for ExistentialDepositsTimesOneHundred<NativeCurrencyId, NativeED, OtherEDs>
{
	fn get(currency_id: &CurrencyId) -> Balance {
		if *currency_id == NativeCurrencyId::get() {
			NativeED::get().saturating_mul(100u128)
		} else {
			OtherEDs::get(currency_id).saturating_mul(100u128)
		}
	}
}

/// Convert weight to gas
pub struct WeightToGas;
impl Convert<Weight, u64> for WeightToGas {
	fn convert(weight: Weight) -> u64 {
		weight
			.checked_div(gas_to_weight_ratio::RATIO)
			.expect("Compile-time constant is not zero; qed;")
	}
}

// TODO: somehow estimate this value. Start from a conservative value.
pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(10);
/// The ratio that `Normal` extrinsics should occupy. Start from a conservative value.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(70);
/// Parachain only have 0.5 second of computation time.
pub const MAXIMUM_BLOCK_WEIGHT: Weight = 500 * WEIGHT_PER_MILLIS;

const_assert!(NORMAL_DISPATCH_RATIO.deconstruct() >= AVERAGE_ON_INITIALIZE_RATIO.deconstruct());

parameter_types! {
	/// Maximum length of block. Up to 5MB.
	pub RuntimeBlockLength: limits::BlockLength =
		limits::BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	/// Block weights base values and limits.
	pub RuntimeBlockWeights: limits::BlockWeights = limits::BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have an extra reserved space, so that they
			// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
			weights.reserved = Some(
				MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT,
			);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();
}

parameter_types! {
	/// A limit for off-chain phragmen unsigned solution submission.
	///
	/// We want to keep it as high as possible, but can't risk having it reject,
	/// so we always subtract the base block execution weight.
	pub OffchainSolutionWeightLimit: Weight = RuntimeBlockWeights::get()
		.get(DispatchClass::Normal)
		.max_extrinsic
		.expect("Normal extrinsics have weight limit configured by default; qed")
		.saturating_sub(BlockExecutionWeight::get());
}

pub struct DummyNomineeFilter;
impl<AccountId> Contains<AccountId> for DummyNomineeFilter {
	fn contains(_: &AccountId) -> bool {
		true
	}
}

// TODO: make those const fn
pub fn dollar(currency_id: CurrencyId) -> Balance {
	10u128.saturating_pow(currency_id.decimals().expect("Not support Non-Token decimals").into())
}

pub fn cent(currency_id: CurrencyId) -> Balance {
	dollar(currency_id) / 100
}

pub fn millicent(currency_id: CurrencyId) -> Balance {
	cent(currency_id) / 1000
}

pub fn microcent(currency_id: CurrencyId) -> Balance {
	millicent(currency_id) / 1000
}

pub type GeneralCouncilInstance = pallet_collective::Instance1;
pub type FinancialCouncilInstance = pallet_collective::Instance2;
pub type HomaCouncilInstance = pallet_collective::Instance3;
pub type TechnicalCommitteeInstance = pallet_collective::Instance4;

pub type GeneralCouncilMembershipInstance = pallet_membership::Instance1;
pub type FinancialCouncilMembershipInstance = pallet_membership::Instance2;
pub type HomaCouncilMembershipInstance = pallet_membership::Instance3;
pub type TechnicalCommitteeMembershipInstance = pallet_membership::Instance4;
pub type OperatorMembershipInstanceAcala = pallet_membership::Instance5;

// General Council
pub type EnsureRootOrAllGeneralCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, GeneralCouncilInstance, 1, 1>,
>;

pub type EnsureRootOrHalfGeneralCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, GeneralCouncilInstance, 1, 2>,
>;

pub type EnsureRootOrOneThirdsGeneralCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, GeneralCouncilInstance, 1, 3>,
>;

pub type EnsureRootOrTwoThirdsGeneralCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, GeneralCouncilInstance, 2, 3>,
>;

pub type EnsureRootOrThreeFourthsGeneralCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, GeneralCouncilInstance, 3, 4>,
>;

pub type EnsureRootOrOneGeneralCouncil =
	EnsureOneOf<EnsureRoot<AccountId>, pallet_collective::EnsureMember<AccountId, GeneralCouncilInstance>>;

// Financial Council
pub type EnsureRootOrAllFinancialCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, FinancialCouncilInstance, 1, 1>,
>;

pub type EnsureRootOrHalfFinancialCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, FinancialCouncilInstance, 1, 2>,
>;

pub type EnsureRootOrOneThirdsFinancialCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, FinancialCouncilInstance, 1, 3>,
>;

pub type EnsureRootOrTwoThirdsFinancialCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, FinancialCouncilInstance, 2, 3>,
>;

pub type EnsureRootOrThreeFourthsFinancialCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, FinancialCouncilInstance, 3, 4>,
>;

// Homa Council
pub type EnsureRootOrAllHomaCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, HomaCouncilInstance, 1, 1>,
>;

pub type EnsureRootOrHalfHomaCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, HomaCouncilInstance, 1, 2>,
>;

pub type EnsureRootOrOneThirdsHomaCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, HomaCouncilInstance, 1, 3>,
>;

pub type EnsureRootOrTwoThirdsHomaCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, HomaCouncilInstance, 2, 3>,
>;

pub type EnsureRootOrThreeFourthsHomaCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, HomaCouncilInstance, 3, 4>,
>;

// Technical Committee Council
pub type EnsureRootOrAllTechnicalCommittee = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCommitteeInstance, 1, 1>,
>;

pub type EnsureRootOrHalfTechnicalCommittee = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCommitteeInstance, 1, 2>,
>;

pub type EnsureRootOrOneThirdsTechnicalCommittee = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCommitteeInstance, 1, 3>,
>;

pub type EnsureRootOrTwoThirdsTechnicalCommittee = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCommitteeInstance, 2, 3>,
>;

pub type EnsureRootOrThreeFourthsTechnicalCommittee = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCommitteeInstance, 3, 4>,
>;

/// The type used to represent the kinds of proxying allowed.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum ProxyType {
	Any,
	CancelProxy,
	Governance,
	Auction,
	Swap,
	Loan,
	DexLiquidity,
	StableAssetSwap,
	StableAssetLiquidity,
	Homa,
}

impl Default for ProxyType {
	fn default() -> Self {
		Self::Any
	}
}

pub struct EvmLimits<T>(PhantomData<T>);
impl<T> EvmLimits<T>
where
	T: frame_system::Config,
{
	pub fn max_gas_limit() -> u64 {
		let weights = T::BlockWeights::get();
		let normal_weight = weights.get(DispatchClass::Normal);
		WeightToGas::convert(normal_weight.max_extrinsic.unwrap_or(weights.max_block))
	}

	pub fn max_storage_limit() -> u32 {
		let length = T::BlockLength::get();
		*length.max.get(DispatchClass::Normal)
	}
}

#[cfg(feature = "std")]
/// Returns `evm_genesis_accounts`
pub fn evm_genesis(evm_accounts: Vec<H160>) -> BTreeMap<H160, GenesisAccount<Balance, Nonce>> {
	let contracts_json = &include_bytes!("../../../predeploy-contracts/resources/bytecodes.json")[..];
	let contracts: Vec<(String, String, String)> = serde_json::from_slice(contracts_json).unwrap();
	let mut accounts = BTreeMap::new();
	for (_, address, code_string) in contracts {
		let account = GenesisAccount {
			nonce: 0u32,
			balance: 0u128,
			storage: BTreeMap::new(),
			code: Bytes::from_str(&code_string).unwrap().0,
			enable_contract_development: false,
		};

		let addr = H160::from_slice(
			from_hex(address.as_str())
				.expect("predeploy-contracts must specify address")
				.as_slice(),
		);
		accounts.insert(addr, account);
	}

	for dev_acc in evm_accounts {
		let account = GenesisAccount {
			nonce: 0u32,
			balance: 1000 * dollar(ACA),
			storage: BTreeMap::new(),
			code: vec![],
			enable_contract_development: true,
		};
		accounts.insert(dev_acc, account);
	}

	accounts
}

#[cfg(test)]
mod tests {
	use super::*;
	use primitives::evm::SYSTEM_CONTRACT_ADDRESS_PREFIX;

	#[test]
	fn system_contracts_filter_works() {
		assert!(SystemContractsFilter::is_allowed(H160::from_low_u64_be(1)));

		let mut max_allowed_addr = [0u8; 20];
		max_allowed_addr[SYSTEM_CONTRACT_ADDRESS_PREFIX.len()] = 127u8;
		assert!(SystemContractsFilter::is_allowed(max_allowed_addr.into()));

		let mut min_blocked_addr = [0u8; 20];
		min_blocked_addr[SYSTEM_CONTRACT_ADDRESS_PREFIX.len() - 1] = 1u8;
		assert!(!SystemContractsFilter::is_allowed(min_blocked_addr.into()));
	}

	#[test]
	fn check_max_normal_priority() {
		let max_normal_priority: TransactionPriority = (MaxTipsOfPriority::get() / TipPerWeightStep::get()
			* RuntimeBlockWeights::get()
				.max_block
				.min(*RuntimeBlockLength::get().max.get(DispatchClass::Normal) as u64) as u128)
			.try_into()
			.expect("Check that there is no overflow here");
		assert!(max_normal_priority < MinOperationalPriority::get() / 2); // 50%
	}
}
