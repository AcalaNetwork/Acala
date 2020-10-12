//! Mocks for the cdp treasury module.

#![cfg(test)]

use super::*;
use frame_support::{
	impl_outer_dispatch, impl_outer_event, impl_outer_origin, ord_parameter_types, parameter_types,
	weights::IdentityFee,
};
use primitives::{Amount, TokenSymbol};
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup, FixedPointNumber, Perbill};
use sp_std::cell::RefCell;
use support::{CDPTreasury, Rate, Ratio};

pub type AccountId = u128;
pub type BlockNumber = u64;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CAROL: AccountId = 3;
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const BTC: CurrencyId = CurrencyId::Token(TokenSymbol::XBTC);
pub const ACA_AUSD_LP: CurrencyId = CurrencyId::DEXShare(TokenSymbol::ACA, TokenSymbol::AUSD);
pub const BTC_AUSD_LP: CurrencyId = CurrencyId::DEXShare(TokenSymbol::XBTC, TokenSymbol::AUSD);

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

impl_outer_dispatch! {
	pub enum Call for Runtime where origin: Origin {
		orml_currencies::Currencies,
		frame_system::System,
	}
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		frame_system<T>,
		orml_tokens<T>,
		pallet_balances<T>,
		orml_currencies<T>,
		dex<T>,
	}
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
	type Call = Call;
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
	type OnReceived = Accounts;
	type WeightInfo = ();
}
pub type Tokens = orml_tokens::Module<Runtime>;

parameter_types! {
	pub const ExistentialDeposit: Balance = 0;
}

impl pallet_balances::Trait for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = TestEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = Accounts;
	type MaxLocks = ();
	type WeightInfo = ();
}
pub type PalletBalances = pallet_balances::Module<Runtime>;

pub type AdaptedBasicCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Amount, BlockNumber>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
}

impl orml_currencies::Trait for Runtime {
	type Event = TestEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}
pub type Currencies = orml_currencies::Module<Runtime>;

parameter_types! {
	pub const TransactionByteFee: Balance = 2;
}

impl pallet_transaction_payment::Trait for Runtime {
	type Currency = PalletBalances;
	type OnTransactionPayment = ();
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = IdentityFee<Balance>;
	type FeeMultiplierUpdate = ();
}

pub struct MockCDPTreasury;
impl CDPTreasury<AccountId> for MockCDPTreasury {
	type Balance = Balance;
	type CurrencyId = CurrencyId;

	fn get_surplus_pool() -> Self::Balance {
		Default::default()
	}
	fn get_debit_pool() -> Self::Balance {
		Default::default()
	}
	fn get_total_collaterals(_: Self::CurrencyId) -> Self::Balance {
		Default::default()
	}
	fn get_debit_proportion(_: Self::Balance) -> Ratio {
		Default::default()
	}
	fn on_system_debit(_: Self::Balance) -> DispatchResult {
		Ok(())
	}
	fn on_system_surplus(_: Self::Balance) -> DispatchResult {
		Ok(())
	}
	fn issue_debit(_: &AccountId, _: Self::Balance, _: bool) -> DispatchResult {
		Ok(())
	}
	fn burn_debit(_: &AccountId, _: Self::Balance) -> DispatchResult {
		Ok(())
	}
	fn deposit_surplus(_: &AccountId, _: Self::Balance) -> DispatchResult {
		Ok(())
	}
	fn deposit_collateral(_: &AccountId, _: Self::CurrencyId, _: Self::Balance) -> DispatchResult {
		Ok(())
	}
	fn withdraw_collateral(_: &AccountId, _: Self::CurrencyId, _: Self::Balance) -> DispatchResult {
		Ok(())
	}
}

thread_local! {
	static IS_SHUTDOWN: RefCell<bool> = RefCell::new(false);
}

ord_parameter_types! {
	pub const Zero: AccountId = 0;
}

parameter_types! {
	pub GetExchangeFee: Rate = Rate::saturating_from_rational(0, 100);
	pub const GetStableCurrencyId: CurrencyId = AUSD;
	pub EnabledCurrencyIds: Vec<CurrencyId> = vec![ACA, BTC];
	pub const DEXModuleId: ModuleId = ModuleId(*b"aca/dexm");
}

impl dex::Trait for Runtime {
	type Event = TestEvent;
	type Currency = Currencies;
	type EnabledCurrencyIds = EnabledCurrencyIds;
	type GetBaseCurrencyId = GetStableCurrencyId;
	type GetExchangeFee = GetExchangeFee;
	type CDPTreasury = MockCDPTreasury;
	type ModuleId = DEXModuleId;
	type WeightInfo = ();
}
pub type DEXModule = dex::Module<Runtime>;

parameter_types! {
	pub AllNonNativeCurrencyIds: Vec<CurrencyId> = vec![AUSD, BTC];
	pub const NewAccountDeposit: Balance = 100;
	pub const TreasuryModuleId: ModuleId = ModuleId(*b"py/trsry");
	pub MaxSlippageSwapWithDEX: Ratio = Ratio::one();
}

impl Trait for Runtime {
	type AllNonNativeCurrencyIds = AllNonNativeCurrencyIds;
	type NativeCurrencyId = GetNativeCurrencyId;
	type Currency = Currencies;
	type DEX = DEXModule;
	type OnCreatedAccount = frame_system::CallOnCreatedAccount<Runtime>;
	type KillAccount = frame_system::CallKillAccount<Runtime>;
	type NewAccountDeposit = NewAccountDeposit;
	type TreasuryModuleId = TreasuryModuleId;
	type MaxSlippageSwapWithDEX = MaxSlippageSwapWithDEX;
}
pub type Accounts = Module<Runtime>;

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![(ALICE, AUSD, 10000), (ALICE, BTC, 1000)],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: vec![(ALICE, 100000 + NewAccountDeposit::get())],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
