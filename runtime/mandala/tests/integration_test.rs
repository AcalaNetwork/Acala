#![cfg(test)]

use codec::Encode;
use frame_support::{
	assert_noop, assert_ok,
	traits::{schedule::DispatchTime, Currency, GenesisBuild, OnFinalize, OnInitialize, OriginTrait},
};
use frame_system::RawOrigin;
use mandala_runtime::{
	get_all_module_accounts, AccountId, AuthoritysOriginId, Balance, Balances, BlockNumber, Call, CreateClassDeposit,
	CreateTokenDeposit, CurrencyId, DSWFModuleId, EnabledTradingPairs, Event, EvmAccounts, GetNativeCurrencyId,
	NativeTokenExistentialDeposit, NftModuleId, Origin, OriginCaller, Perbill, Proxy, Runtime, SevenDays, System,
	TokenSymbol, EVM, NFT,
};
use module_cdp_engine::LiquidationStrategy;
use module_support::{CDPTreasury, DEXManager, Price, Rate, Ratio, RiskManager};
use orml_authority::DelayedOrigin;
use orml_traits::{Change, MultiCurrency};
use sp_io::hashing::keccak_256;
use sp_runtime::{
	traits::{AccountIdConversion, BadOrigin},
	DispatchError, DispatchResult, FixedPointNumber, MultiAddress,
};

const ORACLE1: [u8; 32] = [0u8; 32];
const ORACLE2: [u8; 32] = [1u8; 32];
const ORACLE3: [u8; 32] = [2u8; 32];

const ALICE: [u8; 32] = [4u8; 32];
const BOB: [u8; 32] = [5u8; 32];

pub type OracleModule = orml_oracle::Module<Runtime, orml_oracle::Instance1>;
pub type DexModule = module_dex::Module<Runtime>;
pub type CdpEngineModule = module_cdp_engine::Module<Runtime>;
pub type LoansModule = module_loans::Module<Runtime>;
pub type CdpTreasuryModule = module_cdp_treasury::Module<Runtime>;
pub type SystemModule = frame_system::Module<Runtime>;
pub type EmergencyShutdownModule = module_emergency_shutdown::Module<Runtime>;
pub type AuctionManagerModule = module_auction_manager::Module<Runtime>;
pub type AuthorityModule = orml_authority::Module<Runtime>;
pub type Currencies = module_currencies::Module<Runtime>;
pub type SchedulerModule = pallet_scheduler::Module<Runtime>;

fn run_to_block(n: u32) {
	while SystemModule::block_number() < n {
		SchedulerModule::on_finalize(SystemModule::block_number());
		SystemModule::set_block_number(SystemModule::block_number() + 1);
		SchedulerModule::on_initialize(SystemModule::block_number());
	}
}

fn last_event() -> Event {
	SystemModule::events().pop().expect("Event expected").event
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn balances(mut self, endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>) -> Self {
		self.endowed_accounts = endowed_accounts;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		let native_currency_id = GetNativeCurrencyId::get();
		let existential_deposit = NativeTokenExistentialDeposit::get();
		let initial_enabled_trading_pairs = EnabledTradingPairs::get();

		module_dex::GenesisConfig::<Runtime> {
			initial_enabled_trading_pairs: initial_enabled_trading_pairs,
			initial_listing_trading_pairs: Default::default(),
			initial_added_liquidity_pools: vec![],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self
				.endowed_accounts
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
			endowed_accounts: self
				.endowed_accounts
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
			],
			phantom: Default::default(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| SystemModule::set_block_number(1));
		ext
	}
}

pub fn origin_of(account_id: AccountId) -> <Runtime as frame_system::Config>::Origin {
	<Runtime as frame_system::Config>::Origin::signed(account_id)
}

fn set_oracle_price(prices: Vec<(CurrencyId, Price)>) -> DispatchResult {
	OracleModule::on_finalize(0);
	assert_ok!(OracleModule::feed_values(
		origin_of(AccountId::from(ORACLE1)),
		prices.clone(),
	));
	assert_ok!(OracleModule::feed_values(
		origin_of(AccountId::from(ORACLE2)),
		prices.clone(),
	));
	assert_ok!(OracleModule::feed_values(origin_of(AccountId::from(ORACLE3)), prices,));
	Ok(())
}

fn amount(amount: u128) -> u128 {
	amount.saturating_mul(Price::accuracy())
}

fn alice() -> secp256k1::SecretKey {
	secp256k1::SecretKey::parse(&keccak_256(b"Alice")).unwrap()
}

fn bob() -> secp256k1::SecretKey {
	secp256k1::SecretKey::parse(&keccak_256(b"Bob")).unwrap()
}

pub fn alice_account_id() -> AccountId {
	let address = EvmAccounts::eth_address(&alice());
	let mut data = [0u8; 32];
	data[0..4].copy_from_slice(b"evm:");
	data[4..24].copy_from_slice(&address[..]);
	AccountId::from(Into::<[u8; 32]>::into(data))
}

