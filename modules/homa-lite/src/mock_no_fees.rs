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

//! Mocks for the HomaLite module that costs no Fees.

#![cfg(test)]

pub use crate as module_homa_lite;
pub use crate::mock::*;

mod homa_lite {
	pub use super::super::*;
}
pub struct MockXcm;
impl XcmTransfer<AccountId, Balance, CurrencyId> for MockXcm {
	fn transfer(
		who: AccountId,
		_currency_id: CurrencyId,
		amount: Balance,
		_dest: MultiLocation,
		_dest_weight: Weight,
	) -> DispatchResult {
		Currencies::slash(KSM, &who, amount);
		match who {
			INVALID_CALLER => Err(DispatchError::Other("invalid caller")),
			_ => Ok(()),
		}
	}

	/// Transfer `MultiAsset`
	fn transfer_multi_asset(
		_who: AccountId,
		_asset: MultiAsset,
		_dest: MultiLocation,
		_dest_weight: Weight,
	) -> DispatchResult {
		Ok(())
	}
}
impl InvertLocation for MockXcm {
	fn ancestry() -> MultiLocation {
		Parachain(2000).into()
	}

	fn invert_location(l: &MultiLocation) -> Result<MultiLocation, ()> {
		Ok(l.clone())
	}
}

impl SendXcm for MockXcm {
	fn send_xcm(dest: impl Into<MultiLocation>, msg: Xcm<()>) -> SendResult {
		let dest = dest.into();
		match dest {
			MultiLocation {
				parents: 1,
				interior: Junctions::Here,
			} => Ok(()),
			_ => Err(SendError::CannotReachDestination(dest, msg)),
		}
	}
}

impl ExecuteXcm<Call> for MockXcm {
	fn execute_xcm_in_credit(
		_origin: impl Into<MultiLocation>,
		mut _message: Xcm<Call>,
		_weight_limit: Weight,
		_weight_credit: Weight,
	) -> Outcome {
		Outcome::Complete(0)
	}
}

pub struct MockEnsureXcmOrigin;
impl EnsureOrigin<Origin> for MockEnsureXcmOrigin {
	type Success = MultiLocation;
	fn try_origin(_o: Origin) -> Result<Self::Success, Origin> {
		Ok(MultiLocation::here())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn successful_origin() -> Origin {
		let zero_account_id = AccountId::decode(&mut sp_runtime::traits::TrailingZeroInput::zeroes())
			.expect("infinite length input; no invalid inputs for type; qed");
		Origin::from(RawOrigin::Signed(zero_account_id))
	}
}
pub struct MockWeigher;
impl WeightBounds<Call> for MockWeigher {
	fn weight(_message: &mut Xcm<Call>) -> Result<Weight, ()> {
		Ok(0)
	}

