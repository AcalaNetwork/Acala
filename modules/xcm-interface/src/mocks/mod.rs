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
	construct_runtime, derive_impl, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, Everything, Nothing},
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use orml_traits::xcm_transfer::Transferred;
use primitives::{CurrencyId, TokenSymbol};
use sp_runtime::{traits::IdentityLookup, AccountId32, BuildStorage};
use xcm_builder::{EnsureXcmOrigin, FixedWeightBounds, SignedToAccountId32};
use xcm_executor::traits::XcmAssetTransfers;

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
	pub UniversalLocation: InteriorLocation =
		Parachain(2000).into();
}

ord_parameter_types! {
	pub const One: AccountId = ALICE;
}

parameter_types! {
	pub const GetStakingCurrencyId: CurrencyId = DOT;
	pub const ParachainAccount: AccountId = AccountId32::new([0u8; 32]);
	pub const ParachainId: module_relaychain::ParaId = module_relaychain::ParaId::new(2000);
	pub SelfLocation: Location = Location::new(1, Parachain(ParachainId::get().into()));
}

pub struct SubAccountIndexLocationConvertor;
impl Convert<u16, Location> for SubAccountIndexLocationConvertor {
	fn convert(_sub_account_index: u16) -> Location {
		(Parent, Parachain(2000)).into()
	}
}

pub struct MockXcmTransfer;
impl XcmTransfer<AccountId, Balance, CurrencyId> for MockXcmTransfer {
	fn transfer(
		_who: AccountId,
		_currency_id: CurrencyId,
		_amount: Balance,
		_dest: Location,
		_dest_weight_limit: WeightLimit,
	) -> Result<Transferred<AccountId32>, DispatchError> {
		unimplemented!()
	}

	/// Transfer `Asset`
	fn transfer_multiasset(
		_who: AccountId,
		_asset: Asset,
		_dest: Location,
		_dest_weight_limit: WeightLimit,
	) -> Result<Transferred<AccountId32>, DispatchError> {
		unimplemented!()
	}

	fn transfer_with_fee(
		_who: AccountId,
		_currency_id: CurrencyId,
		_amount: Balance,
		_fee: Balance,
		_dest: Location,
		_dest_weight_limit: WeightLimit,
	) -> Result<Transferred<AccountId>, DispatchError> {
		unimplemented!()
	}

	/// Transfer `AssetWithFee`
	fn transfer_multiasset_with_fee(
		_who: AccountId,
		_asset: Asset,
		_fee: Asset,
		_dest: Location,
		_dest_weight_limit: WeightLimit,
	) -> Result<Transferred<AccountId32>, DispatchError> {
		unimplemented!()
	}

	fn transfer_multicurrencies(
		_who: AccountId,
		_currencies: Vec<(CurrencyId, Balance)>,
		_fee_item: u32,
		_dest: Location,
		_dest_weight_limit: WeightLimit,
	) -> Result<Transferred<AccountId32>, DispatchError> {
		unimplemented!()
	}

	fn transfer_multiassets(
		_who: AccountId,
		_assets: Assets,
		_fee: Asset,
		_dest: Location,
		_dest_weight_limit: WeightLimit,
	) -> Result<Transferred<AccountId32>, DispatchError> {
		unimplemented!()
	}
}

pub struct AccountIdToLocation;
impl Convert<AccountId, Location> for AccountIdToLocation {
	fn convert(account: AccountId) -> Location {
		Junction::AccountId32 {
			network: None,
			id: account.into(),
		}
		.into()
	}
}

pub enum Weightless {}
impl PreparedMessage for Weightless {
	fn weight_of(&self) -> Weight {
		unreachable!()
	}
}

pub struct MockExec;
impl<T> ExecuteXcm<T> for MockExec {
	type Prepared = Weightless;

	fn prepare(_message: Xcm<T>) -> Result<Self::Prepared, Xcm<T>> {
		unreachable!()
	}

	fn execute(_origin: impl Into<Location>, _pre: Weightless, _hash: &mut XcmHash, _weight_credit: Weight) -> Outcome {
		unreachable!()
	}

	fn prepare_and_execute(
		_origin: impl Into<Location>,
		message: Xcm<T>,
		_id: &mut XcmHash,
		weight_limit: Weight,
		_weight_credit: Weight,
	) -> Outcome {
		let o = match (message.0.len(), &message.0.first()) {
			(
				1,
				Some(Transact {
					require_weight_at_most, ..
				}),
			) => {
				if require_weight_at_most.all_lte(weight_limit) {
					Outcome::Complete {
						used: *require_weight_at_most,
					}
				} else {
					Outcome::Error {
						error: XcmError::WeightLimitReached(*require_weight_at_most),
					}
				}
			}
			// use 1000 to decide that it's not supported.
			_ => Outcome::Incomplete {
				used: Weight::from_parts(1000, 1000).min(weight_limit),
				error: XcmError::Unimplemented,
			},
		};
		o
	}

	fn charge_fees(_location: impl Into<Location>, _fees: Assets) -> XcmResult {
		Err(XcmError::Unimplemented)
	}
}

impl XcmAssetTransfers for MockExec {
	type IsReserve = ();
	type IsTeleporter = ();
	type AssetTransactor = ();
}

#[macro_export]
macro_rules! impl_mock {
	($relaychain:ty) => {
		pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;
		pub type Block = frame_system::mocking::MockBlock<Runtime>;

		#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
		impl frame_system::Config for Runtime {
			type AccountId = AccountId;
			type Lookup = IdentityLookup<Self::AccountId>;
			type Block = Block;
			type AccountData = pallet_balances::AccountData<Balance>;
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
			type MaxFreezes = ();
		}

		impl pallet_xcm::Config for Runtime {
			type RuntimeEvent = RuntimeEvent;
			type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
			type XcmRouter = ();
			type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
			type XcmExecuteFilter = Everything;
			type XcmExecutor = MockExec;
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
			type SovereignSubAccountLocationConvert = SubAccountIndexLocationConvertor;
			type RelayChainCallBuilder = module_relaychain::RelayChainCallBuilder<ParachainId, $relaychain>;
			type XcmTransfer = MockXcmTransfer;
			type SelfLocation = SelfLocation;
			type AccountIdToLocation = AccountIdToLocation;
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