pub fn bob_account_id() -> AccountId {
	let address = EvmAccounts::eth_address(&bob());
	let mut data = [0u8; 32];
	data[0..4].copy_from_slice(b"evm:");
	data[4..24].copy_from_slice(&address[..]);
	AccountId::from(Into::<[u8; 32]>::into(data))
}

#[cfg(not(feature = "with-ethereum-compatibility"))]
use sp_core::H160;
#[cfg(not(feature = "with-ethereum-compatibility"))]
fn deploy_contract(account: AccountId) -> Result<H160, DispatchError> {
	// pragma solidity ^0.5.0;
	//
	// contract Factory {
	//     Contract[] newContracts;
	//
	//     function createContract () public payable {
	//         Contract newContract = new Contract();
	//         newContracts.push(newContract);
	//     }
	// }
	//
	// contract Contract {}
	let contract = hex_literal::hex!("608060405234801561001057600080fd5b5061016f806100206000396000f3fe608060405260043610610041576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063412a5a6d14610046575b600080fd5b61004e610050565b005b600061005a6100e2565b604051809103906000f080158015610076573d6000803e3d6000fd5b50905060008190806001815401808255809150509060018203906000526020600020016000909192909190916101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505050565b6040516052806100f28339019056fe6080604052348015600f57600080fd5b50603580601d6000396000f3fe6080604052600080fdfea165627a7a7230582092dc1966a8880ddf11e067f9dd56a632c11a78a4afd4a9f05924d427367958cc0029a165627a7a723058202b2cc7384e11c452cdbf39b68dada2d5e10a632cc0174a354b8b8c83237e28a40029").to_vec();

	EVM::create(Origin::signed(account), contract, 0, 1000000000, 1000000000)
		.map_or_else(|e| Err(e.error), |_| Ok(()))?;

	if let Event::module_evm(module_evm::Event::Created(address)) = System::events().iter().last().unwrap().event {
		Ok(address)
	} else {
		Err("deploy_contract failed".into())
	}
}

#[test]
fn emergency_shutdown_and_cdp_treasury() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::AUSD),
				2_000_000u128,
			),
			(
				AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::AUSD),
				8_000_000u128,
			),
			(
				AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::XBTC),
				1_000_000u128,
			),
			(
				AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::DOT),
				200_000_000u128,
			),
			(
				AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::LDOT),
				40_000_000u128,
			),
		])
		.build()
		.execute_with(|| {
			assert_ok!(CdpTreasuryModule::deposit_collateral(
				&AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::XBTC),
				1_000_000
			));
			assert_ok!(CdpTreasuryModule::deposit_collateral(
				&AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::DOT),
				200_000_000
			));
			assert_ok!(CdpTreasuryModule::deposit_collateral(
				&AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::LDOT),
				40_000_000
			));
			assert_eq!(
				CdpTreasuryModule::total_collaterals(CurrencyId::Token(TokenSymbol::XBTC)),
				1_000_000
			);
			assert_eq!(
				CdpTreasuryModule::total_collaterals(CurrencyId::Token(TokenSymbol::DOT)),
				200_000_000
			);
			assert_eq!(
				CdpTreasuryModule::total_collaterals(CurrencyId::Token(TokenSymbol::LDOT)),
				40_000_000
			);

			assert_noop!(
				EmergencyShutdownModule::refund_collaterals(origin_of(AccountId::from(ALICE)), 1_000_000),
				module_emergency_shutdown::Error::<Runtime>::CanNotRefund,
			);
			assert_ok!(EmergencyShutdownModule::emergency_shutdown(
				<Runtime as frame_system::Config>::Origin::root()
			));
			assert_ok!(EmergencyShutdownModule::open_collateral_refund(
				<Runtime as frame_system::Config>::Origin::root()
			));
			assert_ok!(EmergencyShutdownModule::refund_collaterals(
				origin_of(AccountId::from(ALICE)),
				1_000_000
			));

			assert_eq!(
				CdpTreasuryModule::total_collaterals(CurrencyId::Token(TokenSymbol::XBTC)),
				900_000
			);
			assert_eq!(
				CdpTreasuryModule::total_collaterals(CurrencyId::Token(TokenSymbol::DOT)),
				180_000_000
			);
			assert_eq!(
				CdpTreasuryModule::total_collaterals(CurrencyId::Token(TokenSymbol::LDOT)),
				36_000_000
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::AUSD), &AccountId::from(ALICE)),
				1_000_000
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::XBTC), &AccountId::from(ALICE)),
				100_000
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::DOT), &AccountId::from(ALICE)),
				20_000_000
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::LDOT), &AccountId::from(ALICE)),
				4_000_000
			);
		});
}

