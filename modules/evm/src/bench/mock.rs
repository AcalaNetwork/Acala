// This file is part of Acala.

// Copyright (C) 2020-2025 Acala Foundation.
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

#![cfg(any(feature = "std", feature = "wasm-bench"))]

use super::super::*;

use frame_support::{
	construct_runtime, derive_impl, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, FindAuthor, Nothing},
	weights::{ConstantMultiplier, IdentityFee},
	ConsensusEngineId, PalletId,
};
use frame_system::EnsureSignedBy;
use module_support::{
	mocks::{MockAddressMapping, MockErc20InfoMapping, TestRandomness},
	DEXIncentives, Price, PriceProvider, SpecificJointsSwap,
};
use orml_traits::{parameter_type_with_key, MultiReservableCurrency};
pub use primitives::{
	define_combined_task, Address, Amount, BlockNumber, CurrencyId, Header, Multiplier, ReserveIdentifier, Signature,
	TokenSymbol,
};
use sp_core::H160;
use sp_runtime::{
	generic,
	traits::{AccountIdConversion, BlockNumberProvider, IdentityLookup},
	AccountId32, FixedU128, Percent,
};

type Balance = u128;
type Ratio = FixedU128;
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);

mod evm_mod {
	pub use super::super::super::*;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountId = AccountId32;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = pallet_balances::AccountData<Balance>;
	type BlockHashCount = ConstU32<10>;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = module_support::SystemAccountStore<Runtime>;
	type MaxLocks = ();
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = ReserveIdentifier;
	type WeightInfo = ();
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
}

impl pallet_timestamp::Config for Runtime {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ConstU64<1000>;
	type WeightInfo = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

impl orml_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ReserveIdentifier;
	type DustRemovalWhitelist = Nothing;
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
}

impl orml_currencies::Config for Runtime {
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}
pub type AdaptedBasicCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;

define_combined_task! {
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	pub enum ScheduledTasks {
		EvmTask(EvmTask<Runtime>),
	}
}

pub struct MockBlockNumberProvider;
impl BlockNumberProvider for MockBlockNumberProvider {
	type BlockNumber = u32;

	fn current_block_number() -> Self::BlockNumber {
		Zero::zero()
	}
}

parameter_types! {
	pub MinimumWeightRemainInBlock: Weight = Weight::zero();
}

impl module_idle_scheduler::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type Index = Nonce;
	type Task = ScheduledTasks;
	type MinimumWeightRemainInBlock = MinimumWeightRemainInBlock;
	type RelayChainBlockNumberProvider = MockBlockNumberProvider;
	type DisableBlockThreshold = ConstU32<6>;
}

pub struct GasToWeight;
impl Convert<u64, Weight> for GasToWeight {
	fn convert(a: u64) -> Weight {
		Weight::from_parts(a, 0)
	}
}

pub struct AuthorGiven;
impl FindAuthor<AccountId32> for AuthorGiven {
	fn find_author<'a, I>(_digests: I) -> Option<AccountId32>
	where
		I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
	{
		Some(AccountId32::from([1; 32]))
	}
}

parameter_types! {
	pub NetworkContractSource: H160 = H160::from_low_u64_be(1);
}

ord_parameter_types! {
	pub const CouncilAccount: AccountId32 = AccountId32::from([1u8; 32]);
	pub const TreasuryAccount: AccountId32 = AccountId32::from([2u8; 32]);
	pub const NetworkContractAccount: AccountId32 = AccountId32::from([0u8; 32]);
	pub const StorageDepositPerByte: Balance = convert_decimals_to_evm(10);
}

impl Config for Runtime {
	type AddressMapping = MockAddressMapping;
	type Currency = Balances;
	type TransferAll = Currencies;
	type NewContractExtraBytes = ConstU32<100>;
	type StorageDepositPerByte = StorageDepositPerByte;
	type TxFeePerGas = ConstU128<20_000_000>;

	type RuntimeEvent = RuntimeEvent;
	type PrecompilesType = ();
	type PrecompilesValue = ();
	type GasToWeight = GasToWeight;
	type ChargeTransactionPayment = module_transaction_payment::ChargeTransactionPayment<Runtime>;

	type NetworkContractOrigin = EnsureSignedBy<NetworkContractAccount, AccountId32>;
	type NetworkContractSource = NetworkContractSource;
	type DeveloperDeposit = ConstU128<1000>;
	type PublicationFee = ConstU128<200>;
	type TreasuryAccount = TreasuryAccount;
	type FreePublicationOrigin = EnsureSignedBy<CouncilAccount, AccountId32>;

