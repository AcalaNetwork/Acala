// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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

//! Mocks for the Homa module.

#![cfg(test)]

use super::*;
use cumulus_primitives_core::ParaId;
use frame_support::{
	ord_parameter_types, parameter_types,
	traits::{Everything, Nothing},
};
use frame_system::{EnsureRoot, EnsureSignedBy, RawOrigin};
use module_relaychain::RelayChainCallBuilder;
use module_support::mocks::MockAddressMapping;
use orml_traits::{parameter_type_with_key, XcmTransfer};
use primitives::{Amount, TokenSymbol};
use sp_core::H256;
use sp_io::hashing::blake2_256;
use sp_runtime::{testing::Header, traits::IdentityLookup, AccountId32};
use xcm::latest::prelude::*;
use xcm_executor::traits::{InvertLocation, WeightBounds};

pub type AccountId = AccountId32;
pub type BlockNumber = u64;

mod homa {
	pub use super::super::*;
}

pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);
pub const CHARLIE: AccountId = AccountId32::new([3u8; 32]);
pub const DAVE: AccountId = AccountId32::new([255u8; 32]);
pub const INVALID_CALLER: AccountId = AccountId32::new([254u8; 32]);
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);
pub const MOCK_XCM_ACCOUNTID: AccountId = AccountId32::new([255u8; 32]);
pub const PARACHAIN_ID: u32 = 2000;

/// For testing only. Does not check for overflow.
pub fn dollar(b: Balance) -> Balance {
	b * 1_000_000_000_000
}

/// For testing only. Does not check for overflow.
pub fn cent(b: Balance) -> Balance {
	b * 10_000_000_000
}

/// mock XCM transfer.
pub struct MockXcmTransfer;
impl XcmTransfer<AccountId, Balance, CurrencyId> for MockXcmTransfer {
	fn transfer(
		who: AccountId,
		currency_id: CurrencyId,
		amount: Balance,
		dest: MultiLocation,
		_dest_weight: Weight,
	) -> DispatchResult {
		match who {
			INVALID_CALLER => Err(DispatchError::Other("invalid caller")),
			_ => Ok(()),
		}?;
		match currency_id {
			ACA => Err(DispatchError::Other("unacceptable currency id")),
			_ => Ok(()),
		}?;

		Currencies::withdraw(ACA, &who, amount)
	}

	fn transfer_multi_asset(
		_who: AccountId,
		_asset: MultiAsset,
		_dest: MultiLocation,
		_dest_weight: Weight,
	) -> DispatchResult {
		unimplemented!()
	}
}

/// mock XCM.
pub struct MockXcm;
impl InvertLocation for MockXcm {
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
		Origin::from(RawOrigin::Signed(Default::default()))
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

impl pallet_xcm::Config for Runtime {
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

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Runtime {
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
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

impl orml_tokens::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type MaxLocks = ();
	type DustRemovalWhitelist = Nothing;
}

parameter_types! {
	pub const NativeTokenExistentialDeposit: Balance = 0;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = NativeTokenExistentialDeposit;
	type AccountStore = frame_system::Pallet<Runtime>;
	type MaxLocks = ();
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

pub type AdaptedBasicCurrency = module_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
}

impl module_currencies::Config for Runtime {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
	type AddressMapping = MockAddressMapping;
	type EVMBridge = ();
	type SweepOrigin = EnsureRoot<AccountId>;
	type OnDust = ();
}

pub struct MockConvertor;
impl Convert<SubAccountIndex, MultiLocation> for MockConvertor {
	fn convert(index: SubAccountIndex) -> MultiLocation {
		let entropy = (b"modlpy/utilisuba", ParachainAccount::get(), index).using_encoded(blake2_256);
		let subaccount = AccountId32::decode(&mut &entropy[..]).unwrap_or_default();
		MultiLocation::new(
			1,
			X1(Junction::AccountId32 {
				network: NetworkId::Any,
				id: subaccount.into(),
			}),
		)
	}
}

ord_parameter_types! {
	pub const HomaAdmin: AccountId = DAVE;
}

parameter_types! {
	pub const LiquidCurrencyId: CurrencyId = LDOT;
	pub const HomaPalletId: PalletId = PalletId(*b"aca/homa");
	pub MintThreshold: Balance = dollar(1);
	pub RedeemThreshold: Balance = dollar(10);
	pub DefaultExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(1, 10);
	pub ParachainAccount: AccountId = AccountId32::new([250u8; 32]);
	pub ActiveSubAccountsIndexList: Vec<SubAccountIndex> = vec![0, 1, 2];
	pub SoftBondedCapPerSubAccount: Balance = dollar(10);
	pub FastMatchKeepers: Vec<AccountId> = vec![CHARLIE, DAVE];
	pub const BondingDuration: EraIndex = 28;
	pub const RelayChainUnbondingSlashingSpans: EraIndex = 7;
	pub EstimatedRewardRatePerEra: Rate = Rate::saturating_from_rational(1, 100);
	pub XcmTransferFee: Balance = cent(50);
	pub XcmMessageFee: Balance = cent(50);
	pub ParachainId: ParaId = ParaId::from(PARACHAIN_ID);
}

impl Config for Runtime {
	type Event = Event;
	type Currency = Currencies;
	type GovernanceOrigin = EnsureSignedBy<HomaAdmin, AccountId>;
	type StakingCurrencyId = GetNativeCurrencyId;
	type LiquidCurrencyId = LiquidCurrencyId;
	type PalletId = HomaPalletId;
	type DefaultExchangeRate = DefaultExchangeRate;
	type MintThreshold = MintThreshold;
	type RedeemThreshold = RedeemThreshold;
	type ParachainAccount = ParachainAccount;
	type ActiveSubAccountsIndexList = ActiveSubAccountsIndexList;
	type SoftBondedCapPerSubAccount = SoftBondedCapPerSubAccount;
	type FastMatchKeepers = FastMatchKeepers;
	type BondingDuration = BondingDuration;
	type RelayChainUnbondingSlashingSpans = RelayChainUnbondingSlashingSpans;
	type EstimatedRewardRatePerEra = EstimatedRewardRatePerEra;
	type XcmTransferFee = XcmTransferFee;
	type XcmMessageFee = XcmMessageFee;
	type RelayChainCallBuilder = RelayChainCallBuilder<Runtime, ParachainId>;
	type XcmTransfer = MockXcmTransfer;
	type SovereignSubAccountLocationConvert = MockConvertor;
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Homa: homa::{Pallet, Call, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		Currencies: module_currencies::{Pallet, Call, Event<T>},
		PalletXcm: pallet_xcm::{Pallet, Call, Event<T>, Origin},
	}
);

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self { balances: vec![] }
	}
}

impl ExtBuilder {
	pub fn balances(mut self, balances: Vec<(AccountId, CurrencyId, Balance)>) -> Self {
		self.balances = balances;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self
				.balances
				.clone()
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id == DOT)
				.map(|(account_id, _, initial_balance)| (account_id, initial_balance))
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self
				.balances
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id != DOT)
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