#[test]
fn liquidate_cdp() {
	ExtBuilder::default()
		.balances(vec![
			(AccountId::from(ALICE), CurrencyId::Token(TokenSymbol::XBTC), amount(10)),
			(
				AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::AUSD),
				amount(1_000_000),
			),
			(AccountId::from(BOB), CurrencyId::Token(TokenSymbol::XBTC), amount(101)),
		])
		.build()
		.execute_with(|| {
			assert_ok!(set_oracle_price(vec![(
				CurrencyId::Token(TokenSymbol::XBTC),
				Price::saturating_from_rational(10000, 1)
			)])); // 10000 usd

			assert_ok!(DexModule::add_liquidity(
				origin_of(AccountId::from(BOB)),
				CurrencyId::Token(TokenSymbol::XBTC),
				CurrencyId::Token(TokenSymbol::AUSD),
				amount(100),
				amount(1_000_000),
				false,
			));

			assert_ok!(CdpEngineModule::set_collateral_params(
				<Runtime as frame_system::Config>::Origin::root(),
				CurrencyId::Token(TokenSymbol::XBTC),
				Change::NewValue(Some(Rate::zero())),
				Change::NewValue(Some(Ratio::saturating_from_rational(200, 100))),
				Change::NewValue(Some(Rate::saturating_from_rational(20, 100))),
				Change::NewValue(Some(Ratio::saturating_from_rational(200, 100))),
				Change::NewValue(amount(1000000)),
			));

			assert_ok!(CdpEngineModule::adjust_position(
				&AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC),
				amount(10) as i128,
				amount(500_000) as i128
			));

			assert_ok!(CdpEngineModule::adjust_position(
				&AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::XBTC),
				amount(1) as i128,
				amount(50_000) as i128
			));

			assert_eq!(
				LoansModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).debit,
				amount(500_000)
			);
			assert_eq!(
				LoansModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).collateral,
				amount(10)
			);
			assert_eq!(
				LoansModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(BOB)).debit,
				amount(50_000)
			);
			assert_eq!(
				LoansModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(BOB)).collateral,
				amount(1)
			);
			assert_eq!(CdpTreasuryModule::debit_pool(), 0);
			assert_eq!(AuctionManagerModule::collateral_auctions(0), None);

			assert_ok!(CdpEngineModule::set_collateral_params(
				<Runtime as frame_system::Config>::Origin::root(),
				CurrencyId::Token(TokenSymbol::XBTC),
				Change::NoChange,
				Change::NewValue(Some(Ratio::saturating_from_rational(400, 100))),
				Change::NoChange,
				Change::NewValue(Some(Ratio::saturating_from_rational(400, 100))),
				Change::NoChange,
			));

			assert_ok!(CdpEngineModule::liquidate_unsafe_cdp(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC)
			));

			let liquidate_alice_xbtc_cdp_event =
				Event::module_cdp_engine(module_cdp_engine::Event::LiquidateUnsafeCDP(
					CurrencyId::Token(TokenSymbol::XBTC),
					AccountId::from(ALICE),
					amount(10),
					amount(50_000),
					LiquidationStrategy::Auction,
				));
			assert!(SystemModule::events()
				.iter()
				.any(|record| record.event == liquidate_alice_xbtc_cdp_event));

			assert_eq!(
				LoansModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).debit,
				0
			);
			assert_eq!(
				LoansModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).collateral,
				0
			);
			assert_eq!(AuctionManagerModule::collateral_auctions(0).is_some(), true);
			assert_eq!(CdpTreasuryModule::debit_pool(), amount(50_000));

			assert_ok!(CdpEngineModule::liquidate_unsafe_cdp(
				AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::XBTC)
			));

			let liquidate_bob_xbtc_cdp_event = Event::module_cdp_engine(module_cdp_engine::Event::LiquidateUnsafeCDP(
				CurrencyId::Token(TokenSymbol::XBTC),
				AccountId::from(BOB),
				amount(1),
				amount(5_000),
				LiquidationStrategy::Exchange,
			));
			assert!(SystemModule::events()
				.iter()
				.any(|record| record.event == liquidate_bob_xbtc_cdp_event));

			assert_eq!(
				LoansModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(BOB)).debit,
				0
			);
			assert_eq!(
				LoansModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(BOB)).collateral,
				0
			);
			assert_eq!(CdpTreasuryModule::debit_pool(), amount(55_000));
			assert!(CdpTreasuryModule::surplus_pool() >= amount(5_000));
		});
}

