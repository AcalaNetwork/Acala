//! Mocks for the incentives module.

#![cfg(test)]

use super::*;
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	impl_outer_origin, ord_parameter_types, parameter_types,
};
use frame_system::EnsureSignedBy;
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};
use sp_std::cell::RefCell;
pub use support::{CDPTreasury, DEXManager, Price, Ratio};

pub type AccountId = u128;
pub type BlockNumber = u64;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const ACA: CurrencyId = CurrencyId::ACA;
pub const AUSD: CurrencyId = CurrencyId::AUSD;
pub const BTC: CurrencyId = CurrencyId::XBTC;
pub const DOT: CurrencyId = CurrencyId::DOT;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

mod incentives {
	pub use super::super::*;
}

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u32 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

impl frame_system::Trait for Runtime {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Call = ();
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
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
	type SystemWeightInfo = ();
}

impl orml_tokens::Trait for Runtime {
	type Event = ();
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type OnReceived = ();
	type WeightInfo = ();
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
	fn get_target_amount(_: CurrencyId, _: CurrencyId, _: Balance) -> Balance {
		unimplemented!()
	}

	fn get_supply_amount(_: CurrencyId, _: CurrencyId, _: Balance) -> Balance {
		unimplemented!()
	}

	fn exchange_currency(
		_: AccountId,
		_: CurrencyId,
		_: Balance,
		_: CurrencyId,
		_: Balance,
	) -> sp_std::result::Result<Balance, DispatchError> {
		unimplemented!()
	}

	fn get_exchange_slippage(_: CurrencyId, _: CurrencyId, _: Balance) -> Option<Ratio> {
		unimplemented!()
	}

	fn get_liquidity_pool(currency_id: CurrencyId) -> (Balance, Balance) {
		match currency_id {
			CurrencyId::XBTC => (100, 500),
			CurrencyId::DOT => (100, 400),
			_ => (0, 0),
		}
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

impl orml_rewards::Trait for Runtime {
	type Share = Share;
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
}

ord_parameter_types! {
	pub const Four: AccountId = 4;
}

impl Trait for Runtime {
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
