//! Mocks for the transaction payment module.

#![cfg(test)]

use super::*;
use crate as transaction_payment;
use frame_support::{construct_runtime, ord_parameter_types, parameter_types, weights::WeightToFeeCoefficients};
use orml_traits::parameter_type_with_key;
use primitives::{evm::EvmAddress, mocks::MockAddressMapping, Amount, TokenSymbol, TradingPair};
use smallvec::smallvec;
use sp_core::{crypto::AccountId32, H256};
use sp_runtime::{
	testing::Header, traits::IdentityLookup, DispatchError, DispatchResult, FixedPointNumber, ModuleId, Perbill,
};
use sp_std::cell::RefCell;
use support::{EVMBridge, InvokeContext, Ratio};

pub type AccountId = AccountId32;
pub type BlockNumber = u64;

pub const ALICE: AccountId = AccountId::new([1u8; 32]);
pub const BOB: AccountId = AccountId::new([2u8; 32]);
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub static ExtrinsicBaseWeight: u64 = 0;
}

pub struct BlockWeights;
impl Get<frame_system::limits::BlockWeights> for BlockWeights {
	fn get() -> frame_system::limits::BlockWeights {
		frame_system::limits::BlockWeights::builder()
			.base_block(0)
			.for_class(DispatchClass::all(), |weights| {
				weights.base_extrinsic = EXTRINSIC_BASE_WEIGHT.with(|v| *v.borrow()).into();
			})
			.for_class(DispatchClass::non_mandatory(), |weights| {
				weights.max_total = 1024.into();
			})
			.build_or_panic()
	}
}

impl frame_system::Config for Runtime {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Call = Call;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type BlockWeights = BlockWeights;
	type BlockLength = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

impl orml_tokens::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
}

parameter_types! {
	pub const NativeTokenExistentialDeposit: Balance = 0;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = NativeTokenExistentialDeposit;
	type AccountStore = System;
	type MaxLocks = ();
	type WeightInfo = ();
}

pub type AdaptedBasicCurrency = module_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Amount, BlockNumber>;

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

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
}

impl module_currencies::Config for Runtime {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type WeightInfo = ();
	type AddressMapping = MockAddressMapping;
	type EVMBridge = MockEVMBridge;
}

thread_local! {
	static IS_SHUTDOWN: RefCell<bool> = RefCell::new(false);
}

ord_parameter_types! {
	pub const Zero: AccountId = AccountId::new([0u8; 32]);
}

parameter_types! {
	pub const DEXModuleId: ModuleId = ModuleId(*b"aca/dexm");
	pub const GetExchangeFee: (u32, u32) = (0, 100);
	pub const TradingPathLimit: u32 = 3;
	pub EnabledTradingPairs : Vec<TradingPair> = vec![TradingPair::new(AUSD, ACA), TradingPair::new(AUSD, DOT)];
}

impl module_dex::Config for Runtime {
	type Event = Event;
	type Currency = Currencies;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = TradingPathLimit;
	type ModuleId = DEXModuleId;
	type DEXIncentives = ();
	type WeightInfo = ();
	type ListingOrigin = frame_system::EnsureSignedBy<Zero, AccountId>;
}

parameter_types! {
	pub AllNonNativeCurrencyIds: Vec<CurrencyId> = vec![AUSD, DOT];
	pub MaxSlippageSwapWithDEX: Ratio = Ratio::one();
	pub const StableCurrencyId: CurrencyId = AUSD;
	pub static TransactionByteFee: u128 = 1;
}

impl Config for Runtime {
	type AllNonNativeCurrencyIds = AllNonNativeCurrencyIds;
	type NativeCurrencyId = GetNativeCurrencyId;
	type StableCurrencyId = StableCurrencyId;
	type Currency = PalletBalances;
	type MultiCurrency = Currencies;
	type OnTransactionPayment = ();
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = WeightToFee;
	type FeeMultiplierUpdate = ();
	type DEX = DEXModule;
	type MaxSlippageSwapWithDEX = MaxSlippageSwapWithDEX;
	type WeightInfo = ();
}

thread_local! {
	static WEIGHT_TO_FEE: RefCell<u128> = RefCell::new(1);
}

pub struct WeightToFee;
impl WeightToFeePolynomial for WeightToFee {
	type Balance = u128;

	fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
		smallvec![frame_support::weights::WeightToFeeCoefficient {
			degree: 1,
			coeff_frac: Perbill::zero(),
			coeff_integer: WEIGHT_TO_FEE.with(|v| *v.borrow()),
			negative: false,
		}]
	}
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system::{Module, Call, Config, Storage, Event<T>},
		TransactionPayment: transaction_payment::{Module, Call, Storage},
		PalletBalances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
		Tokens: orml_tokens::{Module, Storage, Event<T>, Config<T>},
		Currencies: module_currencies::{Module, Call, Event<T>},
		DEXModule: module_dex::{Module, Storage, Call, Event<T>, Config<T>},
	}
);

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
	base_weight: u64,
	byte_fee: u128,
	weight_to_fee: u128,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![(ALICE, AUSD, 10000), (ALICE, DOT, 1000)],
			base_weight: 0,
			byte_fee: 2,
			weight_to_fee: 1,
		}
	}
}

impl ExtBuilder {
	pub fn base_weight(mut self, base_weight: u64) -> Self {
		self.base_weight = base_weight;
		self
	}
	pub fn byte_fee(mut self, byte_fee: u128) -> Self {
		self.byte_fee = byte_fee;
		self
	}
	pub fn weight_fee(mut self, weight_to_fee: u128) -> Self {
		self.weight_to_fee = weight_to_fee;
		self
	}
	fn set_constants(&self) {
		EXTRINSIC_BASE_WEIGHT.with(|v| *v.borrow_mut() = self.base_weight);
		TRANSACTION_BYTE_FEE.with(|v| *v.borrow_mut() = self.byte_fee);
		WEIGHT_TO_FEE.with(|v| *v.borrow_mut() = self.weight_to_fee);
	}
	pub fn build(self) -> sp_io::TestExternalities {
		self.set_constants();
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: vec![(ALICE, 100000)],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		module_dex::GenesisConfig::<Runtime> {
			initial_listing_trading_pairs: vec![],
			initial_enabled_trading_pairs: EnabledTradingPairs::get(),
			initial_added_liquidity_pools: vec![],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