#[test]
fn test_dex_module() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::AUSD),
				(1_000_000_000_000_000_000u128),
			),
			(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC),
				(1_000_000_000_000_000_000u128),
			),
			(
				AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::AUSD),
				(1_000_000_000_000_000_000u128),
			),
			(
				AccountId::from(BOB),
				CurrencyId::Token(TokenSymbol::XBTC),
				(1_000_000_000_000_000_000u128),
			),
		])
		.build()
		.execute_with(|| {
			assert_eq!(
				DexModule::get_liquidity_pool(
					CurrencyId::Token(TokenSymbol::XBTC),
					CurrencyId::Token(TokenSymbol::AUSD)
				),
				(0, 0)
			);
			assert_eq!(
				Currencies::total_issuance(CurrencyId::DEXShare(TokenSymbol::AUSD, TokenSymbol::XBTC)),
				0
			);
			assert_eq!(
				Currencies::free_balance(
					CurrencyId::DEXShare(TokenSymbol::AUSD, TokenSymbol::XBTC),
					&AccountId::from(ALICE)
				),
				0
			);

			assert_noop!(
				DexModule::add_liquidity(
					origin_of(AccountId::from(ALICE)),
					CurrencyId::Token(TokenSymbol::XBTC),
					CurrencyId::Token(TokenSymbol::AUSD),
					0,
					10000000,
					false,
				),
				module_dex::Error::<Runtime>::InvalidLiquidityIncrement,
			);

			assert_ok!(DexModule::add_liquidity(
				origin_of(AccountId::from(ALICE)),
				CurrencyId::Token(TokenSymbol::XBTC),
				CurrencyId::Token(TokenSymbol::AUSD),
				10000,
				10000000,
				false,
			));

			let add_liquidity_event = Event::module_dex(module_dex::Event::AddLiquidity(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::AUSD),
				10000000,
				CurrencyId::Token(TokenSymbol::XBTC),
				10000,
				10000000,
			));
			assert!(SystemModule::events()
				.iter()
				.any(|record| record.event == add_liquidity_event));

			assert_eq!(
				DexModule::get_liquidity_pool(
					CurrencyId::Token(TokenSymbol::XBTC),
					CurrencyId::Token(TokenSymbol::AUSD)
				),
				(10000, 10000000)
			);
			assert_eq!(
				Currencies::total_issuance(CurrencyId::DEXShare(TokenSymbol::AUSD, TokenSymbol::XBTC)),
				10000000
			);
			assert_eq!(
				Currencies::free_balance(
					CurrencyId::DEXShare(TokenSymbol::AUSD, TokenSymbol::XBTC),
					&AccountId::from(ALICE)
				),
				10000000
			);
			assert_ok!(DexModule::add_liquidity(
				origin_of(AccountId::from(BOB)),
				CurrencyId::Token(TokenSymbol::XBTC),
				CurrencyId::Token(TokenSymbol::AUSD),
				1,
				1000,
				false,
			));
			assert_eq!(
				DexModule::get_liquidity_pool(
					CurrencyId::Token(TokenSymbol::XBTC),
					CurrencyId::Token(TokenSymbol::AUSD)
				),
				(10001, 10001000)
			);
			assert_eq!(
				Currencies::total_issuance(CurrencyId::DEXShare(TokenSymbol::AUSD, TokenSymbol::XBTC)),
				10001000
			);
			assert_eq!(
				Currencies::free_balance(
					CurrencyId::DEXShare(TokenSymbol::AUSD, TokenSymbol::XBTC),
					&AccountId::from(BOB)
				),
				1000
			);
			assert_noop!(
				DexModule::add_liquidity(
					origin_of(AccountId::from(BOB)),
					CurrencyId::Token(TokenSymbol::XBTC),
					CurrencyId::Token(TokenSymbol::AUSD),
					1,
					999,
					false,
				),
				module_dex::Error::<Runtime>::InvalidLiquidityIncrement,
			);
			assert_eq!(
				DexModule::get_liquidity_pool(
					CurrencyId::Token(TokenSymbol::XBTC),
					CurrencyId::Token(TokenSymbol::AUSD)
				),
				(10001, 10001000)
			);
			assert_eq!(
				Currencies::total_issuance(CurrencyId::DEXShare(TokenSymbol::AUSD, TokenSymbol::XBTC)),
				10001000
			);
			assert_eq!(
				Currencies::free_balance(
					CurrencyId::DEXShare(TokenSymbol::AUSD, TokenSymbol::XBTC),
					&AccountId::from(BOB)
				),
				1000
			);
			assert_ok!(DexModule::add_liquidity(
				origin_of(AccountId::from(BOB)),
				CurrencyId::Token(TokenSymbol::XBTC),
				CurrencyId::Token(TokenSymbol::AUSD),
				2,
				1000,
				false,
			));
			assert_eq!(
				DexModule::get_liquidity_pool(
					CurrencyId::Token(TokenSymbol::XBTC),
					CurrencyId::Token(TokenSymbol::AUSD)
				),
				(10002, 10002000)
			);
			assert_ok!(DexModule::add_liquidity(
				origin_of(AccountId::from(BOB)),
				CurrencyId::Token(TokenSymbol::XBTC),
				CurrencyId::Token(TokenSymbol::AUSD),
				1,
				1001,
				false,
			));
			assert_eq!(
				DexModule::get_liquidity_pool(
					CurrencyId::Token(TokenSymbol::XBTC),
					CurrencyId::Token(TokenSymbol::AUSD)
				),
				(10003, 10003000)
			);

			assert_eq!(
				Currencies::total_issuance(CurrencyId::DEXShare(TokenSymbol::AUSD, TokenSymbol::XBTC)),
				10002998
			);
		});
}

