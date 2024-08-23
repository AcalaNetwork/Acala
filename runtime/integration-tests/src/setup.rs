// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
use frame_support::traits::{OnFinalize, OnIdle, OnInitialize};
pub use frame_support::{
	assert_noop, assert_ok,
	traits::{Currency as PalletCurrency, Get},
};
pub use frame_system::RawOrigin;
pub use parity_scale_codec::{Decode, Encode};
pub use primitives::TradingPair;
use runtime_common::evm_genesis;
pub use runtime_common::{
	cent, dollar, millicent, FixedRateOfAsset, MaxTipsOfPriority, ProxyType, Ratio, TipPerWeightStep, ACA, AUSD, DOT,
	LDOT,
};
pub use xcm::v4::{prelude::*, Weight as XcmWeight};
pub use xcm_executor::XcmExecutor;

pub use module_support::{
	mocks::MockAddressMapping, AddressMapping, CDPTreasury, DEXManager, Price, Rate, RiskManager,
};

pub use orml_traits::{location::RelativeLocations, Change, GetByKey, MultiCurrency};

pub use insta::assert_debug_snapshot;
pub use primitives::{
	currency::*,
	evm::{CHAIN_ID_ACALA_TESTNET, CHAIN_ID_KARURA_TESTNET, CHAIN_ID_MANDALA},
};
use sp_consensus_aura::AURA_ENGINE_ID;
pub use sp_core::H160;
use sp_io::hashing::keccak_256;
pub use sp_runtime::{
	traits::{AccountIdConversion, BadOrigin, BlakeTwo256, Convert, Hash, Header, Zero},
	BuildStorage, Digest, DigestItem, DispatchError, DispatchResult, FixedPointNumber, FixedU128, MultiAddress,
	Perbill, Permill,
};

#[cfg(feature = "with-mandala-runtime")]
pub use mandala_imports::*;
#[cfg(feature = "with-mandala-runtime")]
mod mandala_imports {
	pub use mandala_runtime::xcm_config::*;
	use mandala_runtime::AlternativeFeeSurplus;
	pub use mandala_runtime::{
		create_x2_parachain_location, get_all_module_accounts, AcalaOracle, AcalaSwap, AccountId, AggregatedDex,
		AssetRegistry, AuctionManager, Aura, AuraExt, Authority, AuthoritysOriginId, Authorship, Balance, Balances,
		BlockNumber, CDPEnginePalletId, CDPTreasuryPalletId, CdpEngine, CdpTreasury, CollatorSelection,
		CreateClassDeposit, CreateTokenDeposit, Currencies, CurrencyId, DataDepositPerByte, DealWithFees,
		DefaultDebitExchangeRate, DefaultExchangeRate, Dex, EmergencyShutdown, EvmAccounts, ExistentialDeposits,
		FinancialCouncil, GetNativeCurrencyId, Homa, Honzon, IdleScheduler, Loans, MinRewardDistributeAmount,
		MinimumDebitValue, NativeTokenExistentialDeposit, NftPalletId, OneDay, OriginCaller, ParachainInfo,
		ParachainSystem, Proxy, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin, Scheduler, Session, SessionKeys,
		SessionManager, SevenDays, StableAsset, StableAssetPalletId, System, Timestamp, TokenSymbol, Tokens,
		TransactionPayment, TransactionPaymentPalletId, TreasuryAccount, TreasuryPalletId, UncheckedExtrinsic, Utility,
		Vesting, XcmInterface, EVM, NFT,
	};
	use primitives::TradingPair;
	use runtime_common::{ACA, AUSD, DOT, LDOT};
	use sp_runtime::Percent;

