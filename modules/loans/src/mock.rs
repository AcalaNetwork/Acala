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

//! Mocks for the loans module.

#![cfg(test)]

use super::*;
use frame_support::{
	construct_runtime, derive_impl, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, Nothing},
	PalletId,
};
use frame_system::EnsureSignedBy;
use module_support::{mocks::MockStableAsset, AuctionManager, RiskManager, SpecificJointsSwap};
use orml_traits::parameter_type_with_key;
use primitives::TokenSymbol;
use sp_runtime::{
	traits::{AccountIdConversion, IdentityLookup},
	BuildStorage,
};
use std::collections::HashMap;

pub type AccountId = u128;
pub type AuctionId = u32;
pub type BlockNumber = u64;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const BTC: CurrencyId = CurrencyId::ForeignAsset(255);

mod loans {
	pub use super::super::*;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = pallet_balances::AccountData<Balance>;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		100
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

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = frame_system::Pallet<Runtime>;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = ();
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
}

impl orml_currencies::Config for Runtime {
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}
pub type AdaptedBasicCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Amount, BlockNumber>;

pub struct MockAuctionManager;
impl AuctionManager<AccountId> for MockAuctionManager {
	type CurrencyId = CurrencyId;
	type Balance = Balance;
	type AuctionId = AuctionId;

	fn new_collateral_auction(
		_refund_recipient: &AccountId,
		_currency_id: Self::CurrencyId,
		_amount: Self::Balance,
		_target: Self::Balance,
	) -> DispatchResult {
		Ok(())
	}

	fn cancel_auction(_id: Self::AuctionId) -> DispatchResult {
		Ok(())
	}

	fn get_total_target_in_auction() -> Self::Balance {
		Default::default()
	}

	fn get_total_collateral_in_auction(_id: Self::CurrencyId) -> Self::Balance {
		Default::default()
	}
}

ord_parameter_types! {
	pub const One: AccountId = 1;
}

parameter_types! {
	pub const GetStableCurrencyId: CurrencyId = AUSD;
	pub const CDPTreasuryPalletId: PalletId = PalletId(*b"aca/cdpt");
	pub TreasuryAccount: AccountId = PalletId(*b"aca/hztr").into_account_truncating();
	pub AlternativeSwapPathJointList: Vec<Vec<CurrencyId>> = vec![];
}

impl module_cdp_treasury::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = MockAuctionManager;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type DEX = ();
	type Swap = SpecificJointsSwap<(), AlternativeSwapPathJointList>;
	type MaxAuctionsCount = ConstU32<10_000>;
	type PalletId = CDPTreasuryPalletId;
	type TreasuryAccount = TreasuryAccount;
	type WeightInfo = ();
	type StableAsset = MockStableAsset<CurrencyId, Balance, AccountId, BlockNumber>;
}

// mock risk manager
pub struct MockRiskManager;
impl RiskManager<AccountId, CurrencyId, Balance, Balance> for MockRiskManager {
	fn get_debit_value(_currency_id: CurrencyId, debit_balance: Balance) -> Balance {
		debit_balance / Balance::from(2u64)
	}

	fn check_position_valid(
		currency_id: CurrencyId,
		_collateral_balance: Balance,
		_debit_balance: Balance,
		check_required_ratio: bool,
	) -> DispatchResult {
		match currency_id {
			DOT => {
				if check_required_ratio {
					Err(sp_runtime::DispatchError::Other(
						"mock below required collateral ratio error",
					))
				} else {
					Err(sp_runtime::DispatchError::Other("mock below liquidation ratio error"))
				}
			}
			BTC => Ok(()),
			_ => Err(sp_runtime::DispatchError::Other("mock below liquidation ratio error")),
		}
	}

	fn check_debit_cap(currency_id: CurrencyId, total_debit_balance: Balance) -> DispatchResult {
		match (currency_id, total_debit_balance) {
			(DOT, 1000) => Err(sp_runtime::DispatchError::Other("mock exceed debit value cap error")),
			(BTC, 1000) => Err(sp_runtime::DispatchError::Other("mock exceed debit value cap error")),
			(_, _) => Ok(()),
		}
	}
}

parameter_types! {
	pub static DotShares: HashMap<AccountId, Balance> = HashMap::new();
}

pub struct MockOnUpdateLoan;
impl Handler<(AccountId, CurrencyId, Amount, Balance)> for MockOnUpdateLoan {
	fn handle(info: &(AccountId, CurrencyId, Amount, Balance)) -> DispatchResult {
		let (who, currency_id, adjustment, previous_amount) = info;
		let adjustment_abs = TryInto::<Balance>::try_into(adjustment.saturating_abs()).unwrap_or_default();
		let new_share_amount = if adjustment.is_positive() {
			previous_amount.saturating_add(adjustment_abs)
		} else {
			previous_amount.saturating_sub(adjustment_abs)
		};

		if *currency_id == DOT {
			DotShares::mutate(|v| {
				let mut old_map = v.clone();
				old_map.insert(*who, new_share_amount);
				*v = old_map;
			});
		}
		return Ok(());
	}
}

parameter_types! {
	pub const LoansPalletId: PalletId = PalletId(*b"aca/loan");
}

impl Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type RiskManager = MockRiskManager;
	type CDPTreasury = CDPTreasuryModule;
	type PalletId = LoansPalletId;
	type OnUpdateLoan = MockOnUpdateLoan;
}

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		LoansModule: loans,
		Tokens: orml_tokens,
		PalletBalances: pallet_balances,
		Currencies: orml_currencies,
		CDPTreasuryModule: module_cdp_treasury,
	}
);

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			balances: vec![
				(ALICE, DOT, 1000),
				(ALICE, BTC, 1000),
				(BOB, DOT, 1000),
				(BOB, BTC, 1000),
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
