//! Mocks for staking pool module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_event, impl_outer_origin, ord_parameter_types, parameter_types};
use frame_system::EnsureSignedBy;
use orml_traits::parameter_type_with_key;
use primitives::Amount;
use primitives::TokenSymbol;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{CheckedAdd, CheckedMul, CheckedSub, IdentityLookup},
	FixedPointOperand, Perbill,
};
use sp_std::cell::RefCell;
use std::collections::HashMap;
use support::PolkadotStakingLedger;

pub type AccountId = u128;
pub type BlockNumber = u64;
pub type PolkadotAccountId = u128;

pub const ALICE: AccountId = 0;
pub const BOB: AccountId = 1;
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

mod staking_pool {
	pub use super::super::*;
}

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		frame_system<T>,
		staking_pool<T>,
		orml_tokens<T>,
		pallet_balances<T>,
		orml_currencies<T>,
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u32 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

impl frame_system::Config for Runtime {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Call = ();
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = TestEvent;
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();
	type PalletInfo = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = ();
	type BlockExecutionWeight = ();
	type ExtrinsicBaseWeight = ();
	type MaximumExtrinsicWeight = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
}
pub type System = frame_system::Module<Runtime>;

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}
impl orml_tokens::Config for Runtime {
	type Event = TestEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
}
pub type TokensModule = orml_tokens::Module<Runtime>;

parameter_types! {
	pub const ExistentialDeposit: Balance = 1;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = TestEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type MaxLocks = ();
	type WeightInfo = ();
}
type PalletBalances = pallet_balances::Module<Runtime>;
pub type NativeCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Amount, BlockNumber>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
}

impl orml_currencies::Config for Runtime {
	type Event = TestEvent;
	type MultiCurrency = TokensModule;
	type NativeCurrency = NativeCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}
pub type CurrenciesModule = orml_currencies::Module<Runtime>;

pub struct MockNomineesProvider;
impl NomineesProvider<PolkadotAccountId> for MockNomineesProvider {
	fn nominees() -> Vec<PolkadotAccountId> {
		vec![1, 2, 3]
	}
}

parameter_types! {
	pub const BondingDuration: EraIndex = 4;
	pub const EraLength: BlockNumber = 10;
}

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, Default)]
pub struct Status {
	pub bonded: Balance,
	pub free: Balance,
	pub unlocking: Vec<(EraIndex, Balance)>,
}

thread_local! {
	pub static BRIDGE_STATUS: RefCell<HashMap<u32, Status>> = RefCell::new(HashMap::new());
}

pub struct MockBridge;

impl PolkadotBridgeType<BlockNumber, EraIndex> for MockBridge {
	type BondingDuration = BondingDuration;
	type EraLength = EraLength;
	type PolkadotAccountId = PolkadotAccountId;
}

impl PolkadotBridgeCall<AccountId, BlockNumber, Balance, EraIndex> for MockBridge {
	fn bond_extra(account_index: u32, amount: Balance) -> DispatchResult {
		BRIDGE_STATUS.with(|v| {
			let mut old_map = v.borrow().clone();
			if let Some(status) = old_map.get_mut(&account_index) {
				status.free = status.free.saturating_sub(amount);
				status.bonded = status.bonded.saturating_add(amount);
			} else {
				old_map.insert(account_index, Default::default());
			};

			*v.borrow_mut() = old_map;
		});

		Ok(())
	}

	fn unbond(account_index: u32, amount: Balance) -> DispatchResult {
		BRIDGE_STATUS.with(|v| {
			let mut old_map = v.borrow().clone();
			if let Some(status) = old_map.get_mut(&account_index) {
				status.bonded = status.bonded.saturating_sub(amount);
				status
					.unlocking
					.push((StakingPoolModule::current_era() + BondingDuration::get(), amount));
			} else {
				old_map.insert(account_index, Default::default());
			}

			*v.borrow_mut() = old_map;
		});

		Ok(())
	}

	fn rebond(_: u32, _: Balance) -> DispatchResult {
		unimplemented!()
	}

	fn withdraw_unbonded(account_index: u32) {
		BRIDGE_STATUS.with(|v| {
			let mut old_map = v.borrow().clone();
			if let Some(status) = old_map.get_mut(&account_index) {
				let current_era = StakingPoolModule::current_era();
				let mut free = status.free;
				let unlocking = status
					.unlocking
					.clone()
					.into_iter()
					.filter(|(era_index, value)| {
						if *era_index > current_era {
							true
						} else {
							free = free.saturating_add(*value);
							false
						}
					})
					.collect::<Vec<_>>();

				status.free = free;
				status.unlocking = unlocking;
			} else {
				old_map.insert(account_index, Default::default());
			};

			*v.borrow_mut() = old_map;
		});
	}

	fn nominate(_account_index: u32, _targets: Vec<Self::PolkadotAccountId>) {}

