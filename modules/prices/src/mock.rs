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

//! Mocks for the prices module.

#![cfg(test)]

use super::*;
use frame_support::{construct_runtime, derive_impl, ord_parameter_types, parameter_types, traits::Nothing};
use frame_system::EnsureSignedBy;
use module_support::{mocks::MockErc20InfoMapping, ExchangeRate, SwapLimit};
use orml_traits::{parameter_type_with_key, DataFeeder};
use primitives::{currency::DexShare, Amount, TokenSymbol};
use sp_core::H160;
use sp_runtime::{
	traits::{IdentityLookup, One as OneT, Zero},
	BuildStorage, DispatchError, FixedPointNumber,
};

pub type AccountId = u128;
pub type BlockNumber = u64;

pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const TAI: CurrencyId = CurrencyId::Token(TokenSymbol::TAI);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);
pub const KSM: CurrencyId = CurrencyId::Token(TokenSymbol::KSM);
pub const TAIKSM: CurrencyId = CurrencyId::StableAssetPoolToken(0);
pub const LP_AUSD_DOT: CurrencyId =
	CurrencyId::DexShare(DexShare::Token(TokenSymbol::AUSD), DexShare::Token(TokenSymbol::DOT));
pub const LIQUID_CROWDLOAN_LEASE_1: CurrencyId = CurrencyId::LiquidCrowdloan(1);
pub const LIQUID_CROWDLOAN_LEASE_2: CurrencyId = CurrencyId::LiquidCrowdloan(2);
pub const LIQUID_CROWDLOAN_LEASE_3: CurrencyId = CurrencyId::LiquidCrowdloan(3);

mod prices {
	pub use super::super::*;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = ();
}

parameter_types! {
	static Changed: bool = false;
}

pub fn mock_oracle_update() {
	Changed::mutate(|v| *v = true)
}

pub struct MockDataProvider;
impl DataProvider<CurrencyId, Price> for MockDataProvider {
	fn get(currency_id: &CurrencyId) -> Option<Price> {
		if Changed::get() {
			match *currency_id {
				AUSD => None,
				TAI => Some(Price::saturating_from_integer(40000)),
				DOT => Some(Price::saturating_from_integer(10)),
				ACA => Some(Price::saturating_from_integer(30)),
				KSM => Some(Price::saturating_from_integer(200)),
				_ => None,
			}
		} else {
			match *currency_id {
				AUSD => Some(Price::saturating_from_rational(99, 100)),
				TAI => Some(Price::saturating_from_integer(50000)),
				DOT => Some(Price::saturating_from_integer(100)),
				ACA => Some(Price::zero()),
				KSM => None,
				_ => None,
			}
		}
	}
}

impl DataFeeder<CurrencyId, Price, AccountId> for MockDataProvider {
	fn feed_value(_: Option<AccountId>, _: CurrencyId, _: Price) -> sp_runtime::DispatchResult {
		Ok(())
	}
}

pub struct MockLiquidStakingExchangeProvider;
impl ExchangeRateProvider for MockLiquidStakingExchangeProvider {
	fn get_exchange_rate() -> ExchangeRate {
		if CHANGED.with(|v| *v.borrow_mut()) {
			ExchangeRate::saturating_from_rational(3, 5)
		} else {
			ExchangeRate::saturating_from_rational(1, 2)
		}
	}
}

pub struct MockDEX;
impl DEXManager<AccountId, Balance, CurrencyId> for MockDEX {
	fn get_liquidity_pool(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> (Balance, Balance) {
		match (currency_id_a, currency_id_b) {
			(AUSD, DOT) => (10000, 200),
			_ => (0, 0),
		}
	}

	fn get_liquidity_token_address(_currency_id_a: CurrencyId, _currency_id_b: CurrencyId) -> Option<H160> {
		unimplemented!()
	}

	fn get_swap_amount(_: &[CurrencyId], _: SwapLimit<Balance>) -> Option<(Balance, Balance)> {
		unimplemented!()
	}

	fn get_best_price_swap_path(
		_: CurrencyId,
		_: CurrencyId,
		_: SwapLimit<Balance>,
		_: Vec<Vec<CurrencyId>>,
	) -> Option<(Vec<CurrencyId>, Balance, Balance)> {
		unimplemented!()
	}

	fn swap_with_specific_path(
		_: &AccountId,
		_: &[CurrencyId],
		_: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		unimplemented!()
	}

	fn add_liquidity(
		_who: &AccountId,
		_currency_id_a: CurrencyId,
		_currency_id_b: CurrencyId,
		_max_amount_a: Balance,
		_max_amount_b: Balance,
		_min_share_increment: Balance,
		_stake_increment_share: bool,
	) -> sp_std::result::Result<(Balance, Balance, Balance), DispatchError> {
		unimplemented!()
	}

	fn remove_liquidity(
		_who: &AccountId,
		_currency_id_a: CurrencyId,
		_currency_id_b: CurrencyId,
		_remove_share: Balance,
		_min_withdrawn_a: Balance,
		_min_withdrawn_b: Balance,
		_by_unstake: bool,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		unimplemented!()
	}
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

impl BlockNumberProvider for MockRelayBlockNumberProvider {
	type BlockNumber = BlockNumber;

	fn current_block_number() -> Self::BlockNumber {
		Self::get()
	}
}

parameter_type_with_key! {
	pub LiquidCrowdloanLeaseBlockNumber: |lease: Lease| -> Option<BlockNumber> {
		#[allow(clippy::match_ref_pats)] // false positive
		match lease {
			&1 => Some(100),
			&2 => Some(200),
			_ => None,
		}
	};
}

parameter_type_with_key! {
	pub PricingPegged: |currency_id: CurrencyId| -> Option<CurrencyId> {
		#[allow(clippy::match_ref_pats)] // false positive
		match currency_id {
			&TAIKSM => Some(KSM),
			_ => None,
		}
	};
}

ord_parameter_types! {
	pub const One: AccountId = 1;
}

parameter_types! {
	pub const GetStableCurrencyId: CurrencyId = AUSD;
	pub const GetStakingCurrencyId: CurrencyId = DOT;
	pub const GetLiquidCurrencyId: CurrencyId = LDOT;
	pub StableCurrencyFixedPrice: Price = Price::one();
	pub static MockRelayBlockNumberProvider: BlockNumber = 0;
	pub RewardRatePerRelaychainBlock: Rate = Rate::saturating_from_rational(1, 1000);
}

impl Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Source = MockDataProvider;
	type GetStableCurrencyId = GetStableCurrencyId;
	type StableCurrencyFixedPrice = StableCurrencyFixedPrice;
	type GetStakingCurrencyId = GetStakingCurrencyId;
	type GetLiquidCurrencyId = GetLiquidCurrencyId;
	type LockOrigin = EnsureSignedBy<One, AccountId>;
	type LiquidStakingExchangeRateProvider = MockLiquidStakingExchangeProvider;
	type DEX = MockDEX;
	type Currency = Tokens;
	type Erc20InfoMapping = MockErc20InfoMapping;
	type LiquidCrowdloanLeaseBlockNumber = LiquidCrowdloanLeaseBlockNumber;
	type RelayChainBlockNumber = MockRelayBlockNumberProvider;
	type RewardRatePerRelaychainBlock = RewardRatePerRelaychainBlock;
	type PricingPegged = PricingPegged;
	type WeightInfo = ();
}

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		PricesModule: prices,
		Tokens: orml_tokens,
	}
);

pub struct ExtBuilder;

impl Default for ExtBuilder {
	fn default() -> Self {
		ExtBuilder
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		t.into()
	}
}
