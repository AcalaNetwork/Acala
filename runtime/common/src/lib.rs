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

//! Common runtime code for Acala, Karura and Mandala.

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "256"]

use cumulus_pallet_parachain_system::{CheckAssociatedRelayNumber, RelayChainStateProof};
use frame_support::{
	dispatch::DispatchClass,
	parameter_types,
	traits::{Contains, EitherOfDiverse, Get, Randomness},
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, WEIGHT_REF_TIME_PER_SECOND},
		Weight,
	},
};
use frame_system::{limits, pallet_prelude::BlockNumberFor, EnsureRoot};
use orml_traits::{currency::MutationHooks, GetByKey};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use polkadot_parachain_primitives::primitives::RelayChainBlockNumber;
use primitives::{
	evm::{is_system_contract, CHAIN_ID_ACALA_TESTNET, CHAIN_ID_KARURA_TESTNET, CHAIN_ID_MANDALA},
	Balance, CurrencyId,
};
use scale_info::TypeInfo;
use sp_core::H160;
use sp_runtime::{
	traits::{Convert, Hash},
	transaction_validity::TransactionPriority,
	Perbill, RuntimeDebug, Saturating,
};
use sp_std::{marker::PhantomData, prelude::*};
use static_assertions::const_assert;

pub use check_nonce::CheckNonce;
pub use module_support::{ExchangeRate, PrecompileCallerFilter, Price, Rate, Ratio};
pub use precompile::{
	AllPrecompiles, DEXPrecompile, EVMPrecompile, MultiCurrencyPrecompile, NFTPrecompile, OraclePrecompile,
	SchedulePrecompile, StableAssetPrecompile,
};
pub use primitives::{
	currency::{TokenInfo, ACA, AUSD, BNC, DOT, KAR, KBTC, KINT, KSM, KUSD, LCDOT, LDOT, LKSM, PHA, TAI, TAP, VSKSM},
	AccountId,
};
pub use xcm_impl::{local_currency_location, native_currency_location, AcalaDropAssets, FixedRateOfAsset, XcmExecutor};

#[cfg(feature = "std")]
use module_evm::GenesisAccount;
#[cfg(feature = "std")]
use sp_core::bytes::from_hex;
#[cfg(feature = "std")]
use std::{collections::btree_map::BTreeMap, str::FromStr};

pub mod bench;
pub mod check_nonce;
pub mod precompile;
pub mod xcm_config;
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
}

/// The call is allowed only if caller is a system contract.
pub struct SystemContractsFilter;
impl PrecompileCallerFilter for SystemContractsFilter {
	fn is_allowed(caller: H160) -> bool {
		is_system_contract(&caller)
	}
}

/// Convert gas to weight
pub struct GasToWeight;
impl Convert<u64, Weight> for GasToWeight {
	fn convert(gas: u64) -> Weight {
		Weight::from_parts(gas.saturating_mul(gas_to_weight_ratio::RATIO), 0)
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
			.ref_time()
			.checked_div(gas_to_weight_ratio::RATIO)
			.expect("Compile-time constant is not zero; qed;")
	}
}

pub struct CheckRelayNumber<EvmChainID, RelayNumberStrictlyIncreases>(EvmChainID, RelayNumberStrictlyIncreases);
impl<EvmChainID: Get<u64>, RelayNumberStrictlyIncreases: CheckAssociatedRelayNumber> CheckAssociatedRelayNumber
	for CheckRelayNumber<EvmChainID, RelayNumberStrictlyIncreases>
{
	fn check_associated_relay_number(current: RelayChainBlockNumber, previous: RelayChainBlockNumber) {
		match EvmChainID::get() {
			CHAIN_ID_MANDALA | CHAIN_ID_KARURA_TESTNET | CHAIN_ID_ACALA_TESTNET => {
				if current <= previous {
					log::warn!(
						"Relay chain block number was reset, current: {:?}, previous: {:?}",
						current,
						previous
					);
				}
			}
			_ => RelayNumberStrictlyIncreases::check_associated_relay_number(current, previous),
		}
	}
}

