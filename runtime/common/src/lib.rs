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

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::Get;
use frame_support::{
	parameter_types,
	traits::Contains,
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, WEIGHT_PER_MILLIS},
		DispatchClass, Weight,
	},
	RuntimeDebug,
};
use frame_system::{limits, EnsureOneOf, EnsureRoot};
pub use module_support::{ExchangeRate, PrecompileCallerFilter, Price, Rate, Ratio};
use primitives::{evm::is_system_contract, Balance, BlockNumber, CurrencyId};
use scale_info::TypeInfo;
use sp_core::{
	u32_trait::{_1, _2, _3, _4},
	H160,
};
use sp_runtime::{
	traits::{BlockNumberProvider, Convert},
	transaction_validity::TransactionPriority,
	FixedPointNumber, Perbill,
};
use static_assertions::const_assert;

pub mod check_nonce;
pub mod precompile;

#[cfg(test)]
mod mock;

pub use check_nonce::CheckNonce;
use orml_traits::GetByKey;
pub use precompile::{
	AllPrecompiles, DexPrecompile, MultiCurrencyPrecompile, NFTPrecompile, OraclePrecompile, ScheduleCallPrecompile,
	StateRentPrecompile,
};
pub use primitives::{
	currency::{TokenInfo, ACA, AUSD, BNC, DOT, KAR, KBTC, KINT, KSM, KUSD, LCDOT, LDOT, LKSM, PHA, RENBTC, VSKSM},
	AccountId,
};
use sp_std::{marker::PhantomData, prelude::*};
pub use xcm::latest::prelude::*;
pub use xcm_builder::TakeRevenue;
pub use xcm_executor::{traits::DropAssets, Assets};

mod gas_to_weight_ratio;

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

pub fn calculate_asset_ratio(foreign_asset: (AssetId, u128), native_asset: (AssetId, u128)) -> Ratio {
	Ratio::saturating_from_rational(foreign_asset.1, native_asset.1)
}

pub struct RelayChainBlockNumberProvider<T>(sp_std::marker::PhantomData<T>);

impl<T: cumulus_pallet_parachain_system::Config> BlockNumberProvider for RelayChainBlockNumberProvider<T> {
	type BlockNumber = BlockNumber;

	fn current_block_number() -> Self::BlockNumber {
		cumulus_pallet_parachain_system::Pallet::<T>::validation_data()
			.map(|d| d.relay_parent_number)
			.unwrap_or_default()
	}
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
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_1, _1, AccountId, GeneralCouncilInstance>,
>;

pub type EnsureRootOrHalfGeneralCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_1, _2, AccountId, GeneralCouncilInstance>,
>;

pub type EnsureRootOrOneThirdsGeneralCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_1, _3, AccountId, GeneralCouncilInstance>,
>;

pub type EnsureRootOrTwoThirdsGeneralCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_2, _3, AccountId, GeneralCouncilInstance>,
>;

pub type EnsureRootOrThreeFourthsGeneralCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_3, _4, AccountId, GeneralCouncilInstance>,
>;

pub type EnsureRootOrOneGeneralCouncil =
	EnsureOneOf<AccountId, EnsureRoot<AccountId>, pallet_collective::EnsureMember<AccountId, GeneralCouncilInstance>>;

// Financial Council
pub type EnsureRootOrAllFinancialCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_1, _1, AccountId, FinancialCouncilInstance>,
>;

pub type EnsureRootOrHalfFinancialCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_1, _2, AccountId, FinancialCouncilInstance>,
>;

pub type EnsureRootOrOneThirdsFinancialCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_1, _3, AccountId, FinancialCouncilInstance>,
>;

pub type EnsureRootOrTwoThirdsFinancialCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_2, _3, AccountId, FinancialCouncilInstance>,
>;

pub type EnsureRootOrThreeFourthsFinancialCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_3, _4, AccountId, FinancialCouncilInstance>,
>;

// Homa Council
pub type EnsureRootOrAllHomaCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_1, _1, AccountId, HomaCouncilInstance>,
>;

pub type EnsureRootOrHalfHomaCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_1, _2, AccountId, HomaCouncilInstance>,
>;

pub type EnsureRootOrOneThirdsHomaCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_1, _3, AccountId, HomaCouncilInstance>,
>;

pub type EnsureRootOrTwoThirdsHomaCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_2, _3, AccountId, HomaCouncilInstance>,
>;

pub type EnsureRootOrThreeFourthsHomaCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_3, _4, AccountId, HomaCouncilInstance>,
>;

// Technical Committee Council
pub type EnsureRootOrAllTechnicalCommittee = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_1, _1, AccountId, TechnicalCommitteeInstance>,
>;

pub type EnsureRootOrHalfTechnicalCommittee = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_1, _2, AccountId, TechnicalCommitteeInstance>,
>;

pub type EnsureRootOrOneThirdsTechnicalCommittee = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_1, _3, AccountId, TechnicalCommitteeInstance>,
>;

pub type EnsureRootOrTwoThirdsTechnicalCommittee = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_2, _3, AccountId, TechnicalCommitteeInstance>,
>;

pub type EnsureRootOrThreeFourthsTechnicalCommittee = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_3, _4, AccountId, TechnicalCommitteeInstance>,
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
}
impl Default for ProxyType {
	fn default() -> Self {
		Self::Any
	}
}