	fn instr_weight(_message: &Instruction<Call>) -> Result<Weight, ()> {
		Ok(0)
	}
}

impl pallet_xcm::Config for NoFeeRuntime {
	type Event = Event;
	type SendXcmOrigin = MockEnsureXcmOrigin;
	type XcmRouter = MockXcm;
	type ExecuteXcmOrigin = MockEnsureXcmOrigin;
	type XcmExecuteFilter = Nothing;
	type XcmExecutor = MockXcm;
	type XcmTeleportFilter = Everything;
	type XcmReserveTransferFilter = Everything;
	type Weigher = MockWeigher;
	type LocationInverter = MockXcm;
	type Origin = Origin;
	type Call = Call;
	const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
}

impl frame_system::Config for NoFeeRuntime {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

impl orml_tokens::Config for NoFeeRuntime {
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = Nothing;
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
}

impl pallet_balances::Config for NoFeeRuntime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ConstU128<0>;
	type AccountStore = frame_system::Pallet<NoFeeRuntime>;
	type MaxLocks = ();
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

pub type AdaptedBasicCurrency =
	module_currencies::BasicCurrencyAdapter<NoFeeRuntime, PalletBalances, Amount, BlockNumber>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACALA;
	pub Erc20HoldingAccount: H160 = H160::from_low_u64_be(1);
}

impl module_currencies::Config for NoFeeRuntime {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type Erc20HoldingAccount = Erc20HoldingAccount;
	type WeightInfo = ();
	type AddressMapping = MockAddressMapping;
	type EVMBridge = ();
	type GasToWeight = ();
	type SweepOrigin = EnsureSignedBy<Root, AccountId>;
	type OnDust = ();
}

parameter_types! {
	pub const StakingCurrencyId: CurrencyId = KSM;
	pub const LiquidCurrencyId: CurrencyId = LKSM;
	pub MinimumMintThreshold: Balance = millicent(1);
	pub MinimumRedeemThreshold: Balance = millicent(1);
	pub const MockXcmDestination: MultiLocation = MOCK_XCM_DESTINATION;
	pub const MockXcmAccountId: AccountId = MOCK_XCM_ACCOUNT_ID;
	pub DefaultExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(1, 10);
	pub const MaxRewardPerEra: Permill = Permill::zero();
	pub BaseWithdrawFee: Permill = Permill::zero(); // 0.1%
	pub const ParachainAccount: AccountId = DAVE;
	pub ParachainId: ParaId = ParaId::from(PARACHAIN_ID);
}

ord_parameter_types! {
	pub const Root: AccountId = DAVE;
}

impl Config for NoFeeRuntime {
	type Event = Event;
	type WeightInfo = ();
	type Currency = Currencies;
	type StakingCurrencyId = StakingCurrencyId;
	type LiquidCurrencyId = LiquidCurrencyId;
	type GovernanceOrigin = EnsureRoot<AccountId>;
	type MinimumMintThreshold = MinimumMintThreshold;
	type MinimumRedeemThreshold = MinimumRedeemThreshold;
	type XcmTransfer = MockXcm;
	type SovereignSubAccountLocation = MockXcmDestination;
	type SubAccountIndex = ConstU16<0>;
	type DefaultExchangeRate = DefaultExchangeRate;
	type MaxRewardPerEra = MaxRewardPerEra;
	type MintFee = ConstU128<0>;
	type RelayChainCallBuilder = RelayChainCallBuilder<NoFeeRuntime, ParachainId>;
	type BaseWithdrawFee = BaseWithdrawFee;
	type HomaUnbondFee = ConstU128<0>;
	type RelayChainBlockNumber = MockRelayBlockNumberProvider;
	type ParachainAccount = ParachainAccount;
	type MaximumRedeemRequestMatchesForMint = ConstU32<100>;
	type RelayChainUnbondingSlashingSpans = ConstU32<5>;
	type MaxScheduledUnbonds = ConstU32<14>;
	type StakingUpdateFrequency = ConstU64<100>;
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<NoFeeRuntime>;
type Block = frame_system::mocking::MockBlock<NoFeeRuntime>;

frame_support::construct_runtime!(
	pub enum NoFeeRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		HomaLite: module_homa_lite::{Pallet, Call, Storage, Event<T>},
		PalletBalances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		Currencies: module_currencies::{Pallet, Call, Event<T>},
		PalletXcm: pallet_xcm::{Pallet, Call, Event<T>, Origin},
	}
);

pub struct ExtBuilder {
	tokens_balances: Vec<(AccountId, CurrencyId, Balance)>,
	native_balances: Vec<(AccountId, Balance)>,
}

#[allow(dead_code)]
impl ExtBuilder {
	pub fn empty() -> Self {
		Self {
			tokens_balances: vec![],
			native_balances: vec![],
		}
	}
}

impl Default for ExtBuilder {
	fn default() -> Self {
		let initial = dollar(INITIAL_BALANCE);
		Self {
			tokens_balances: vec![
				(ALICE, KSM, initial),
				(BOB, KSM, initial),
				(DAVE, LKSM, initial),
				(INVALID_CALLER, KSM, initial),
			],
			native_balances: vec![
				(ALICE, initial),
				(BOB, initial),
				(DAVE, initial),
				(INVALID_CALLER, initial),
			],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<NoFeeRuntime>()
			.unwrap();

		pallet_balances::GenesisConfig::<NoFeeRuntime> {
			balances: self.native_balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<NoFeeRuntime> {
			balances: self.tokens_balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
