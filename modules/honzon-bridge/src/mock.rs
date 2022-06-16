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

//! Mocks for Honzon Bridge module.

#![cfg(test)]

pub use crate as module_honzon_bridge;

pub use frame_support::{
	construct_runtime,
	pallet_prelude::GenesisBuild,
	parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Everything, Nothing},
};
pub use frame_system::{EnsureRoot, EnsureSignedBy, RawOrigin};
pub use module_honzon_bridge::*;
pub use module_support::mocks::MockAddressMapping;
pub use orml_traits::parameter_type_with_key;
use sp_core::H160;

pub use primitives::{AccountId, Amount, BlockNumber, TokenSymbol};

/// For testing only. Does not check for overflow.
pub fn dollar(b: Balance) -> Balance {
	b * 1_000_000_000_000
}
pub const ALICE: AccountId = AccountId::new([1u8; 32]);
pub const INITIAL_BALANCE: Balance = 1_000_000;
pub const ACALA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const KUSD: CurrencyId = CurrencyId::Token(TokenSymbol::KUSD);

impl frame_system::Config for Runtime {
	type BaseCallFilter = Everything;
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Call = Call;
	type Hash = sp_runtime::testing::H256;
	type Hashing = sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = sp_runtime::traits::IdentityLookup<Self::AccountId>;
	type Header = sp_runtime::testing::Header;
	type Event = Event;
	type BlockHashCount = ConstU64<250>;
	type BlockWeights = ();
	type BlockLength = ();
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
impl orml_tokens::Config for Runtime {
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

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ConstU128<0>;
	type AccountStore = frame_system::Pallet<Runtime>;
	type MaxLocks = ();
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

pub type AdaptedBasicCurrency = module_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Amount, BlockNumber>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACALA;
	pub Erc20HoldingAccount: H160 = H160::from_low_u64_be(1);
}

impl module_currencies::Config for Runtime {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type Erc20HoldingAccount = Erc20HoldingAccount;
	type WeightInfo = ();
	type AddressMapping = MockAddressMapping;
	type EVMBridge = ();
	type GasToWeight = ();
	type SweepOrigin = EnsureRoot<AccountId>;
	type OnDust = ();
}

parameter_types! {
	pub const StablecoinCurrencyId: CurrencyId = AUSD;
	pub const BridgedStableCoinCurrencyId: CurrencyId = KUSD;
	pub const HonzonBridgePalletId: PalletId = PalletId(*b"aca/hzbg");
}

impl Config for Runtime {
	type Event = Event;
	type WeightInfo = ();
	type Currency = Currencies;
	type StablecoinCurrencyId = StablecoinCurrencyId;
	type BridgedStableCoinCurrencyId = BridgedStableCoinCurrencyId;
	type PalletId = HonzonBridgePalletId;
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system::{Pallet, Call, Event<T>},
		PalletBalances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		Currencies: module_currencies::{Pallet, Call, Event<T>},
		HonzonBridge: module_honzon_bridge::{Pallet, Call, Event<T>},
	}
);

#[allow(dead_code)]
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
				(ALICE, KUSD, initial),
				(ALICE, AUSD, initial),
				(HonzonBridge::account_id(), AUSD, initial),
				(HonzonBridge::account_id(), KUSD, initial),
			],
			native_balances: vec![(ALICE, initial), (HonzonBridge::account_id(), initial)],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self.native_balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self.tokens_balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