#[repr(u16)]
pub enum RelayChainSubAccountId {
	HomaLite = 0,
}

/// `DropAssets` implementation support asset amount lower thant ED handled by `TakeRevenue`.
///
/// parameters type:
/// - `NC`: native currency_id type.
/// - `NB`: the ExistentialDeposit amount of native currency_id.
/// - `GK`: the ExistentialDeposit amount of tokens.
pub struct AcalaDropAssets<X, T, C, NC, NB, GK>(PhantomData<(X, T, C, NC, NB, GK)>);
impl<X, T, C, NC, NB, GK> DropAssets for AcalaDropAssets<X, T, C, NC, NB, GK>
where
	X: DropAssets,
	T: TakeRevenue,
	C: Convert<MultiLocation, Option<CurrencyId>>,
	NC: Get<CurrencyId>,
	NB: Get<Balance>,
	GK: GetByKey<CurrencyId, Balance>,
{
	fn drop_assets(origin: &MultiLocation, assets: Assets) -> Weight {
		let multi_assets: Vec<MultiAsset> = assets.into();
		let mut asset_traps: Vec<MultiAsset> = vec![];
		for asset in multi_assets {
			if let MultiAsset {
				id: Concrete(location),
				fun: Fungible(amount),
			} = asset.clone()
			{
				let currency_id = C::convert(location);
				// burn asset(do nothing here) if convert result is None
				if let Some(currency_id) = currency_id {
					let ed = ExistentialDepositsForDropAssets::<NC, NB, GK>::get(&currency_id);
					if amount < ed {
						T::take_revenue(asset);
					} else {
						asset_traps.push(asset);
					}
				}
			}
		}
		if !asset_traps.is_empty() {
			X::drop_assets(origin, asset_traps.into());
		}
		0
	}
}

/// `ExistentialDeposit` for tokens, give priority to match native token, then handled by
/// `ExistentialDeposits`.
///
/// parameters type:
/// - `NC`: native currency_id type.
/// - `NB`: the ExistentialDeposit amount of native currency_id.
/// - `GK`: the ExistentialDeposit amount of tokens.
pub struct ExistentialDepositsForDropAssets<NC, NB, GK>(PhantomData<(NC, NB, GK)>);
impl<NC, NB, GK> ExistentialDepositsForDropAssets<NC, NB, GK>
where
	NC: Get<CurrencyId>,
	NB: Get<Balance>,
	GK: GetByKey<CurrencyId, Balance>,
{
	fn get(currency_id: &CurrencyId) -> Balance {
		if currency_id == &NC::get() {
			NB::get()
		} else {
			GK::get(currency_id)
		}
	}
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
