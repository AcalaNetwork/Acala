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

#![cfg(test)]

use super::*;
use crate as xcm_interface;
use frame_support::{
	construct_runtime, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Everything, Nothing},
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use orml_traits::xcm_transfer::Transferred;
use primitives::{CurrencyId, TokenSymbol};
use sp_core::H256;
use sp_runtime::{traits::IdentityLookup, AccountId32, BuildStorage};
use xcm_builder::{EnsureXcmOrigin, FixedWeightBounds, SignedToAccountId32};

pub mod kusama;
pub mod polkadot;

pub type AccountId = AccountId32;

pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);

parameter_types! {
	pub const UnitWeightCost: XcmWeight = XcmWeight::from_parts(10, 10);
	pub const BaseXcmWeight: XcmWeight = XcmWeight::from_parts(100_000_000, 100_000_000);
	pub const MaxInstructions: u32 = 100;
	pub const MaxAssetsIntoHolding: u32 = 64;
}

parameter_types! {
	pub const RelayNetwork: NetworkId = NetworkId::Polkadot;
	pub UniversalLocation: InteriorMultiLocation =
		X1(Parachain(2000).into());
}

ord_parameter_types! {
	pub const One: AccountId = ALICE;
}

parameter_types! {
	pub const GetStakingCurrencyId: CurrencyId = DOT;
	pub const ParachainAccount: AccountId = AccountId32::new([0u8; 32]);
	pub const ParachainId: module_relaychain::ParaId = module_relaychain::ParaId::new(2000);
	pub SelfLocation: MultiLocation = MultiLocation::new(1, X1(Parachain(ParachainId::get().into())));
}

pub struct SubAccountIndexMultiLocationConvertor;
impl Convert<u16, MultiLocation> for SubAccountIndexMultiLocationConvertor {
	fn convert(_sub_account_index: u16) -> MultiLocation {
		(Parent, Parachain(2000)).into()
	}
}

pub struct MockXcmTransfer;
impl XcmTransfer<AccountId, Balance, CurrencyId> for MockXcmTransfer {
	fn transfer(
		_who: AccountId,
		_currency_id: CurrencyId,
		_amount: Balance,
		_dest: MultiLocation,
		_dest_weight_limit: WeightLimit,
	) -> Result<Transferred<AccountId32>, DispatchError> {
		unimplemented!()
	}

	/// Transfer `MultiAsset`
	fn transfer_multiasset(
		_who: AccountId,
		_asset: MultiAsset,
		_dest: MultiLocation,
		_dest_weight_limit: WeightLimit,
	) -> Result<Transferred<AccountId32>, DispatchError> {
		unimplemented!()
	}

	fn transfer_with_fee(
		_who: AccountId,
		_currency_id: CurrencyId,
		_amount: Balance,
		_fee: Balance,
		_dest: MultiLocation,
		_dest_weight_limit: WeightLimit,
	) -> Result<Transferred<AccountId>, DispatchError> {
		unimplemented!()
	}

	/// Transfer `MultiAssetWithFee`
	fn transfer_multiasset_with_fee(
		_who: AccountId,
		_asset: MultiAsset,
		_fee: MultiAsset,
		_dest: MultiLocation,
		_dest_weight_limit: WeightLimit,
	) -> Result<Transferred<AccountId32>, DispatchError> {
		unimplemented!()
	}

	fn transfer_multicurrencies(
		_who: AccountId,
		_currencies: Vec<(CurrencyId, Balance)>,
		_fee_item: u32,
		_dest: MultiLocation,
		_dest_weight_limit: WeightLimit,
	) -> Result<Transferred<AccountId32>, DispatchError> {
		unimplemented!()
	}

	fn transfer_multiassets(
		_who: AccountId,
		_assets: MultiAssets,
		_fee: MultiAsset,
		_dest: MultiLocation,
		_dest_weight_limit: WeightLimit,
	) -> Result<Transferred<AccountId32>, DispatchError> {
		unimplemented!()
	}
}

pub struct AccountIdToMultiLocation;
impl Convert<AccountId, MultiLocation> for AccountIdToMultiLocation {
	fn convert(account: AccountId) -> MultiLocation {
		X1(Junction::AccountId32 {
			network: None,
			id: account.into(),
		})
		.into()
	}
}

