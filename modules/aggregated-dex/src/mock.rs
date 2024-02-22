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

//! Mocks for the Aggregated DEX module.

#![cfg(test)]

use super::*;
use frame_support::{
	derive_impl, match_types, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Nothing},
	PalletId,
};
use frame_system::EnsureSignedBy;
pub use module_support::{ExchangeRate, RebasedStableAsset};
use orml_tokens::ConvertBalance;
pub use orml_traits::{parameter_type_with_key, MultiCurrency};
use primitives::{Amount, TokenSymbol, TradingPair};
use sp_runtime::{traits::IdentityLookup, AccountId32, ArithmeticError, BuildStorage, FixedPointNumber};

pub type AccountId = AccountId32;

mod aggregated_dex {
	pub use super::super::*;
}

pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);
pub const STABLE_ASSET: CurrencyId = CurrencyId::StableAssetPoolToken(0);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = pallet_balances::AccountData<Balance>;
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
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = Nothing;
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
	type RuntimeEvent = RuntimeEvent;
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
impl ConvertBalance<Balance, Balance> for ConvertBalanceHoma {
	type AssetId = CurrencyId;

	fn convert_balance(balance: Balance, asset_id: CurrencyId) -> sp_std::result::Result<Balance, ArithmeticError> {
		match asset_id {
			LDOT => ExchangeRate::saturating_from_rational(1, 10)
				.checked_mul_int(balance)
				.ok_or(ArithmeticError::Overflow),
			_ => Ok(balance),
		}
	}

	fn convert_balance_back(
		balance: Balance,
		asset_id: CurrencyId,
	) -> sp_std::result::Result<Balance, ArithmeticError> {
		match asset_id {
			LDOT => ExchangeRate::saturating_from_rational(10, 1)
				.checked_mul_int(balance)
				.ok_or(ArithmeticError::Overflow),
			_ => Ok(balance),
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
	type RuntimeEvent = RuntimeEvent;
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
	type StableAsset = StableAssetWrapper;
	type GovernanceOrigin = EnsureSignedBy<Admin, AccountId>;
	type DexSwapJointList = DexSwapJointList;
	type SwapPathLimit = ConstU32<3>;
	type WeightInfo = ();
}

pub type StableAssetWrapper =
	RebasedStableAsset<StableAsset, ConvertBalanceHoma, RebasedStableAssetErrorConvertor<Runtime>>;

type Block = frame_system::mocking::MockBlock<Runtime>;

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		AggregatedDex: aggregated_dex,
		Dex: module_dex,
		Tokens: orml_tokens,
		StableAsset: nutsfinance_stable_asset,
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
		let mut t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self.balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