#[test]
fn test_honzon_module() {
	ExtBuilder::default()
		.balances(vec![(
			AccountId::from(ALICE),
			CurrencyId::Token(TokenSymbol::XBTC),
			amount(1_000),
		)])
		.build()
		.execute_with(|| {
			assert_ok!(set_oracle_price(vec![(
				CurrencyId::Token(TokenSymbol::XBTC),
				Price::saturating_from_rational(1, 1)
			)]));

			assert_ok!(CdpEngineModule::set_collateral_params(
				<Runtime as frame_system::Config>::Origin::root(),
				CurrencyId::Token(TokenSymbol::XBTC),
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(amount(10000)),
			));
			assert_ok!(CdpEngineModule::adjust_position(
				&AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC),
				amount(100) as i128,
				amount(500) as i128
			));
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::XBTC), &AccountId::from(ALICE)),
				amount(900)
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::AUSD), &AccountId::from(ALICE)),
				amount(50)
			);
			assert_eq!(
				LoansModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).debit,
				amount(500)
			);
			assert_eq!(
				LoansModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).collateral,
				amount(100)
			);
			assert_eq!(
				CdpEngineModule::liquidate(
					<Runtime as frame_system::Config>::Origin::none(),
					CurrencyId::Token(TokenSymbol::XBTC),
					MultiAddress::Id(AccountId::from(ALICE))
				)
				.is_ok(),
				false
			);
			assert_ok!(CdpEngineModule::set_collateral_params(
				<Runtime as frame_system::Config>::Origin::root(),
				CurrencyId::Token(TokenSymbol::XBTC),
				Change::NoChange,
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 1))),
				Change::NoChange,
				Change::NoChange,
				Change::NoChange,
			));
			assert_ok!(CdpEngineModule::liquidate(
				<Runtime as frame_system::Config>::Origin::none(),
				CurrencyId::Token(TokenSymbol::XBTC),
				MultiAddress::Id(AccountId::from(ALICE))
			));

			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::XBTC), &AccountId::from(ALICE)),
				amount(900)
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::AUSD), &AccountId::from(ALICE)),
				amount(50)
			);
			assert_eq!(
				LoansModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).debit,
				0
			);
			assert_eq!(
				LoansModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).collateral,
				0
			);
		});
}

#[test]
fn test_cdp_engine_module() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::AUSD),
				amount(1000),
			),
			(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC),
				amount(1000),
			),
		])
		.build()
		.execute_with(|| {
			assert_ok!(CdpEngineModule::set_collateral_params(
				<Runtime as frame_system::Config>::Origin::root(),
				CurrencyId::Token(TokenSymbol::XBTC),
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(amount(10000)),
			));

			let new_collateral_params = CdpEngineModule::collateral_params(CurrencyId::Token(TokenSymbol::XBTC));

			assert_eq!(
				new_collateral_params.stability_fee,
				Some(Rate::saturating_from_rational(1, 100000))
			);
			assert_eq!(
				new_collateral_params.liquidation_ratio,
				Some(Ratio::saturating_from_rational(3, 2))
			);
			assert_eq!(
				new_collateral_params.liquidation_penalty,
				Some(Rate::saturating_from_rational(2, 10))
			);
			assert_eq!(
				new_collateral_params.required_collateral_ratio,
				Some(Ratio::saturating_from_rational(9, 5))
			);
			assert_eq!(new_collateral_params.maximum_total_debit_value, amount(10000));

			assert_eq!(
				CdpEngineModule::calculate_collateral_ratio(
					CurrencyId::Token(TokenSymbol::XBTC),
					100,
					50,
					Price::saturating_from_rational(1, 1)
				),
				Ratio::saturating_from_rational(100 * 10, 50)
			);

			assert_ok!(CdpEngineModule::check_debit_cap(
				CurrencyId::Token(TokenSymbol::XBTC),
				amount(99999)
			));
			assert_eq!(
				CdpEngineModule::check_debit_cap(CurrencyId::Token(TokenSymbol::XBTC), amount(100001)).is_ok(),
				false
			);

			assert_ok!(CdpEngineModule::adjust_position(
				&AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC),
				amount(100) as i128,
				0
			));
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::XBTC), &AccountId::from(ALICE)),
				amount(900)
			);
			assert_eq!(
				LoansModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).debit,
				0
			);
			assert_eq!(
				LoansModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).collateral,
				amount(100)
			);

			assert_noop!(
				CdpEngineModule::settle_cdp_has_debit(AccountId::from(ALICE), CurrencyId::Token(TokenSymbol::XBTC)),
				module_cdp_engine::Error::<Runtime>::NoDebitValue,
			);

			assert_ok!(set_oracle_price(vec![
				(
					CurrencyId::Token(TokenSymbol::AUSD),
					Price::saturating_from_rational(1, 1)
				),
				(
					CurrencyId::Token(TokenSymbol::XBTC),
					Price::saturating_from_rational(3, 1)
				)
			]));

			assert_ok!(CdpEngineModule::adjust_position(
				&AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC),
				0,
				amount(100) as i128
			));
			assert_eq!(
				LoansModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).debit,
				amount(100)
			);
			assert_eq!(CdpTreasuryModule::debit_pool(), 0);
			assert_eq!(
				CdpTreasuryModule::total_collaterals(CurrencyId::Token(TokenSymbol::XBTC)),
				0
			);
			assert_ok!(CdpEngineModule::settle_cdp_has_debit(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC)
			));

			let settle_cdp_in_debit_event = Event::module_cdp_engine(module_cdp_engine::Event::SettleCDPInDebit(
				CurrencyId::Token(TokenSymbol::XBTC),
				AccountId::from(ALICE),
			));
			assert!(SystemModule::events()
				.iter()
				.any(|record| record.event == settle_cdp_in_debit_event));

			assert_eq!(
				LoansModule::positions(CurrencyId::Token(TokenSymbol::XBTC), AccountId::from(ALICE)).debit,
				0
			);
			assert_eq!(CdpTreasuryModule::debit_pool(), amount(10));
			assert_eq!(
				CdpTreasuryModule::total_collaterals(CurrencyId::Token(TokenSymbol::XBTC)),
				3333333333333333330
			);
		});
}