#[macro_export]
macro_rules! impl_mock {
	($relaychain:ty) => {
		pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;
		pub type Block = frame_system::mocking::MockBlock<Runtime>;

		impl frame_system::Config for Runtime {
			type RuntimeOrigin = RuntimeOrigin;
			type Nonce = u64;
			type RuntimeCall = RuntimeCall;
			type Hash = H256;
			type Hashing = ::sp_runtime::traits::BlakeTwo256;
			type AccountId = AccountId;
			type Lookup = IdentityLookup<Self::AccountId>;
			type Block = Block;
			type RuntimeEvent = RuntimeEvent;
			type BlockHashCount = ConstU64<250>;
			type BlockWeights = ();
			type BlockLength = ();
			type Version = ();
			type PalletInfo = PalletInfo;
			type AccountData = pallet_balances::AccountData<Balance>;
			type OnNewAccount = ();
			type OnKilledAccount = ();
			type DbWeight = ();
			type BaseCallFilter = Everything;
			type SystemWeightInfo = ();
			type SS58Prefix = ();
			type OnSetCode = ();
			type MaxConsumers = ConstU32<16>;
		}

		impl pallet_balances::Config for Runtime {
			type Balance = Balance;
			type DustRemoval = ();
			type RuntimeEvent = RuntimeEvent;
			type ExistentialDeposit = ConstU128<1>;
			type AccountStore = System;
			type MaxLocks = ();
			type MaxReserves = ();
			type ReserveIdentifier = [u8; 8];
			type WeightInfo = ();
			type RuntimeHoldReason = RuntimeHoldReason;
			type RuntimeFreezeReason = RuntimeFreezeReason;
			type FreezeIdentifier = ();
			type MaxHolds = ();
			type MaxFreezes = ();
		}

		impl pallet_xcm::Config for Runtime {
			type RuntimeEvent = RuntimeEvent;
			type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
			type XcmRouter = ();
			type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
			type XcmExecuteFilter = Everything;
			type XcmExecutor = ();
			type XcmTeleportFilter = Nothing;
			type XcmReserveTransferFilter = Everything;
			type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
			type UniversalLocation = UniversalLocation;
			type RuntimeOrigin = RuntimeOrigin;
			type RuntimeCall = RuntimeCall;
			const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
			type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
			type Currency = Balances;
			type CurrencyMatcher = ();
			type TrustedLockers = ();
			type SovereignAccountOf = ();
			type MaxLockers = ConstU32<8>;
			type WeightInfo = pallet_xcm::TestWeightInfo;
			type AdminOrigin = EnsureRoot<AccountId>;
			type MaxRemoteLockConsumers = ConstU32<0>;
			type RemoteLockConsumerIdentifier = ();
		}

		impl Config for Runtime {
			type RuntimeEvent = RuntimeEvent;
			type UpdateOrigin = EnsureSignedBy<One, AccountId>;
			type StakingCurrencyId = GetStakingCurrencyId;
			type ParachainAccount = ParachainAccount;
			type RelayChainUnbondingSlashingSpans = ConstU32<28>;
			type SovereignSubAccountLocationConvert = SubAccountIndexMultiLocationConvertor;
			type RelayChainCallBuilder = module_relaychain::RelayChainCallBuilder<ParachainId, $relaychain>;
			type XcmTransfer = MockXcmTransfer;
			type SelfLocation = SelfLocation;
			type AccountIdToMultiLocation = AccountIdToMultiLocation;
		}

		construct_runtime!(
			pub enum Runtime {
				System: frame_system,
				Balances: pallet_balances,
				PolkadotXcm: pallet_xcm,
				XcmInterface: xcm_interface,
			}
		);
	}
}

pub struct ExtBuilder;

impl Default for ExtBuilder {
	fn default() -> Self {
		ExtBuilder
	}
}

impl ExtBuilder {
	pub fn build<Runtime: frame_system::Config>(self) -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| frame_system::Pallet::<Runtime>::set_block_number(1u32.into()));
		ext
	}
}
