//! Mocks for the dex module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_event, impl_outer_origin, ord_parameter_types, parameter_types};
use frame_system::EnsureSignedBy;
use orml_traits::{parameter_type_with_key, MultiReservableCurrency};
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

ord_parameter_types! {
	pub const ListingOrigin: AccountId = 3;
}

parameter_types! {
	pub const GetExchangeFee: (u32, u32) = (1, 100);
	pub const TradingPathLimit: usize = 3;
	pub const DEXModuleId: ModuleId = ModuleId(*b"aca/dexm");
}

impl Config for Runtime {
	type Event = TestEvent;
	type Currency = Tokens;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = TradingPathLimit;
	type ModuleId = DEXModuleId;
	type WeightInfo = ();
	type DEXIncentives = MockDEXIncentives;
	type ListingOrigin = EnsureSignedBy<ListingOrigin, AccountId>;
}
pub type DexModule = Module<Runtime>;

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
	initial_listing_trading_pairs: Vec<(TradingPair, (Balance, Balance), (Balance, Balance), BlockNumber)>,
	initial_enabled_trading_pairs: Vec<TradingPair>,
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
			initial_listing_trading_pairs: vec![],
			initial_enabled_trading_pairs: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn initialize_listing_trading_pairs(mut self) -> Self {
		self.initial_listing_trading_pairs = vec![
			(
				AUSD_DOT_PAIR,
				(5_000_000_000_000u128, 1_000_000_000_000u128),
				(5_000_000_000_000_000u128, 1_000_000_000_000_000u128),
				10,
			),
			(
				AUSD_XBTC_PAIR,
				(20_000_000_000_000u128, 1_000_000_000u128),
				(20_000_000_000_000_000u128, 1_000_000_000_000u128),
				10,
			),
			(
				DOT_XBTC_PAIR,
				(4_000_000_000_000u128, 1_000_000_000u128),
				(4_000_000_000_000_000u128, 1_000_000_000_000u128),
				20,
			),
		];
		self
	}

	pub fn initialize_enabled_trading_pairs(mut self) -> Self {
		self.initial_enabled_trading_pairs = vec![AUSD_DOT_PAIR, AUSD_XBTC_PAIR, DOT_XBTC_PAIR];
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

		GenesisConfig::<Runtime> {
			initial_listing_trading_pairs: self.initial_listing_trading_pairs,
			initial_enabled_trading_pairs: self.initial_enabled_trading_pairs,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