	parameter_types! {
		pub EnabledTradingPairs: Vec<TradingPair> = vec![
			TradingPair::from_currency_ids(USD_CURRENCY, NATIVE_CURRENCY).unwrap(),
			TradingPair::from_currency_ids(USD_CURRENCY, RELAY_CHAIN_CURRENCY).unwrap(),
			TradingPair::from_currency_ids(USD_CURRENCY, LIQUID_CURRENCY).unwrap(),
			TradingPair::from_currency_ids(RELAY_CHAIN_CURRENCY, NATIVE_CURRENCY).unwrap(),
			TradingPair::from_currency_ids(RELAY_CHAIN_CURRENCY, LIQUID_CURRENCY).unwrap(),
		];
	}

	pub const NATIVE_CURRENCY: CurrencyId = ACA;
	pub const LIQUID_CURRENCY: CurrencyId = LDOT;
	pub const RELAY_CHAIN_CURRENCY: CurrencyId = DOT;
	pub const USD_CURRENCY: CurrencyId = AUSD;
	pub const LPTOKEN: CurrencyId = CurrencyId::DexShare(
		primitives::DexShare::Token(TokenSymbol::AUSD),
		primitives::DexShare::Token(TokenSymbol::DOT),
	);
	pub const NATIVE_TOKEN_SYMBOL: TokenSymbol = TokenSymbol::ACA;
	pub const ALTERNATIVE_SURPLUS: Percent = AlternativeFeeSurplus::get();
}

#[cfg(feature = "with-karura-runtime")]
pub use karura_imports::*;
#[cfg(feature = "with-karura-runtime")]
mod karura_imports {
	use frame_support::parameter_types;
	pub use karura_runtime::xcm_config::*;
	use karura_runtime::AlternativeFeeSurplus;
	pub use karura_runtime::{
		constants::parachains, create_x2_parachain_location, get_all_module_accounts, AcalaOracle, AcalaSwap,
		AccountId, AggregatedDex, AssetRegistry, AuctionManager, Aura, AuraExt, Authority, AuthoritysOriginId, Balance,
		Balances, BlockNumber, CDPEnginePalletId, CDPTreasuryPalletId, CdpEngine, CdpTreasury, CreateClassDeposit,
		CreateTokenDeposit, Currencies, CurrencyId, DataDepositPerByte, DefaultDebitExchangeRate, DefaultExchangeRate,
		Dex, EmergencyShutdown, EvmAccounts, ExistentialDeposits, FinancialCouncil, GetNativeCurrencyId, Homa, Honzon,
		IdleScheduler, KaruraFoundationAccounts, Loans, MinimumDebitValue, NativeTokenExistentialDeposit, NftPalletId,
		OneDay, OriginCaller, ParachainAccount, ParachainInfo, ParachainSystem, PolkadotXcm, Proxy, Runtime,
		RuntimeCall, RuntimeEvent, RuntimeOrigin, Scheduler, Session, SessionManager, SevenDays, StableAsset,
		StableAssetPalletId, System, Timestamp, TokenSymbol, Tokens, TransactionPayment, TransactionPaymentPalletId,
		TreasuryPalletId, Utility, Vesting, XTokens, XcmInterface, EVM, NFT,
	};
	use primitives::TradingPair;
	use runtime_common::{KAR, KSM, KUSD, LKSM};
	use sp_runtime::{traits::AccountIdConversion, Percent};

	parameter_types! {
		pub EnabledTradingPairs: Vec<TradingPair> = vec![
			TradingPair::from_currency_ids(USD_CURRENCY, NATIVE_CURRENCY).unwrap(),
			TradingPair::from_currency_ids(USD_CURRENCY, RELAY_CHAIN_CURRENCY).unwrap(),
			TradingPair::from_currency_ids(USD_CURRENCY, LIQUID_CURRENCY).unwrap(),
			TradingPair::from_currency_ids(RELAY_CHAIN_CURRENCY, NATIVE_CURRENCY).unwrap(),
		];
		pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
	}

