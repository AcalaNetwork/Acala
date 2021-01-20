//! Mocks for the NFT benchmarking.

#![cfg(test)]

use super::*;

use codec::{Decode, Encode};
use frame_support::{
	impl_outer_dispatch, impl_outer_origin, parameter_types,
	traits::{Filter, InstanceFilter},
	weights::Weight,
	RuntimeDebug,
};
use orml_traits::parameter_type_with_key;
use primitives::{evm::EvmAddress, mocks::MockAddressMapping, Amount, BlockNumber, CurrencyId};
use sp_core::{crypto::AccountId32, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	DispatchError, DispatchResult, ModuleId, Perbill,
};
use support::{EVMBridge, InvokeContext};

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

impl_outer_dispatch! {
	pub enum Call for Runtime where origin: Origin {
		frame_system::System,
		pallet_balances::Balances,
		pallet_proxy::Proxy,
		pallet_utility::Utility,
	}
}

// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Runtime;
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: Weight = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

pub type AccountId = AccountId32;

impl frame_system::Config for Runtime {
	type BaseCallFilter = BaseFilter;
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Call = Call;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Version = ();
	type PalletInfo = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}
parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}
impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type Event = ();
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Module<Runtime>;
	type MaxLocks = ();
	type WeightInfo = ();
}
impl pallet_utility::Config for Runtime {
	type Event = ();
	type Call = Call;
	type WeightInfo = ();
}
parameter_types! {
	pub const ProxyDepositBase: u64 = 1;
	pub const ProxyDepositFactor: u64 = 1;
	pub const MaxProxies: u16 = 4;
	pub const MaxPending: u32 = 2;
	pub const AnnouncementDepositBase: u64 = 1;
	pub const AnnouncementDepositFactor: u64 = 1;
}
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug)]
pub enum ProxyType {
	Any,
	JustTransfer,
	JustUtility,
}
impl Default for ProxyType {
	fn default() -> Self {
		Self::Any
	}
}
impl InstanceFilter<Call> for ProxyType {
	fn filter(&self, c: &Call) -> bool {
		match self {
			ProxyType::Any => true,
			ProxyType::JustTransfer => matches!(c, Call::Balances(pallet_balances::Call::transfer(..))),
			ProxyType::JustUtility => matches!(c, Call::Utility(..)),
		}
	}
	fn is_superset(&self, o: &Self) -> bool {
		self == &ProxyType::Any || self == o
	}
}
pub struct BaseFilter;
impl Filter<Call> for BaseFilter {
	fn filter(c: &Call) -> bool {
		match *c {
			// Remark is used as a no-op call in the benchmarking
			Call::System(SystemCall::remark(_)) => true,
			Call::System(_) => false,
			_ => true,
		}
	}
}
impl pallet_proxy::Config for Runtime {
	type Event = ();
	type Call = Call;
	type Currency = Balances;
	type ProxyType = ProxyType;
	type ProxyDepositBase = ProxyDepositBase;
	type ProxyDepositFactor = ProxyDepositFactor;
	type MaxProxies = MaxProxies;
	type WeightInfo = ();
	type CallHasher = BlakeTwo256;
	type MaxPending = MaxPending;
	type AnnouncementDepositBase = AnnouncementDepositBase;
	type AnnouncementDepositFactor = AnnouncementDepositFactor;
}

type System = frame_system::Module<Runtime>;
type Balances = pallet_balances::Module<Runtime>;
type Utility = pallet_utility::Module<Runtime>;
type Proxy = pallet_proxy::Module<Runtime>;

pub type NativeCurrency = module_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

impl orml_tokens::Config for Runtime {
	type Event = ();
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
}
pub type Tokens = orml_tokens::Module<Runtime>;

pub struct MockEVMBridge;
impl<AccountId, Balance> EVMBridge<AccountId, Balance> for MockEVMBridge
where
	AccountId: Default,
	Balance: Default,
{
	fn total_supply(_context: InvokeContext) -> Result<Balance, DispatchError> {
		Ok(Default::default())
	}

	fn balance_of(_context: InvokeContext, _address: EvmAddress) -> Result<Balance, DispatchError> {
		Ok(Default::default())
	}

	fn transfer(_context: InvokeContext, _to: EvmAddress, _value: Balance) -> DispatchResult {
		Ok(())
	}

	fn get_origin() -> Option<AccountId> {
		None
	}

	fn set_origin(_origin: AccountId) {}
}

impl module_currencies::Config for Runtime {
	type Event = ();
	type MultiCurrency = Tokens;
	type NativeCurrency = NativeCurrency;
	type WeightInfo = ();
	type AddressMapping = MockAddressMapping;
	type EVMBridge = MockEVMBridge;
}

parameter_types! {
	pub const CreateClassDeposit: Balance = 200;
	pub const CreateTokenDeposit: Balance = 100;
	pub const NftModuleId: ModuleId = ModuleId(*b"aca/aNFT");
}
impl module_nft::Config for Runtime {
	type Event = ();
	type CreateClassDeposit = CreateClassDeposit;
	type CreateTokenDeposit = CreateTokenDeposit;
	type ModuleId = NftModuleId;
	type Currency = NativeCurrency;
	type WeightInfo = ();
}

impl orml_nft::Config for Runtime {
	type ClassId = u32;
	type TokenId = u64;
	type ClassData = ClassData;
	type TokenData = TokenData;
}

use frame_system::Call as SystemCall;

impl crate::Config for Runtime {}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