	type Runner = crate::runner::stack::Runner<Self>;
	type FindAuthor = AuthorGiven;
	type Randomness = TestRandomness<Self>;
	type Task = ScheduledTasks;
	type IdleScheduler = IdleScheduler;
	type WeightInfo = ();
}

parameter_types! {
	pub const GetStableCurrencyId: CurrencyId = AUSD;
	pub MaxSwapSlippageCompareToOracle: Ratio = Ratio::one();
	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
	pub const TransactionPaymentPalletId: PalletId = PalletId(*b"aca/fees");
	pub KaruraTreasuryAccount: AccountId32 = TreasuryPalletId::get().into_account_truncating();
	pub const CustomFeeSurplus: Percent = Percent::from_percent(50);
	pub const AlternativeFeeSurplus: Percent = Percent::from_percent(25);
	pub DefaultFeeTokens: Vec<CurrencyId> = vec![AUSD];
	pub const TradingPathLimit: u32 = 4;
	pub const ExistenceRequirement: u128 = 1;
	pub AlternativeSwapPathJointList: Vec<Vec<CurrencyId>> = vec![];
}
ord_parameter_types! {
	pub const ListingOrigin: AccountId32 = AccountId32::new([1u8; 32]);
}
pub struct MockPriceSource;
impl PriceProvider<CurrencyId> for MockPriceSource {
	fn get_relative_price(_base: CurrencyId, _quote: CurrencyId) -> Option<Price> {
		Some(Price::one())
	}

	fn get_price(_currency_id: CurrencyId) -> Option<Price> {
		Some(Price::one())
	}
}

impl module_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type NativeCurrencyId = GetNativeCurrencyId;
	type Currency = Balances;
	type MultiCurrency = Currencies;
	type OnTransactionPayment = ();
	type OperationalFeeMultiplier = ConstU64<5>;
	type TipPerWeightStep = ConstU128<1>;
	type MaxTipsOfPriority = ConstU128<1000>;
	type AlternativeFeeSwapDeposit = ExistenceRequirement;
	type WeightToFee = IdentityFee<Balance>;
	type LengthToFee = ConstantMultiplier<Balance, ConstU128<10>>;
	type FeeMultiplierUpdate = ();
	type Swap = SpecificJointsSwap<Dex, AlternativeSwapPathJointList>;
	type MaxSwapSlippageCompareToOracle = MaxSwapSlippageCompareToOracle;
	type TradingPathLimit = TradingPathLimit;
	type PriceSource = MockPriceSource;
	type WeightInfo = ();
	type PalletId = TransactionPaymentPalletId;
	type TreasuryAccount = KaruraTreasuryAccount;
	type UpdateOrigin = EnsureSignedBy<ListingOrigin, AccountId32>;
	type CustomFeeSurplus = CustomFeeSurplus;
	type AlternativeFeeSurplus = AlternativeFeeSurplus;
	type DefaultFeeTokens = DefaultFeeTokens;
}

pub struct MockDEXIncentives;
impl DEXIncentives<AccountId32, CurrencyId, Balance> for MockDEXIncentives {
	fn do_deposit_dex_share(who: &AccountId32, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult {
		Tokens::reserve(lp_currency_id, who, amount)
	}

	fn do_withdraw_dex_share(who: &AccountId32, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult {
		let _ = Tokens::unreserve(lp_currency_id, who, amount);
		Ok(())
	}
}

parameter_types! {
	pub const GetExchangeFee: (u32, u32) = (1, 100);
	pub const DEXPalletId: PalletId = PalletId(*b"aca/dexm");
}

impl module_dex::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Tokens;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = TradingPathLimit;
	type PalletId = DEXPalletId;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type Erc20InfoMapping = MockErc20InfoMapping;
	type WeightInfo = ();
	type DEXIncentives = MockDEXIncentives;
	type ListingOrigin = EnsureSignedBy<ListingOrigin, AccountId32>;
	type ExtendedProvisioningBlocks = ConstU32<0>;
	type OnLiquidityPoolUpdated = ();
}

pub type SignedExtra = (frame_system::CheckWeight<Runtime>,);
pub type UncheckedExtrinsic = sp_runtime::generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Dex: module_dex,
		EVM: evm_mod,
		Tokens: orml_tokens,
		Balances: pallet_balances,
		Currencies: orml_currencies,
		IdleScheduler: module_idle_scheduler,
		TransactionPayment: module_transaction_payment,
	}
);