	pub const NATIVE_CURRENCY: CurrencyId = KAR;
	pub const LIQUID_CURRENCY: CurrencyId = LKSM;
	pub const RELAY_CHAIN_CURRENCY: CurrencyId = KSM;
	pub const USD_CURRENCY: CurrencyId = KUSD;
	pub const LPTOKEN: CurrencyId = CurrencyId::DexShare(
		primitives::DexShare::Token(TokenSymbol::KUSD),
		primitives::DexShare::Token(TokenSymbol::KSM),
	);
	pub const NATIVE_TOKEN_SYMBOL: TokenSymbol = TokenSymbol::KAR;
	pub const ALTERNATIVE_SURPLUS: Percent = AlternativeFeeSurplus::get();
}

#[cfg(feature = "with-acala-runtime")]
pub use acala_imports::*;
#[cfg(feature = "with-acala-runtime")]
mod acala_imports {
	pub use acala_runtime::xcm_config::*;
	use acala_runtime::AlternativeFeeSurplus;
	pub use acala_runtime::{
		constants::parachains, create_x2_parachain_location, get_all_module_accounts, AcalaFoundationAccounts,
		AcalaOracle, AcalaSwap, AccountId, AggregatedDex, AssetRegistry, AuctionManager, Aura, AuraExt, Authority,
		AuthoritysOriginId, Balance, Balances, BlockNumber, CDPEnginePalletId, CDPTreasuryPalletId, CdpEngine,
		CdpTreasury, CreateClassDeposit, CreateTokenDeposit, Currencies, CurrencyId, DataDepositPerByte,
		DefaultDebitExchangeRate, DefaultExchangeRate, Dex, EmergencyShutdown, EvmAccounts, ExistentialDeposits,
		FinancialCouncil, GetNativeCurrencyId, Homa, Honzon, IdleScheduler, Loans, MinimumDebitValue,
		NativeTokenExistentialDeposit, NftPalletId, OneDay, OriginCaller, ParachainAccount, ParachainInfo,
		ParachainSystem, PolkadotXcm, Proxy, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin, Scheduler, Session,
		SessionManager, SevenDays, StableAsset, StableAssetPalletId, System, Timestamp, TokenSymbol, Tokens,
		TransactionPayment, TransactionPaymentPalletId, TreasuryPalletId, Utility, Vesting, XTokens, XcmInterface, EVM,
		NFT,
	};
	use frame_support::parameter_types;
	use primitives::TradingPair;
	use runtime_common::{ACA, AUSD, DOT, LCDOT, LDOT};
	use sp_runtime::traits::AccountIdConversion;
	use sp_runtime::Percent;

	parameter_types! {
		pub EnabledTradingPairs: Vec<TradingPair> = vec![
			TradingPair::from_currency_ids(USD_CURRENCY, NATIVE_CURRENCY).unwrap(),
			TradingPair::from_currency_ids(USD_CURRENCY, RELAY_CHAIN_CURRENCY).unwrap(),
			TradingPair::from_currency_ids(USD_CURRENCY, LIQUID_CURRENCY).unwrap(),
			TradingPair::from_currency_ids(USD_CURRENCY, LCDOT).unwrap(),
			TradingPair::from_currency_ids(RELAY_CHAIN_CURRENCY, NATIVE_CURRENCY).unwrap(),
			TradingPair::from_currency_ids(RELAY_CHAIN_CURRENCY, LCDOT).unwrap(),
		];
		pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
	}

	pub const NATIVE_CURRENCY: CurrencyId = ACA;
	pub const LIQUID_CURRENCY: CurrencyId = LDOT;
	pub const RELAY_CHAIN_CURRENCY: CurrencyId = DOT;
	pub const USD_CURRENCY: CurrencyId = AUSD;
	pub const LPTOKEN: CurrencyId = CurrencyId::DexShare(
		primitives::DexShare::Token(TokenSymbol::AUSD),
		primitives::DexShare::Token(TokenSymbol::DOT),
	);
	pub const NATIVE_TOKEN_SYMBOL: TokenSymbol = TokenSymbol::ACA;
	pub const ALTERNATIVE_SURPLUS: Percent = AlternativeFeeSurplus::get();
}

