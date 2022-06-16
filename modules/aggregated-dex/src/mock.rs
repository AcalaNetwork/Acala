// This file is part of Acala.

// Copyright (C) 2022 Acala Foundation.
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

//! Mocks for the Aggregated DEX module.

#![cfg(test)]

use super::*;
use frame_support::{
	match_types, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Everything, Nothing},
	PalletId,
};
use frame_system::EnsureSignedBy;
pub use orml_traits::{parameter_type_with_key, MultiCurrency};
use primitives::{Amount, TokenSymbol, TradingPair};
use sp_runtime::{
	testing::{Header, H256},
	traits::{Bounded, IdentityLookup},
	AccountId32, FixedPointNumber,
};
pub use support::ExchangeRate;

pub type AccountId = AccountId32;
pub type BlockNumber = u64;

mod aggregated_dex {
	pub use super::super::*;
}

pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);
pub const STABLE_ASSET: CurrencyId = CurrencyId::StableAssetPoolToken(0);

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
	type BlockHashCount = ConstU64<250>;
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

ord_parameter_types! {
	pub const Admin: AccountId = BOB;
}

parameter_types! {
	pub const DEXPalletId: PalletId = PalletId(*b"aca/dexm");
	pub const GetExchangeFee: (u32, u32) = (0, 100);
	pub EnabledTradingPairs: Vec<TradingPair> = vec![];
}

impl module_dex::Config for Runtime {
	type Event = Event;
	type Currency = Tokens;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = ConstU32<4>;
	type PalletId = DEXPalletId;
	type Erc20InfoMapping = ();
	type DEXIncentives = ();
	type WeightInfo = ();
	type ListingOrigin = EnsureSignedBy<Admin, AccountId>;
	type ExtendedProvisioningBlocks = ConstU64<0>;
	type OnLiquidityPoolUpdated = ();
}

pub struct EnsurePoolAssetId;
impl nutsfinance_stable_asset::traits::ValidateAssetId<CurrencyId> for EnsurePoolAssetId {
	fn validate(currency_id: CurrencyId) -> bool {
		matches!(currency_id, CurrencyId::StableAssetPoolToken(_))
	}
}

pub struct ConvertBalanceHoma;
impl orml_tokens::ConvertBalance<Balance, Balance> for ConvertBalanceHoma {
	type AssetId = CurrencyId;

	fn convert_balance(balance: Balance, asset_id: CurrencyId) -> Balance {
		match asset_id {
			LDOT => ExchangeRate::saturating_from_rational(1, 10)
				.checked_mul_int(balance)
				.unwrap_or(Bounded::max_value()),
			_ => balance,
		}
	}

	fn convert_balance_back(balance: Balance, asset_id: CurrencyId) -> Balance {
		match asset_id {
			LDOT => ExchangeRate::saturating_from_rational(10, 1)
				.checked_mul_int(balance)
				.unwrap_or(Bounded::max_value()),
			_ => balance,
		}
	}
}

match_types! {
	pub type IsLiquidToken: impl Contains<CurrencyId> = {
		CurrencyId::Token(TokenSymbol::LDOT)
	};
}

type RebaseTokens = orml_tokens::Combiner<
	AccountId,
	IsLiquidToken,
	orml_tokens::Mapper<AccountId, Tokens, ConvertBalanceHoma, Balance, GetLiquidCurrencyId>,
	Tokens,
>;

parameter_types! {
	pub const StableAssetPalletId: PalletId = PalletId(*b"nuts/sta");
}

impl nutsfinance_stable_asset::Config for Runtime {
	type Event = Event;
	type AssetId = CurrencyId;
	type Balance = Balance;
	type Assets = RebaseTokens;
	type PalletId = StableAssetPalletId;

	type AtLeast64BitUnsigned = u128;
	type FeePrecision = ConstU128<10_000_000_000>; // 10 decimals
	type APrecision = ConstU128<100>; // 2 decimals
	type PoolAssetLimit = ConstU32<5>;
	type SwapExactOverAmount = ConstU128<100>;
	type WeightInfo = ();
	type ListingOrigin = EnsureSignedBy<Admin, AccountId>;
	type EnsurePoolAssetId = EnsurePoolAssetId;
}

parameter_types! {
	pub static DexSwapJointList: Vec<Vec<CurrencyId>> = vec![];
	pub const GetLiquidCurrencyId: CurrencyId = LDOT;
}

impl Config for Runtime {
	type DEX = Dex;
	type StableAsset = StableAsset;
	type GovernanceOrigin = EnsureSignedBy<Admin, AccountId>;
	type DexSwapJointList = DexSwapJointList;
	type SwapPathLimit = ConstU32<3>;
	type RebaseTokenAmountConvertor = ConvertBalanceHoma;
	type WeightInfo = ();
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
		AggregatedDex: aggregated_dex::{Pallet, Call, Storage},
		Dex: module_dex::{Pallet, Call, Storage, Config<T>, Event<T>},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		StableAsset: nutsfinance_stable_asset::{Pallet, Call, Storage, Event<T>},
	}
);

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			balances: vec![
				(ALICE, DOT, 100_000_000_000),
				(BOB, AUSD, 1_000_000_000_000_000_000),
				(BOB, DOT, 1_000_000_000_000_000_000),
				(BOB, LDOT, 1_000_000_000_000_000_000),
			],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self.balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