#[test]
fn test_authority_module() {
	const AUTHORITY_ORIGIN_ID: u8 = 33u8;

	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::AUSD),
				amount(1000),
			),
			(
				AccountId::from(ALICE),
				CurrencyId::Token(TokenSymbol::XBTC),
				amount(1000),
			),
			(
				DSWFModuleId::get().into_account(),
				CurrencyId::Token(TokenSymbol::AUSD),
				amount(1000),
			),
		])
		.build()
		.execute_with(|| {
			let ensure_root_call = Call::System(frame_system::Call::fill_block(Perbill::one()));
			let call = Call::Authority(orml_authority::Call::dispatch_as(
				AuthoritysOriginId::Root,
				Box::new(ensure_root_call.clone()),
			));

			// dispatch_as
			assert_ok!(AuthorityModule::dispatch_as(
				Origin::root(),
				AuthoritysOriginId::Root,
				Box::new(ensure_root_call.clone())
			));

			assert_noop!(
				AuthorityModule::dispatch_as(
					Origin::signed(AccountId::from(BOB)),
					AuthoritysOriginId::Root,
					Box::new(ensure_root_call.clone())
				),
				BadOrigin
			);

			assert_noop!(
				AuthorityModule::dispatch_as(
					Origin::signed(AccountId::from(BOB)),
					AuthoritysOriginId::AcalaTreasury,
					Box::new(ensure_root_call.clone())
				),
				BadOrigin
			);

			// schedule_dispatch
			run_to_block(1);
			// DSWF transfer
			let transfer_call = Call::Currencies(module_currencies::Call::transfer(
				AccountId::from(BOB).into(),
				CurrencyId::Token(TokenSymbol::AUSD),
				amount(500),
			));
			let dswf_call = Call::Authority(orml_authority::Call::dispatch_as(
				AuthoritysOriginId::DSWF,
				Box::new(transfer_call.clone()),
			));
			assert_ok!(AuthorityModule::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(2),
				0,
				true,
				Box::new(dswf_call.clone())
			));

			assert_ok!(AuthorityModule::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(2),
				0,
				true,
				Box::new(call.clone())
			));

			let event = Event::orml_authority(orml_authority::Event::Scheduled(
				OriginCaller::orml_authority(DelayedOrigin {
					delay: 1,
					origin: Box::new(OriginCaller::system(RawOrigin::Root)),
				}),
				1,
			));
			assert_eq!(last_event(), event);

			run_to_block(2);
			assert_eq!(
				Currencies::free_balance(
					CurrencyId::Token(TokenSymbol::AUSD),
					&DSWFModuleId::get().into_account()
				),
				amount(500)
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Token(TokenSymbol::AUSD), &AccountId::from(BOB)),
				amount(500)
			);

			// delay < SevenDays
			let event = Event::pallet_scheduler(pallet_scheduler::RawEvent::Dispatched(
				(2, 1),
				Some([AUTHORITY_ORIGIN_ID, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0].to_vec()),
				Err(DispatchError::BadOrigin),
			));
			assert_eq!(last_event(), event);

			// delay = SevenDays
			assert_ok!(AuthorityModule::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(SevenDays::get() + 2),
				0,
				true,
				Box::new(call.clone())
			));

			run_to_block(SevenDays::get() + 2);
			let event = Event::pallet_scheduler(pallet_scheduler::RawEvent::Dispatched(
				(151202, 0),
				Some([AUTHORITY_ORIGIN_ID, 160, 78, 2, 0, 0, 0, 2, 0, 0, 0].to_vec()),
				Ok(()),
			));
			assert_eq!(last_event(), event);

			// with_delayed_origin = false
			assert_ok!(AuthorityModule::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(SevenDays::get() + 3),
				0,
				false,
				Box::new(call.clone())
			));
			let event = Event::orml_authority(orml_authority::Event::Scheduled(
				OriginCaller::system(RawOrigin::Root),
				3,
			));
			assert_eq!(last_event(), event);

			run_to_block(SevenDays::get() + 3);
			let event = Event::pallet_scheduler(pallet_scheduler::RawEvent::Dispatched(
				(151203, 0),
				Some([0, 0, 3, 0, 0, 0].to_vec()),
				Ok(()),
			));
			assert_eq!(last_event(), event);

			assert_ok!(AuthorityModule::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(SevenDays::get() + 4),
				0,
				false,
				Box::new(call.clone())
			));

			// fast_track_scheduled_dispatch
			assert_ok!(AuthorityModule::fast_track_scheduled_dispatch(
				Origin::root(),
				frame_system::RawOrigin::Root.into(),
				4,
				DispatchTime::At(SevenDays::get() + 5),
			));

			// delay_scheduled_dispatch
			assert_ok!(AuthorityModule::delay_scheduled_dispatch(
				Origin::root(),
				frame_system::RawOrigin::Root.into(),
				4,
				4,
			));

			// cancel_scheduled_dispatch
			assert_ok!(AuthorityModule::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(SevenDays::get() + 4),
				0,
				true,
				Box::new(call.clone())
			));
			let event = Event::orml_authority(orml_authority::Event::Scheduled(
				OriginCaller::orml_authority(DelayedOrigin {
					delay: 1,
					origin: Box::new(OriginCaller::system(RawOrigin::Root)),
				}),
				5,
			));
			assert_eq!(last_event(), event);

			let schedule_origin = {
				let origin: <Runtime as orml_authority::Config>::Origin = From::from(Origin::root());
				let origin: <Runtime as orml_authority::Config>::Origin = From::from(DelayedOrigin::<
					BlockNumber,
					<Runtime as orml_authority::Config>::PalletsOrigin,
				> {
					delay: 1,
					origin: Box::new(origin.caller().clone()),
				});
				origin
			};

			let pallets_origin = schedule_origin.caller().clone();
			assert_ok!(AuthorityModule::cancel_scheduled_dispatch(
				Origin::root(),
				pallets_origin,
				5
			));
			let event = Event::orml_authority(orml_authority::Event::Cancelled(
				OriginCaller::orml_authority(DelayedOrigin {
					delay: 1,
					origin: Box::new(OriginCaller::system(RawOrigin::Root)),
				}),
				5,
			));
			assert_eq!(last_event(), event);

			assert_ok!(AuthorityModule::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(SevenDays::get() + 5),
				0,
				false,
				Box::new(call.clone())
			));
			let event = Event::orml_authority(orml_authority::Event::Scheduled(
				OriginCaller::system(RawOrigin::Root),
				6,
			));
			assert_eq!(last_event(), event);

			assert_ok!(AuthorityModule::cancel_scheduled_dispatch(
				Origin::root(),
				frame_system::RawOrigin::Root.into(),
				6
			));
			let event = Event::orml_authority(orml_authority::Event::Cancelled(
				OriginCaller::system(RawOrigin::Root),
				6,
			));
			assert_eq!(last_event(), event);
		});
}