// TODO: somehow estimate this value. Start from a conservative value.
pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(10);
/// The ratio that `Normal` extrinsics should occupy. Start from a conservative value.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(70);
/// We allow for 0.5 seconds of compute with a 12 second average block time.
pub const MAXIMUM_BLOCK_WEIGHT: Weight = Weight::from_parts(
	WEIGHT_REF_TIME_PER_SECOND.saturating_div(2),
	// TODO: drop `* 10` after https://github.com/paritytech/substrate/issues/13501
	// and the benchmarked size is not 10x of the measured size
	polkadot_primitives::v7::MAX_POV_SIZE as u64 * 10,
);

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
pub type EnsureRootOrAllGeneralCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, GeneralCouncilInstance, 1, 1>,
>;

pub type EnsureRootOrHalfGeneralCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, GeneralCouncilInstance, 1, 2>,
>;

pub type EnsureRootOrOneThirdsGeneralCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, GeneralCouncilInstance, 1, 3>,
>;

pub type EnsureRootOrTwoThirdsGeneralCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, GeneralCouncilInstance, 2, 3>,
>;

pub type EnsureRootOrThreeFourthsGeneralCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, GeneralCouncilInstance, 3, 4>,
>;

pub type EnsureRootOrOneGeneralCouncil =
	EitherOfDiverse<EnsureRoot<AccountId>, pallet_collective::EnsureMember<AccountId, GeneralCouncilInstance>>;

// Financial Council
pub type EnsureRootOrAllFinancialCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, FinancialCouncilInstance, 1, 1>,
>;

pub type EnsureRootOrHalfFinancialCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, FinancialCouncilInstance, 1, 2>,
>;

pub type EnsureRootOrOneThirdsFinancialCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, FinancialCouncilInstance, 1, 3>,
>;

pub type EnsureRootOrTwoThirdsFinancialCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, FinancialCouncilInstance, 2, 3>,
>;

pub type EnsureRootOrThreeFourthsFinancialCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, FinancialCouncilInstance, 3, 4>,
>;

// Homa Council
pub type EnsureRootOrAllHomaCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, HomaCouncilInstance, 1, 1>,
>;

pub type EnsureRootOrHalfHomaCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, HomaCouncilInstance, 1, 2>,
>;

pub type EnsureRootOrOneThirdsHomaCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, HomaCouncilInstance, 1, 3>,
>;

pub type EnsureRootOrTwoThirdsHomaCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, HomaCouncilInstance, 2, 3>,
>;

pub type EnsureRootOrThreeFourthsHomaCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, HomaCouncilInstance, 3, 4>,
>;

// Technical Committee Council
pub type EnsureRootOrAllTechnicalCommittee = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCommitteeInstance, 1, 1>,
>;

pub type EnsureRootOrHalfTechnicalCommittee = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCommitteeInstance, 1, 2>,
>;

pub type EnsureRootOrOneThirdsTechnicalCommittee = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCommitteeInstance, 1, 3>,
>;

pub type EnsureRootOrTwoThirdsTechnicalCommittee = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCommitteeInstance, 2, 3>,
>;

pub type EnsureRootOrThreeFourthsTechnicalCommittee = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCommitteeInstance, 3, 4>,
>;

pub type EnsureRootOrOneTechnicalCommittee =
	EitherOfDiverse<EnsureRoot<AccountId>, pallet_collective::EnsureMember<AccountId, TechnicalCommitteeInstance>>;

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

