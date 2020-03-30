//! Mocks for staking pool module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_event, impl_outer_origin, parameter_types};
use primitives::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};
use support::PolkadotStakingLedger;

mod staking_pool {
	pub use super::super::*;
}

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		system<T>,
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
	pub const ExistentialDeposit: u64 = 1;
}

pub type AccountId = u64;
pub type PolkadotAccountId = u64;
pub type BlockNumber = u64;
pub type Balance = u64;
pub type Amount = i64;
pub type CurrencyId = u32;

pub const ALICE: AccountId = 0;
pub const BOB: AccountId = 1;
pub const ACA: CurrencyId = 0;
pub const DOT: CurrencyId = 1;
pub const LDOT: CurrencyId = 2;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

impl system::Trait for Runtime {
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
	type ModuleToIndex = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
}
pub type System = system::Module<Runtime>;

impl orml_tokens::Trait for Runtime {
	type Event = TestEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type ExistentialDeposit = ExistentialDeposit;
	type DustRemoval = ();
}
pub type TokensModule = orml_tokens::Module<Runtime>;

impl pallet_balances::Trait for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = TestEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
}
type PalletBalances = pallet_balances::Module<Runtime>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
	pub const GetStakingCurrencyId: CurrencyId = DOT;
	pub const GetLiquidCurrencyId: CurrencyId = LDOT;
}

pub type NativeCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Balance>;
pub type StakingCurrency = orml_currencies::Currency<Runtime, GetStakingCurrencyId>;
pub type LiquidCurrency = orml_currencies::Currency<Runtime, GetLiquidCurrencyId>;

impl orml_currencies::Trait for Runtime {
	type Event = TestEvent;
	type MultiCurrency = TokensModule;
	type NativeCurrency = NativeCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
}

parameter_types! {
	pub const MaxBondRatio: Ratio = Ratio::from_rational(60, 100);	// 60%
	pub const MinBondRatio: Ratio = Ratio::from_rational(50, 100);	// 50%
	pub const MaxClaimFee: Rate = Rate::from_rational(10, 100);	// 10%
	pub const DefaultExchangeRate: ExchangeRate = ExchangeRate::from_rational(10, 100);	// 1 : 10
}

pub struct MockNomineesProvider;
impl NomineesProvider<PolkadotAccountId> for MockNomineesProvider {
	fn nominees() -> Vec<PolkadotAccountId> {
		vec![1, 2, 3]
	}
}

pub struct MockOnCommission;
impl OnCommission<Balance> for MockOnCommission {
	fn on_commission(_amount: Balance) {}
}

pub struct MockBridge;

parameter_types! {
	pub const BondingDuration: EraIndex = 5;
	pub const EraLength: BlockNumber = 10;
}

impl PolkadotBridgeType<BlockNumber> for MockBridge {
	type BondingDuration = BondingDuration;
	type EraLength = EraLength;
	type PolkadotAccountId = PolkadotAccountId;
}

impl PolkadotBridgeCall<BlockNumber, Balance, AccountId> for MockBridge {
	fn bond_extra(_amount: Balance) -> DispatchResult {
		Ok(())
	}

	fn unbond(_amount: Balance) -> DispatchResult {
		Ok(())
	}

	fn rebond(_amount: Balance) -> DispatchResult {
		Ok(())
	}

	fn withdraw_unbonded() {}

	fn nominate(_targets: Vec<Self::PolkadotAccountId>) {}

	fn payout_nominator() {}

	fn transfer_to_bridge(from: &AccountId, amount: Balance) -> DispatchResult {
		StakingCurrency::withdraw(from, amount)
	}

	fn receive_from_bridge(to: &AccountId, amount: Balance) -> DispatchResult {
		StakingCurrency::deposit(to, amount)
	}
}

impl PolkadotBridgeState<Balance> for MockBridge {
	fn ledger() -> PolkadotStakingLedger<Balance> {
		PolkadotStakingLedger {
			total: StakingPoolModule::total_bonded(),
			active: StakingPoolModule::total_bonded(),
			unlocking: vec![],
		}
	}

	fn balance() -> Balance {
		StakingPoolModule::total_bonded() + StakingPoolModule::unbonding(StakingPoolModule::current_era()).0
	}

	fn current_era() -> EraIndex {
		StakingPoolModule::current_era()
	}
}

impl PolkadotBridge<BlockNumber, Balance, AccountId> for MockBridge {}

impl Trait for Runtime {
	type Event = TestEvent;
	type StakingCurrency = StakingCurrency;
	type LiquidCurrency = LiquidCurrency;
	type Nominees = MockNomineesProvider;
	type OnCommission = MockOnCommission;
	type Bridge = MockBridge;
	type MaxBondRatio = MaxBondRatio;
	type MinBondRatio = MinBondRatio;
	type MaxClaimFee = MaxClaimFee;
	type DefaultExchangeRate = DefaultExchangeRate;
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
	pub fn build(self) -> runtime_io::TestExternalities {
		let mut t = system::GenesisConfig::default().build_storage::<Runtime>().unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