#[test]
fn test_nft_module() {
	ExtBuilder::default()
		.balances(vec![(
			AccountId::from(ALICE),
			CurrencyId::Token(TokenSymbol::ACA),
			amount(1000),
		)])
		.build()
		.execute_with(|| {
			assert_eq!(Balances::free_balance(AccountId::from(ALICE)), amount(1000));
			assert_ok!(NFT::create_class(
				origin_of(AccountId::from(ALICE)),
				vec![1],
				module_nft::Properties(module_nft::ClassProperty::Transferable | module_nft::ClassProperty::Burnable)
			));
			assert_eq!(
				Balances::deposit_into_existing(&NftModuleId::get().into_sub_account(0), 1 * CreateTokenDeposit::get())
					.is_ok(),
				true
			);
			assert_ok!(NFT::mint(
				origin_of(NftModuleId::get().into_sub_account(0)),
				MultiAddress::Id(AccountId::from(BOB)),
				0,
				vec![1],
				1
			));
			assert_ok!(NFT::burn(origin_of(AccountId::from(BOB)), (0, 0)));
			assert_eq!(Balances::free_balance(AccountId::from(BOB)), CreateTokenDeposit::get());
			assert_ok!(NFT::destroy_class(
				origin_of(NftModuleId::get().into_sub_account(0)),
				0,
				MultiAddress::Id(AccountId::from(BOB))
			));
			assert_eq!(
				Balances::free_balance(AccountId::from(BOB)),
				CreateClassDeposit::get() + CreateTokenDeposit::get()
			);
			assert_eq!(Balances::reserved_balance(AccountId::from(BOB)), 0);
			assert_eq!(
				Balances::free_balance(AccountId::from(ALICE)),
				amount(1000) - (CreateClassDeposit::get() + Proxy::deposit(1u32))
			);
		});
}

