//! Mocks for the dex module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_event, impl_outer_origin, ord_parameter_types, parameter_types};
use frame_system::EnsureSignedBy;
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};
use sp_std::cell::RefCell;
use support::{AuctionManager, Rate};

pub type AccountId = u128;
pub type BlockNumber = u64;
pub type Share = u128;
pub type Amount = i128;
pub type AuctionId = u32;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CAROL: AccountId = 3;
pub const AUSD: CurrencyId = CurrencyId::AUSD;
pub const BTC: CurrencyId = CurrencyId::XBTC;
pub const DOT: CurrencyId = CurrencyId::DOT;
pub const ACA: CurrencyId = CurrencyId::ACA;
pub const LDOT: CurrencyId = CurrencyId::LDOT;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

mod dex {
	pub use super::super::*;
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		frame_system<T>,
		dex<T>,
		orml_tokens<T>,
		cdp_treasury,
	}
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
	type SystemWeightInfo = ();
}
pub type System = frame_system::Module<Runtime>;

impl orml_tokens::Trait for Runtime {
	type Event = TestEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type OnReceived = ();
	type WeightInfo = ();
}
pub type Tokens = orml_tokens::Module<Runtime>;

ord_parameter_types! {
	pub const One: AccountId = 1;
}

parameter_types! {
	pub const GetStableCurrencyId: CurrencyId = AUSD;
	pub const MaxAuctionsCount: u32 = 10_000;
	pub const CDPTreasuryModuleId: ModuleId = ModuleId(*b"aca/cdpt");
}

impl cdp_treasury::Trait for Runtime {
	type Event = TestEvent;
	type Currency = Tokens;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = MockAuctionManagerHandler;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type DEX = ();
	type MaxAuctionsCount = MaxAuctionsCount;
	type ModuleId = CDPTreasuryModuleId;
}
pub type CDPTreasuryModule = cdp_treasury::Module<Runtime>;

pub struct MockAuctionManagerHandler;
impl AuctionManager<AccountId> for MockAuctionManagerHandler {
	type CurrencyId = CurrencyId;
	type Balance = Balance;
	type AuctionId = AuctionId;
	fn new_collateral_auction(
		_who: &AccountId,
		_currency_id: Self::CurrencyId,
		_amount: Self::Balance,
		_target: Self::Balance,
	) -> DispatchResult {
		unimplemented!()
	}
	fn new_debit_auction(_amount: Self::Balance, _fix: Self::Balance) -> DispatchResult {
		unimplemented!()
	}
	fn new_surplus_auction(_amount: Self::Balance) -> DispatchResult {
		unimplemented!()
	}
	fn cancel_auction(_id: Self::AuctionId) -> DispatchResult {
		unimplemented!()
	}

	fn get_total_collateral_in_auction(_id: Self::CurrencyId) -> Self::Balance {
		unimplemented!()
	}
	fn get_total_surplus_in_auction() -> Self::Balance {
		unimplemented!()
	}
	fn get_total_debit_in_auction() -> Self::Balance {
		unimplemented!()
	}
	fn get_total_target_in_auction() -> Self::Balance {
		unimplemented!()
	}
}

thread_local! {
	static IS_SHUTDOWN: RefCell<bool> = RefCell::new(false);
}

pub struct MockEmergencyShutdown;
impl EmergencyShutdown for MockEmergencyShutdown {
	fn is_shutdown() -> bool {
		IS_SHUTDOWN.with(|v| *v.borrow_mut())
	}
}

parameter_types! {
	pub const GetBaseCurrencyId: CurrencyId = AUSD;
	pub GetExchangeFee: Rate = Rate::saturating_from_rational(1, 100);
	pub EnabledCurrencyIds : Vec<CurrencyId> = vec![BTC, DOT];
	pub const DEXModuleId: ModuleId = ModuleId(*b"aca/dexm");
}

impl Trait for Runtime {
	type Event = TestEvent;
	type Currency = Tokens;
	type Share = Share;
	type EnabledCurrencyIds = EnabledCurrencyIds;
	type GetBaseCurrencyId = GetBaseCurrencyId;
	type GetExchangeFee = GetExchangeFee;
	type CDPTreasury = CDPTreasuryModule;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type ModuleId = DEXModuleId;
	type EmergencyShutdown = MockEmergencyShutdown;
}
pub type DexModule = Module<Runtime>;

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
	liquidity_incentive_rate: Vec<(CurrencyId, Rate)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, AUSD, 1_000_000_000_000_000_000u128),
				(BOB, AUSD, 1_000_000_000_000_000_000u128),
				(ALICE, BTC, 1_000_000_000_000_000_000u128),
				(BOB, BTC, 1_000_000_000_000_000_000u128),
				(ALICE, DOT, 1_000_000_000_000_000_000u128),
				(BOB, DOT, 1_000_000_000_000_000_000u128),
			],
			liquidity_incentive_rate: vec![
				(BTC, Rate::saturating_from_rational(1, 100)),
				(DOT, Rate::saturating_from_rational(1, 100)),
			],
		}
	}
}

impl ExtBuilder {
	pub fn set_balance(mut self, who: AccountId, currency_id: CurrencyId, balance: Balance) -> Self {
		self.endowed_accounts.push((who, currency_id, balance));
		self
	}
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		dex::GenesisConfig {
			liquidity_incentive_rate: self.liquidity_incentive_rate,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
