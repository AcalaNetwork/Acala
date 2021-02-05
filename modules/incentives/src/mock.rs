//! Mocks for the incentives module.

#![cfg(test)]

use super::*;
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	impl_outer_event, impl_outer_origin, ord_parameter_types, parameter_types,
};
use frame_system::EnsureSignedBy;
use orml_traits::parameter_type_with_key;
use primitives::TokenSymbol;
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup};
use sp_std::cell::RefCell;
pub use support::{CDPTreasury, DEXManager, Price, Ratio};

pub type AccountId = u128;
pub type BlockNumber = u64;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const BTC: CurrencyId = CurrencyId::Token(TokenSymbol::XBTC);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const BTC_AUSD_LP: CurrencyId = CurrencyId::DEXShare(TokenSymbol::XBTC, TokenSymbol::AUSD);
pub const DOT_AUSD_LP: CurrencyId = CurrencyId::DEXShare(TokenSymbol::DOT, TokenSymbol::AUSD);

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

mod incentives {
	pub use super::super::*;
}

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		frame_system<T>,
		incentives<T>,
		orml_tokens<T>,
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
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
	type BlockWeights = ();
	type BlockLength = ();
	type Version = ();
	type PalletInfo = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
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

pub struct MockCDPTreasury;
impl CDPTreasury<AccountId> for MockCDPTreasury {
	type Balance = Balance;
	type CurrencyId = CurrencyId;

	fn get_surplus_pool() -> Balance {
		unimplemented!()
	}

	fn get_debit_pool() -> Balance {
		unimplemented!()
	}

	fn get_total_collaterals(_: CurrencyId) -> Balance {
		unimplemented!()
	}

	fn get_debit_proportion(_: Balance) -> Ratio {
		unimplemented!()
	}

	fn on_system_debit(_: Balance) -> DispatchResult {
		unimplemented!()
	}

	fn on_system_surplus(_: Balance) -> DispatchResult {
		unimplemented!()
	}

	fn issue_debit(who: &AccountId, debit: Balance, _: bool) -> DispatchResult {
		TokensModule::deposit(ACA, who, debit)
	}

	fn burn_debit(_: &AccountId, _: Balance) -> DispatchResult {
		unimplemented!()
	}

	fn deposit_surplus(_: &AccountId, _: Balance) -> DispatchResult {
		unimplemented!()
	}

	fn deposit_collateral(_: &AccountId, _: CurrencyId, _: Balance) -> DispatchResult {
		unimplemented!()
	}

	fn withdraw_collateral(_: &AccountId, _: CurrencyId, _: Balance) -> DispatchResult {
		unimplemented!()
	}
}

pub struct MockDEX;
impl DEXManager<AccountId, CurrencyId, Balance> for MockDEX {
	fn get_liquidity_pool(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> (Balance, Balance) {
		match (currency_id_a, currency_id_b) {
			(AUSD, BTC) => (500, 100),
			(AUSD, DOT) => (400, 100),
			(BTC, AUSD) => (100, 500),
			(DOT, AUSD) => (100, 400),
			_ => (0, 0),
		}
	}

	fn get_swap_target_amount(_: &[CurrencyId], _: Balance, _: Option<Ratio>) -> Option<Balance> {
		unimplemented!()
	}

	fn get_swap_supply_amount(_: &[CurrencyId], _: Balance, _: Option<Ratio>) -> Option<Balance> {
		unimplemented!()
	}

	fn swap_with_exact_supply(
		_: &AccountId,
		_: &[CurrencyId],
		_: Balance,
		_: Balance,
		_: Option<Ratio>,
	) -> sp_std::result::Result<Balance, DispatchError> {
		unimplemented!()
	}

	fn swap_with_exact_target(
		_: &AccountId,
		_: &[CurrencyId],
		_: Balance,
		_: Balance,
		_: Option<Ratio>,
	) -> sp_std::result::Result<Balance, DispatchError> {
		unimplemented!()
	}
}

thread_local! {
	static IS_SHUTDOWN: RefCell<bool> = RefCell::new(false);
}

pub fn mock_shutdown() {
	IS_SHUTDOWN.with(|v| *v.borrow_mut() = true)
}

pub struct MockEmergencyShutdown;
impl EmergencyShutdown for MockEmergencyShutdown {
	fn is_shutdown() -> bool {
		IS_SHUTDOWN.with(|v| *v.borrow_mut())
	}
}

impl orml_rewards::Config for Runtime {
	type Share = Balance;
	type Balance = Balance;
	type PoolId = PoolId;
	type Handler = IncentivesModule;
	type WeightInfo = ();
}
pub type RewardsModule = orml_rewards::Module<Runtime>;

parameter_types! {
	pub const LoansIncentivePool: AccountId = 10;
	pub const DexIncentivePool: AccountId = 11;
	pub const HomaIncentivePool: AccountId = 12;
	pub const AccumulatePeriod: BlockNumber = 10;
	pub const IncentiveCurrencyId: CurrencyId = ACA;
	pub const SavingCurrencyId: CurrencyId = AUSD;
	pub const IncentivesModuleId: ModuleId = ModuleId(*b"aca/inct");
}

ord_parameter_types! {
	pub const Four: AccountId = 4;
}

impl Config for Runtime {
	type Event = TestEvent;
	type LoansIncentivePool = LoansIncentivePool;
	type DexIncentivePool = DexIncentivePool;
	type HomaIncentivePool = HomaIncentivePool;
	type AccumulatePeriod = AccumulatePeriod;
	type IncentiveCurrencyId = IncentiveCurrencyId;
	type SavingCurrencyId = SavingCurrencyId;
	type UpdateOrigin = EnsureSignedBy<Four, AccountId>;
	type CDPTreasury = MockCDPTreasury;
	type Currency = TokensModule;
	type DEX = MockDEX;
	type EmergencyShutdown = MockEmergencyShutdown;
	type ModuleId = IncentivesModuleId;
	type WeightInfo = ();
}

pub type IncentivesModule = Module<Runtime>;

#[derive(Default)]
pub struct ExtBuilder;

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();
		t.into()
	}
}