const ORACLE1: [u8; 32] = [0u8; 32];
const ORACLE2: [u8; 32] = [1u8; 32];
const ORACLE3: [u8; 32] = [2u8; 32];
const ORACLE4: [u8; 32] = [3u8; 32];
const ORACLE5: [u8; 32] = [4u8; 32];

pub const ALICE: [u8; 32] = [4u8; 32];
pub const BOB: [u8; 32] = [5u8; 32];
pub const CHARLIE: [u8; 32] = [6u8; 32];
#[allow(dead_code)]
pub const DAVE: [u8; 32] = [7u8; 32];

pub const INIT_TIMESTAMP: u64 = 30_000;
pub const BLOCK_TIME: u64 = 12000;

pub fn run_to_block(n: u32) {
	while System::block_number() < n {
		Scheduler::on_finalize(System::block_number());

		let block_number = System::block_number() + 1;
		let timestamp = (block_number as u64 * BLOCK_TIME) + INIT_TIMESTAMP;
		let slot = timestamp / Aura::slot_duration();
		let digest = Digest {
			logs: vec![DigestItem::PreRuntime(AURA_ENGINE_ID, slot.encode())],
		};
		System::initialize(&block_number, &Default::default(), &digest);
		Aura::on_initialize(block_number);
		Timestamp::set_timestamp(timestamp);
		CdpEngine::on_initialize(block_number);
		Scheduler::on_initialize(block_number);
		Scheduler::on_initialize(block_number);
		Session::on_initialize(block_number);
		SessionManager::on_initialize(block_number);
		IdleScheduler::on_idle(block_number, Weight::MAX);
	}
}

pub fn set_relaychain_block_number(number: BlockNumber) {
	AuraExt::on_initialize(number);
	ParachainSystem::on_initialize(number);

	let mut sproof_builder = RelayStateSproofBuilder::default();

	let parent_head_data = {
		let header = cumulus_primitives_core::relay_chain::Header::new(
			number,
			sp_core::H256::from_low_u64_be(0),
			sp_core::H256::from_low_u64_be(0),
			Default::default(),
			Default::default(),
		);
		cumulus_primitives_core::relay_chain::HeadData(header.encode())
	};

	sproof_builder.para_id = ParachainInfo::get().into();
	sproof_builder.included_para_head = Some(parent_head_data.clone());

	let (relay_storage_root, proof) = sproof_builder.into_state_root_and_proof();

	assert_ok!(ParachainSystem::set_validation_data(
		RuntimeOrigin::none(),
		cumulus_primitives_parachain_inherent::ParachainInherentData {
			validation_data: cumulus_primitives_core::PersistedValidationData {
				parent_head: Default::default(),
				relay_parent_number: number,
				relay_parent_storage_root: relay_storage_root,
				max_pov_size: Default::default(),
			},
			relay_chain_state: proof,
			downward_messages: Default::default(),
			horizontal_messages: Default::default(),
		}
	));
}

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
	parachain_id: u32,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			balances: vec![],
			parachain_id: 2000,
		}
	}
}

impl ExtBuilder {
	pub fn balances(mut self, balances: Vec<(AccountId, CurrencyId, Balance)>) -> Self {
		self.balances = balances;
		self
	}

	#[allow(dead_code)]
	pub fn parachain_id(mut self, parachain_id: u32) -> Self {
		self.parachain_id = parachain_id;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let t = self.build_storage();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}