	fn payout_nominator(account_index: u32) {
		BRIDGE_STATUS.with(|v| {
			let mut old_map = v.borrow().clone();
			if let Some(status) = old_map.get_mut(&account_index) {
				status.bonded = status
					.bonded
					.saturating_add(Rate::saturating_from_rational(1, 100).saturating_mul_int(status.bonded));
			} else {
				old_map.insert(account_index, Default::default());
			}

			*v.borrow_mut() = old_map;
		});
	}

	fn transfer_to_bridge(account_index: u32, from: &AccountId, amount: Balance) -> DispatchResult {
		BRIDGE_STATUS.with(|v| {
			let mut old_map = v.borrow().clone();
			if let Some(status) = old_map.get_mut(&account_index) {
				status.free = status.free.saturating_add(amount);
			} else {
				old_map.insert(
					account_index,
					Status {
						free: amount,
						..Default::default()
					},
				);
			};

			*v.borrow_mut() = old_map;
		});

		CurrenciesModule::withdraw(DOT, from, amount)
	}

	fn receive_from_bridge(account_index: u32, to: &AccountId, amount: Balance) -> DispatchResult {
		BRIDGE_STATUS.with(|v| {
			let mut old_map = v.borrow().clone();
			if let Some(status) = old_map.get_mut(&account_index) {
				status.free = status.free.saturating_sub(amount);
			} else {
				old_map.insert(account_index, Default::default());
			}

			*v.borrow_mut() = old_map;
		});

		CurrenciesModule::deposit(DOT, to, amount)
	}
}

impl PolkadotBridgeState<Balance, EraIndex> for MockBridge {
	fn staking_ledger(account_index: u32) -> PolkadotStakingLedger<Balance, EraIndex> {
		let map = BRIDGE_STATUS.with(|v| v.borrow().clone());
		let status = map.get(&account_index).unwrap_or(&Default::default()).to_owned();

		let active = status.bonded;
		let mut total = active;
		let unlocking = status
			.unlocking
			.iter()
			.map(|(era, value)| {
				total = total.saturating_add(*value);
				PolkadotUnlockChunk {
					era: *era,
					value: *value,
				}
			})
			.collect::<Vec<_>>();

		PolkadotStakingLedger {
			total,
			active,
			unlocking,
		}
	}

	fn balance(account_index: u32) -> Balance {
		let map = BRIDGE_STATUS.with(|v| v.borrow().clone());
		let status = map.get(&account_index).unwrap_or(&Default::default()).to_owned();
		status.free + Self::staking_ledger(account_index).total
	}

	fn current_era() -> EraIndex {
		StakingPoolModule::current_era()
	}
}

impl PolkadotBridge<AccountId, BlockNumber, Balance, EraIndex> for MockBridge {}

pub struct MockFeeModel;
impl<Balance: FixedPointOperand> FeeModel<Balance> for MockFeeModel {
	/// Linear model:
	/// fee_rate = base_rate + (100% - base_rate) * (1 -
	/// remain_available_percent) * demand_in_available_percent
	fn get_fee(
		remain_available_percent: Ratio,
		available_amount: Balance,
		request_amount: Balance,
		base_rate: Rate,
	) -> Option<Balance> {
		let demand_in_available_percent = Ratio::checked_from_rational(request_amount, available_amount)?;
		let fee_rate = Rate::one()
			.checked_sub(&base_rate)
			.and_then(|n| n.checked_mul(&Rate::one().saturating_sub(remain_available_percent)))
			.and_then(|n| n.checked_mul(&demand_in_available_percent))
			.and_then(|n| n.checked_add(&base_rate))?;

		fee_rate.checked_mul_int(request_amount)
	}
}

parameter_types! {
	pub const GetStakingCurrencyId: CurrencyId = DOT;
	pub const GetLiquidCurrencyId: CurrencyId = LDOT;
	pub DefaultExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(10, 100);	// 1 : 10
	pub const StakingPoolModuleId: ModuleId = ModuleId(*b"aca/stkp");
	pub PoolAccountIndexes: Vec<u32> = vec![1, 2, 3, 4];
}

ord_parameter_types! {
	pub const One: AccountId = 1;
}

impl Config for Runtime {
	type Event = TestEvent;
	type StakingCurrencyId = GetStakingCurrencyId;
	type LiquidCurrencyId = GetLiquidCurrencyId;
	type DefaultExchangeRate = DefaultExchangeRate;
	type ModuleId = StakingPoolModuleId;
	type PoolAccountIndexes = PoolAccountIndexes;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type FeeModel = MockFeeModel;
	type Nominees = MockNomineesProvider;
	type Bridge = MockBridge;
	type Currency = CurrenciesModule;
}
pub type StakingPoolModule = Module<Runtime>;

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![(ALICE, DOT, 1000), (BOB, DOT, 1000)],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		GenesisConfig {
			staking_pool_params: Params {
				target_max_free_unbonded_ratio: Ratio::saturating_from_rational(10, 100),
				target_min_free_unbonded_ratio: Ratio::saturating_from_rational(5, 100),
				target_unbonding_to_free_ratio: Ratio::saturating_from_rational(3, 100),
				unbonding_to_free_adjustment: Rate::saturating_from_rational(1, 100),
				base_fee_rate: Rate::saturating_from_rational(20, 100),
			},
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
