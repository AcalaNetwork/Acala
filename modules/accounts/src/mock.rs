//! Mocks for the cdp treasury module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_dispatch, impl_outer_origin, parameter_types};
use primitives::H256;
use sp_runtime::{
	testing::Header,
	traits::{ConvertInto, IdentityLookup},
	Perbill,
};

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

impl_outer_dispatch! {
	pub enum Call for Runtime where origin: Origin {
		orml_currencies::Currencies,
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u32 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
	pub const ExistentialDeposit: u64 = 0;
	pub const TransferFee: u64 = 0;
	pub const CreationFee: u64 = 2;
	pub const GetNativeCurrencyId: CurrencyId = ACA;
}

pub type AccountId = u64;
pub type BlockNumber = u64;
pub type Balance = u64;
pub type Amount = i64;
pub type CurrencyId = u32;
pub type Moment = u64;

pub const ALICE: AccountId = 0;
pub const BOB: AccountId = 1;
pub const ACA: CurrencyId = 0;
pub const AUSD: CurrencyId = 1;
pub const BTC: CurrencyId = 2;

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
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();
	type ModuleToIndex = ();
}

impl orml_tokens::Trait for Runtime {
	type Event = ();
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type ExistentialDeposit = ExistentialDeposit;
	type DustRemoval = ();
}
pub type Tokens = orml_tokens::Module<Runtime>;

impl pallet_balances::Trait for Runtime {
	type Balance = Balance;
	type OnFreeBalanceZero = ();
	type OnNewAccount = ();
	type OnReapAccount = ();
	type TransferPayment = ();
	type DustRemoval = ();
	type Event = ();
	type ExistentialDeposit = ExistentialDeposit;
	type TransferFee = TransferFee;
	type CreationFee = CreationFee;
}
pub type PalletBalances = pallet_balances::Module<Runtime>;

pub type AdaptedBasicCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Balance>;

impl orml_currencies::Trait for Runtime {
	type Event = ();
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
}
pub type Currencies = orml_currencies::Module<Runtime>;

parameter_types! {
	pub const MinimumPeriod: Moment = 5;
}

impl pallet_timestamp::Trait for Runtime {
	type Moment = Moment;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
}
pub type TimeModule = pallet_timestamp::Module<Runtime>;

parameter_types! {
	pub const TransactionBaseFee: Balance = 0;
	pub const TransactionByteFee: Balance = 2;
}

impl pallet_transaction_payment::Trait for Runtime {
	type Currency = PalletBalances;
	type OnTransactionPayment = ();
	type TransactionBaseFee = TransactionBaseFee;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = ConvertInto;
	type FeeMultiplierUpdate = ();
}
pub type PalletTransactionPayment = pallet_transaction_payment::Module<Runtime>;

parameter_types! {
	pub const FreeTransferCount: u8 = 3;
	pub const FreeTransferPeriod: Moment = 100;
}

impl Trait for Runtime {
	type FreeTransferCount = FreeTransferCount;
	type FreeTransferPeriod = FreeTransferPeriod;
	type Time = TimeModule;
	type Currency = Currencies;
	type Call = Call;
}
pub type Accounts = Module<Runtime>;

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![(ALICE, ACA, 1000), (ALICE, AUSD, 1000), (ALICE, BTC, 1000)],
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