#[test]
fn test_evm_accounts_module() {
	ExtBuilder::default()
		.balances(vec![(
			bob_account_id(),
			CurrencyId::Token(TokenSymbol::ACA),
			amount(1000),
		)])
		.build()
		.execute_with(|| {
			assert_eq!(Balances::free_balance(AccountId::from(ALICE)), 0);
			assert_eq!(Balances::free_balance(bob_account_id()), 1000000000000000000000);
			assert_ok!(EvmAccounts::claim_account(
				Origin::signed(AccountId::from(ALICE)),
				EvmAccounts::eth_address(&alice()),
				EvmAccounts::eth_sign(&alice(), &AccountId::from(ALICE).encode(), &[][..])
			));
			let event = Event::module_evm_accounts(module_evm_accounts::Event::ClaimAccount(
				AccountId::from(ALICE),
				EvmAccounts::eth_address(&alice()),
			));
			assert_eq!(last_event(), event);

			// claim another eth address
			assert_noop!(
				EvmAccounts::claim_account(
					Origin::signed(AccountId::from(ALICE)),
					EvmAccounts::eth_address(&alice()),
					EvmAccounts::eth_sign(&alice(), &AccountId::from(ALICE).encode(), &[][..])
				),
				module_evm_accounts::Error::<Runtime>::AccountIdHasMapped
			);
			assert_noop!(
				EvmAccounts::claim_account(
					Origin::signed(AccountId::from(BOB)),
					EvmAccounts::eth_address(&alice()),
					EvmAccounts::eth_sign(&alice(), &AccountId::from(BOB).encode(), &[][..])
				),
				module_evm_accounts::Error::<Runtime>::EthAddressHasMapped
			);
		});
}

#[cfg(not(feature = "with-ethereum-compatibility"))]
#[test]
fn test_evm_module() {
	ExtBuilder::default()
		.balances(vec![
			(alice_account_id(), CurrencyId::Token(TokenSymbol::ACA), amount(1000)),
			(bob_account_id(), CurrencyId::Token(TokenSymbol::ACA), amount(1000)),
		])
		.build()
		.execute_with(|| {
			assert_eq!(Balances::free_balance(alice_account_id()), amount(1000));
			assert_eq!(Balances::free_balance(bob_account_id()), amount(1000));

			let _alice_address = EvmAccounts::eth_address(&alice());
			let bob_address = EvmAccounts::eth_address(&bob());

			let contract = deploy_contract(alice_account_id()).unwrap();
			let event = Event::module_evm(module_evm::Event::Created(contract));
			assert_eq!(last_event(), event);

			assert_ok!(EVM::transfer_maintainer(
				Origin::signed(alice_account_id()),
				contract,
				bob_address
			));
			let event = Event::module_evm(module_evm::Event::TransferredMaintainer(contract, bob_address));
			assert_eq!(last_event(), event);

			// test EvmAccounts Lookup
			assert_eq!(Balances::free_balance(alice_account_id()), 999999896330000000000);
			assert_eq!(Balances::free_balance(bob_account_id()), amount(1000));
			let to = EvmAccounts::eth_address(&alice());
			assert_ok!(Currencies::transfer(
				Origin::signed(bob_account_id()),
				MultiAddress::Address20(to.0),
				CurrencyId::Token(TokenSymbol::ACA),
				amount(10)
			));
			assert_eq!(Balances::free_balance(alice_account_id()), 1009999896330000000000);
			assert_eq!(Balances::free_balance(bob_account_id()), amount(1000) - amount(10));
		});
}

#[cfg(feature = "with-ethereum-compatibility")]
#[test]
fn test_evm_module() {
	ExtBuilder::default()
		.balances(vec![
			(alice_account_id(), CurrencyId::Token(TokenSymbol::ACA), amount(1000)),
			(bob_account_id(), CurrencyId::Token(TokenSymbol::ACA), amount(1000)),
		])
		.build()
		.execute_with(|| {
			assert_eq!(Balances::free_balance(alice_account_id()), amount(1000));
			assert_eq!(Balances::free_balance(bob_account_id()), amount(1000));

			use std::fs::{self, File};
			use std::io::Read;

			let paths = fs::read_dir("../../runtime/mandala/tests/solidity_test").unwrap();
			let file_names = paths
				.filter_map(|entry| entry.ok().and_then(|e| e.path().to_str().map(|s| String::from(s))))
				.collect::<Vec<String>>();

			for file in file_names {
				let mut f = File::open(&file).expect("File not found");
				let mut contents = String::new();
				f.read_to_string(&mut contents)
					.expect("Something went wrong reading the file.");
				let json: serde_json::Value = serde_json::from_str(&contents).unwrap();

				let bytecode_str = serde_json::to_string(&json["bytecode"]).unwrap();
				let bytecode_str = bytecode_str.replace("\"", "");

				let bytecode = hex::decode(bytecode_str).unwrap();
				assert_ok!(EVM::create(
					Origin::signed(alice_account_id()),
					bytecode,
					0,
					u64::MAX,
					u32::MAX
				));

				match System::events().iter().last().unwrap().event {
					Event::module_evm(module_evm::Event::Created(_)) => {}
					_ => {
						println!(
							"contract {:?} create failed, event: {:?}",
							file,
							System::events().iter().last().unwrap().event
						);
						assert!(false);
					}
				};
			}
		});
}
