//! Mocks for staking pool module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_event, impl_outer_origin, parameter_types};
use primitives::Amount;
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};
use support::PolkadotStakingLedger;

pub type AccountId = u128;
pub type BlockNumber = u64;
pub type PolkadotAccountId = u128;

pub const ALICE: AccountId = 0;
pub const BOB: AccountId = 1;
pub const ACA: CurrencyId = CurrencyId::ACA;
pub const DOT: CurrencyId = CurrencyId::DOT;
pub const LDOT: CurrencyId = CurrencyId::LDOT;

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
}

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
	type DbWeight = ();
	type BlockExecutionWeight = ();
	type ExtrinsicBaseWeight = ();
	type MaximumExtrinsicWeight = ();
	type BaseCallFilter = ();
}
pub type System = system::Module<Runtime>;

impl orml_tokens::Trait for Runtime {
	type Event = TestEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type DustRemoval = ();
	type OnReceived = ();
}
pub type TokensModule = orml_tokens::Module<Runtime>;

parameter_types! {
	pub const ExistentialDeposit: Balance = 1;
}

impl pallet_balances::Trait for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = TestEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
}
type PalletBalances = pallet_balances::Module<Runtime>;
pub type NativeCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Balance>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
}

impl orml_currencies::Trait for Runtime {
	type Event = TestEvent;
	type MultiCurrency = TokensModule;
	type NativeCurrency = NativeCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
}
pub type CurrenciesModule = orml_currencies::Module<Runtime>;

pub struct MockNomineesProvider;
impl NomineesProvider<PolkadotAccountId> for MockNomineesProvider {
	fn nominees() -> Vec<PolkadotAccountId> {
		vec![1, 2, 3]
	}
}

pub struct MockOnCommission;
impl OnCommission<Balance, CurrencyId> for MockOnCommission {
	fn on_commission(_currency_id: CurrencyId, _amount: Balance) {}
}

pub struct MockBridge;

parameter_types! {
	pub const BondingDuration: EraIndex = 4;
	pub const EraLength: BlockNumber = 10;
}

impl PolkadotBridgeType<BlockNumber, EraIndex> for MockBridge {
	type BondingDuration = BondingDuration;
	type EraLength = EraLength;
	type PolkadotAccountId = PolkadotAccountId;
}

impl PolkadotBridgeCall<AccountId, BlockNumber, Balance, EraIndex> for MockBridge {
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
		CurrenciesModule::withdraw(DOT, from, amount)
	}

	fn receive_from_bridge(to: &AccountId, amount: Balance) -> DispatchResult {
		CurrenciesModule::deposit(DOT, to, amount)
	}
}

impl PolkadotBridgeState<Balance, EraIndex> for MockBridge {
	fn ledger() -> PolkadotStakingLedger<Balance, EraIndex> {
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

impl PolkadotBridge<AccountId, BlockNumber, Balance, EraIndex> for MockBridge {}

parameter_types! {
	pub const GetStakingCurrencyId: CurrencyId = DOT;
	pub const GetLiquidCurrencyId: CurrencyId = LDOT;
	pub MaxBondRatio: Ratio = Ratio::saturating_from_rational(60, 100);	// 60%
	pub MinBondRatio: Ratio = Ratio::saturating_from_rational(50, 100);	// 50%
	pub MaxClaimFee: Rate = Rate::saturating_from_rational(10, 100);	// 10%
	pub DefaultExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(10, 100);	// 1 : 10
	pub ClaimFeeReturnRatio: Ratio = Ratio::saturating_from_rational(80, 100);	// 80%
	pub const StakingPoolModuleId: ModuleId = ModuleId(*b"aca/stkp");
}

impl Trait for Runtime {
	type Event = TestEvent;
	type Currency = CurrenciesModule;
	type StakingCurrencyId = GetStakingCurrencyId;
	type LiquidCurrencyId = GetLiquidCurrencyId;
	type Nominees = MockNomineesProvider;
	type OnCommission = MockOnCommission;
	type Bridge = MockBridge;
	type MaxBondRatio = MaxBondRatio;
	type MinBondRatio = MinBondRatio;
	type MaxClaimFee = MaxClaimFee;
	type DefaultExchangeRate = DefaultExchangeRate;
	type ClaimFeeReturnRatio = ClaimFeeReturnRatio;
	type ModuleId = StakingPoolModuleId;
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
		let mut t = system::GenesisConfig::default().build_storage::<Runtime>().unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