	pub fn build_storage(self) -> sp_core::storage::Storage {
		let evm_genesis_accounts = evm_genesis(vec![]);

		let mut t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		let native_currency_id = GetNativeCurrencyId::get();
		let existential_deposit = NativeTokenExistentialDeposit::get();
		let initial_enabled_trading_pairs = EnabledTradingPairs::get();

		module_asset_registry::GenesisConfig::<Runtime> {
			assets: vec![
				(NATIVE_CURRENCY, existential_deposit),
				(LIQUID_CURRENCY, ExistentialDeposits::get(&LIQUID_CURRENCY)),
				(RELAY_CHAIN_CURRENCY, ExistentialDeposits::get(&RELAY_CHAIN_CURRENCY)),
				(USD_CURRENCY, ExistentialDeposits::get(&USD_CURRENCY)),
			],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		module_dex::GenesisConfig::<Runtime> {
			initial_enabled_trading_pairs: initial_enabled_trading_pairs,
			initial_listing_trading_pairs: Default::default(),
			initial_added_liquidity_pools: vec![],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self
				.balances
				.clone()
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id == native_currency_id)
				.map(|(account_id, _, initial_balance)| (account_id, initial_balance))
				.chain(
					get_all_module_accounts()
						.iter()
						.map(|x| (x.clone(), existential_deposit)),
				)
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self
				.balances
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id != native_currency_id)
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_membership::GenesisConfig::<Runtime, pallet_membership::Instance5> {
			members: vec![
				AccountId::from(ORACLE1),
				AccountId::from(ORACLE2),
				AccountId::from(ORACLE3),
				AccountId::from(ORACLE4),
				AccountId::from(ORACLE5),
			]
			.try_into()
			.expect("convert error!"),
			phantom: Default::default(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		module_evm::GenesisConfig::<Runtime> {
			#[cfg(feature = "with-mandala-runtime")]
			chain_id: CHAIN_ID_MANDALA,
			#[cfg(feature = "with-karura-runtime")]
			chain_id: CHAIN_ID_KARURA_TESTNET,
			#[cfg(feature = "with-acala-runtime")]
			chain_id: CHAIN_ID_ACALA_TESTNET,
			accounts: evm_genesis_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		module_session_manager::GenesisConfig::<Runtime> { session_duration: 10 }
			.assimilate_storage(&mut t)
			.unwrap();

		parachain_info::GenesisConfig::<Runtime> {
			_config: Default::default(),
			parachain_id: self.parachain_id.into(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_xcm::GenesisConfig::<Runtime> {
			_config: Default::default(),
			safe_xcm_version: Some(2),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t
	}
}

pub fn set_oracle_price(prices: Vec<(CurrencyId, Price)>) {
	AcalaOracle::on_finalize(0);
	assert_ok!(AcalaOracle::feed_values(
		RuntimeOrigin::signed(AccountId::from(ORACLE1)),
		prices.clone().try_into().unwrap(),
	));
	assert_ok!(AcalaOracle::feed_values(
		RuntimeOrigin::signed(AccountId::from(ORACLE2)),
		prices.clone().try_into().unwrap(),
	));
	assert_ok!(AcalaOracle::feed_values(
		RuntimeOrigin::signed(AccountId::from(ORACLE3)),
		prices.clone().try_into().unwrap(),
	));
	assert_ok!(AcalaOracle::feed_values(
		RuntimeOrigin::signed(AccountId::from(ORACLE4)),
		prices.clone().try_into().unwrap(),
	));
	assert_ok!(AcalaOracle::feed_values(
		RuntimeOrigin::signed(AccountId::from(ORACLE5)),
		prices.try_into().unwrap(),
	));
}

pub fn alice_key() -> libsecp256k1::SecretKey {
	libsecp256k1::SecretKey::parse(&keccak_256(b"Alice")).unwrap()
}

pub fn bob_key() -> libsecp256k1::SecretKey {
	libsecp256k1::SecretKey::parse(&keccak_256(b"Bob")).unwrap()
}

pub fn alice() -> AccountId {
	let address = EvmAccounts::eth_address(&alice_key());
	let mut data = [0u8; 32];
	data[0..4].copy_from_slice(b"evm:");
	data[4..24].copy_from_slice(&address[..]);
	AccountId::from(Into::<[u8; 32]>::into(data))
}

pub fn bob() -> AccountId {
	let address = EvmAccounts::eth_address(&bob_key());
	let mut data = [0u8; 32];
	data[0..4].copy_from_slice(b"evm:");
	data[4..24].copy_from_slice(&address[..]);
	AccountId::from(Into::<[u8; 32]>::into(data))
}
