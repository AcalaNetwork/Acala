//! Mocks for the dex module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_event, impl_outer_origin, parameter_types};
use orml_traits::MultiReservableCurrency;
use primitives::{Amount, TokenSymbol};
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};

pub type BlockNumber = u64;
pub type AccountId = u128;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const XBTC: CurrencyId = CurrencyId::Token(TokenSymbol::XBTC);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const AUSD_XBTC_PAIR: TradingPair = TradingPair(AUSD, XBTC);
pub const AUSD_DOT_PAIR: TradingPair = TradingPair(AUSD, DOT);
pub const DOT_XBTC_PAIR: TradingPair = TradingPair(DOT, XBTC);

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
	}
}

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

parameter_types! {
	pub const BlockHashCount: BlockNumber = 250;
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

impl orml_tokens::Trait for Runtime {
	type Event = TestEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type OnReceived = ();
	type WeightInfo = ();
}
pub type Tokens = orml_tokens::Module<Runtime>;

pub struct MockDEXIncentives;
impl DEXIncentives<AccountId, CurrencyId, Balance> for MockDEXIncentives {
	fn do_deposit_dex_share(who: &AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult {
		Tokens::reserve(lp_currency_id, who, amount)
	}

	fn do_withdraw_dex_share(who: &AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult {
		let _ = Tokens::unreserve(lp_currency_id, who, amount);
		Ok(())
	}
}

parameter_types! {
	pub const GetExchangeFee: (u32, u32) = (1, 100);
	pub const TradingPathLimit: usize = 3;
	pub EnabledTradingPairs : Vec<TradingPair> = vec![AUSD_DOT_PAIR, AUSD_XBTC_PAIR, DOT_XBTC_PAIR];
	pub const DEXModuleId: ModuleId = ModuleId(*b"aca/dexm");
}

impl Trait for Runtime {
	type Event = TestEvent;
	type Currency = Tokens;
	type EnabledTradingPairs = EnabledTradingPairs;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = TradingPathLimit;
	type ModuleId = DEXModuleId;
	type WeightInfo = ();
	type DEXIncentives = MockDEXIncentives;
}
pub type DexModule = Module<Runtime>;

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, AUSD, 1_000_000_000_000_000_000u128),
				(BOB, AUSD, 1_000_000_000_000_000_000u128),
				(ALICE, XBTC, 1_000_000_000_000_000_000u128),
				(BOB, XBTC, 1_000_000_000_000_000_000u128),
				(ALICE, DOT, 1_000_000_000_000_000_000u128),
				(BOB, DOT, 1_000_000_000_000_000_000u128),
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
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