pub struct CurrencyHooks<T, DustAccount>(PhantomData<T>, DustAccount);
impl<T, DustAccount> MutationHooks<T::AccountId, T::CurrencyId, T::Balance> for CurrencyHooks<T, DustAccount>
where
	T: orml_tokens::Config,
	DustAccount: Get<<T as frame_system::Config>::AccountId>,
{
	type OnDust = orml_tokens::TransferDust<T, DustAccount>;
	type OnSlash = ();
	type PreDeposit = ();
	type PostDeposit = ();
	type PreTransfer = ();
	type PostTransfer = ();
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
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

pub struct RandomnessSource<T>(sp_std::marker::PhantomData<T>);
impl<T: frame_system::Config> Randomness<T::Hash, BlockNumberFor<T>> for RandomnessSource<T>
where
	T: frame_system::Config + cumulus_pallet_parachain_system::Config + parachain_info::Config,
{
	fn random(subject: &[u8]) -> (T::Hash, BlockNumberFor<T>) {
		// If the relay randomness is not accessible, so insecure randomness is used and marked as stale
		// with a block number of zero
		let mut randomness: [u8; 32] = [0u8; 32];
		randomness.clone_from_slice(frame_system::Pallet::<T>::parent_hash().as_ref());
		let mut block_number = BlockNumberFor::<T>::default();

		// ValidationData is removed at on_initialize and set at the inherent, this means it could be empty
		// in the on_initialize hook for some pallets and some other inherents so this could fail when
		// invoked by scheduler or some other pallet's on_initialize hook
		if let Some(validation_data) = cumulus_pallet_parachain_system::ValidationData::<T>::get() {
			let relay_storage_root = validation_data.relay_parent_storage_root;

			if let Some(relay_state_proof) = cumulus_pallet_parachain_system::RelayStateProof::<T>::get() {
				if let Ok(relay_chain_state_proof) = RelayChainStateProof::new(
					parachain_info::Pallet::<T>::get(),
					relay_storage_root,
					relay_state_proof,
				) {
					if let Some(current_block_randomness) = relay_chain_state_proof
						.read_optional_entry(polkadot_primitives::well_known_keys::CURRENT_BLOCK_RANDOMNESS)
						.ok()
						.flatten()
					{
						randomness = current_block_randomness;
						// the randomness is from relaychain so there is a delay have a - 4 to indicate the randomness
						// is from previous relay block
						block_number = frame_system::Pallet::<T>::block_number().saturating_sub(4u8.into())
					}
				}
			}
		}

		let mut subject = subject.to_vec();
		subject.reserve(32); // RANDOMNESS_LENGTH is 32
		subject.extend_from_slice(&randomness);

		let random = T::Hashing::hash(&subject[..]);

		(random, block_number)
	}
}

#[cfg(feature = "std")]
/// Returns `evm_genesis_accounts`
pub fn evm_genesis(evm_accounts: Vec<H160>) -> BTreeMap<H160, GenesisAccount<Balance, primitives::Nonce>> {
	let contracts_json = &include_bytes!("../../../predeploy-contracts/resources/bytecodes.json")[..];
	let contracts: Vec<(String, String, String)> = serde_json::from_slice(contracts_json).unwrap();
	let mut accounts = BTreeMap::new();
	for (_, address, code_string) in contracts {
		let account = GenesisAccount {
			nonce: 0u32,
			balance: 0u128,
			storage: BTreeMap::new(),
			code: sp_core::Bytes::from_str(&code_string).unwrap().0,
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

/// Maximum number of blocks simultaneously accepted by the Runtime, not yet included
/// into the relay chain.
pub const UNINCLUDED_SEGMENT_CAPACITY: u32 = 1;
/// How many parachain blocks are processed by the relay chain per parent. Limits the
/// number of blocks authored per slot.
pub const BLOCK_PROCESSING_VELOCITY: u32 = 1;
/// Relay chain slot duration, in milliseconds.
pub const RELAY_CHAIN_SLOT_DURATION_MILLIS: u32 = 6000;

pub type ConsensusHook<Runtime> = cumulus_pallet_aura_ext::FixedVelocityConsensusHook<
	Runtime,
	RELAY_CHAIN_SLOT_DURATION_MILLIS,
	BLOCK_PROCESSING_VELOCITY,
	UNINCLUDED_SEGMENT_CAPACITY,
>;

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
				.ref_time()
				.min(*RuntimeBlockLength::get().max.get(DispatchClass::Normal) as u64) as u128)
			.try_into()
			.expect("Check that there is no overflow here");
		assert!(max_normal_priority < MinOperationalPriority::get() / 2); // 50%
	}
}
