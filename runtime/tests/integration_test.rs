#[cfg(test)]

mod tests {
	use acala_runtime::{
		AccountId, Balance, CurrencyId,
		CurrencyId::{AUSD, DOT, LDOT, XBTC},
		Runtime,
	};
	use frame_support::{
		assert_noop, assert_ok,
		traits::{OnFinalize, OnInitialize},
	};
	use module_cdp_engine::LiquidationStrategy;
	use module_support::CDPTreasury;
	use module_support::{Price, Rate, Ratio, RiskManager};
	use orml_traits::MultiCurrency;
	use sp_runtime::DispatchResult;

	const ORACLE1: [u8; 32] = [0u8; 32];
	const ORACLE2: [u8; 32] = [1u8; 32];
	const ORACLE3: [u8; 32] = [2u8; 32];

	const ALICE: [u8; 32] = [4u8; 32];
	const BOB: [u8; 32] = [5u8; 32];

	pub type OracleModule = orml_oracle::Module<Runtime>;
	pub type DexModule = module_dex::Module<Runtime>;
	pub type CdpEngineModule = module_cdp_engine::Module<Runtime>;
	pub type LoansModule = module_loans::Module<Runtime>;
	pub type CdpTreasuryModule = module_cdp_treasury::Module<Runtime>;
	pub type SystemModule = system::Module<Runtime>;
	pub type EmergencyShutdownModule = module_emergency_shutdown::Module<Runtime>;
	pub type AuctionManagerModule = module_auction_manager::Module<Runtime>;
	pub type Currencies = orml_currencies::Module<Runtime>;

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
			let mut t = system::GenesisConfig::default().build_storage::<Runtime>().unwrap();

			orml_tokens::GenesisConfig::<Runtime> {
				endowed_accounts: self.endowed_accounts,
			}
			.assimilate_storage(&mut t)
			.unwrap();

			module_dex::GenesisConfig::<Runtime> {
				liquidity_incentive_rate: vec![(XBTC, Rate::from_rational(1, 100))],
			}
			.assimilate_storage(&mut t)
			.unwrap();

			pallet_collective::GenesisConfig::<Runtime, pallet_collective::Instance5> {
				members: vec![
					AccountId::from(ORACLE1),
					AccountId::from(ORACLE2),
					AccountId::from(ORACLE3),
				],
				phantom: Default::default(),
			}
			.assimilate_storage(&mut t)
			.unwrap();

			t.into()
		}
	}

	pub fn origin_of(account_id: AccountId) -> <Runtime as system::Trait>::Origin {
		<Runtime as system::Trait>::Origin::signed(account_id)
	}

	fn set_oracle_price(prices: Vec<(CurrencyId, Price)>) -> DispatchResult {
		OracleModule::on_finalize(0);
		assert_ok!(OracleModule::feed_values(
			origin_of(AccountId::from(ORACLE1)),
			prices.clone()
		));
		assert_ok!(OracleModule::feed_values(
			origin_of(AccountId::from(ORACLE2)),
			prices.clone()
		));
		assert_ok!(OracleModule::feed_values(origin_of(AccountId::from(ORACLE3)), prices));
		Ok(())
	}

	fn amount(amount: u128) -> u128 {
		amount.saturating_mul(Price::accuracy())
	}

	#[test]
	fn emergency_shutdown_and_cdp_treasury() {
		ExtBuilder::default()
			.balances(vec![
				(AccountId::from(ALICE), AUSD, 2_000_000u128),
				(AccountId::from(BOB), AUSD, 8_000_000u128),
				(AccountId::from(BOB), XBTC, 1_000_000u128),
				(AccountId::from(BOB), DOT, 200_000_000u128),
				(AccountId::from(BOB), LDOT, 40_000_000u128),
			])
			.build()
			.execute_with(|| {
				assert_ok!(CdpTreasuryModule::transfer_collateral_from(
					XBTC,
					&AccountId::from(BOB),
					1_000_000
				));
				assert_ok!(CdpTreasuryModule::transfer_collateral_from(
					DOT,
					&AccountId::from(BOB),
					200_000_000
				));
				assert_ok!(CdpTreasuryModule::transfer_collateral_from(
					LDOT,
					&AccountId::from(BOB),
					40_000_000
				));
				assert_eq!(CdpTreasuryModule::total_collaterals(XBTC), 1_000_000);
				assert_eq!(CdpTreasuryModule::total_collaterals(DOT), 200_000_000);
				assert_eq!(CdpTreasuryModule::total_collaterals(LDOT), 40_000_000);

				assert_noop!(
					EmergencyShutdownModule::refund_collaterals(origin_of(AccountId::from(ALICE)), 1_000_000),
					module_emergency_shutdown::Error::<Runtime>::CanNotRefund,
				);
				assert_ok!(EmergencyShutdownModule::emergency_shutdown(
					<acala_runtime::Runtime as system::Trait>::Origin::ROOT
				));
				assert_ok!(EmergencyShutdownModule::open_collateral_refund(
					<acala_runtime::Runtime as system::Trait>::Origin::ROOT
				));
				assert_ok!(EmergencyShutdownModule::refund_collaterals(
					origin_of(AccountId::from(ALICE)),
					1_000_000
				));

				assert_eq!(CdpTreasuryModule::total_collaterals(XBTC), 900_000);
				assert_eq!(CdpTreasuryModule::total_collaterals(DOT), 180_000_000);
				assert_eq!(CdpTreasuryModule::total_collaterals(LDOT), 36_000_000);
				assert_eq!(Currencies::free_balance(AUSD, &AccountId::from(ALICE)), 1_000_000);
				assert_eq!(Currencies::free_balance(XBTC, &AccountId::from(ALICE)), 100_000);
				assert_eq!(Currencies::free_balance(DOT, &AccountId::from(ALICE)), 20_000_000);
				assert_eq!(Currencies::free_balance(LDOT, &AccountId::from(ALICE)), 4_000_000);
			});
	}

	#[test]
	fn liquidate_cdp() {
		ExtBuilder::default()
			.balances(vec![
				(AccountId::from(BOB), AUSD, amount(1_000_000)),
				(AccountId::from(ALICE), XBTC, amount(10)),
				(AccountId::from(BOB), XBTC, amount(101)),
			])
			.build()
			.execute_with(|| {
				SystemModule::set_block_number(1);
				assert_ok!(set_oracle_price(vec![(XBTC, Price::from_rational(10000, 1))])); // 10000 usd

				assert_ok!(DexModule::add_liquidity(
					origin_of(AccountId::from(BOB)),
					XBTC,
					amount(100),
					amount(1_000_000)
				));

				assert_ok!(CdpEngineModule::set_collateral_params(
					<acala_runtime::Runtime as system::Trait>::Origin::ROOT,
					XBTC,
					Some(Some(Rate::from_natural(0))),
					Some(Some(Ratio::from_rational(200, 100))),
					Some(Some(Rate::from_rational(20, 100))),
					Some(Some(Ratio::from_rational(200, 100))),
					Some(amount(1000000)),
				));

				assert_ok!(CdpEngineModule::adjust_position(
					&AccountId::from(ALICE),
					XBTC,
					amount(10) as i128,
					amount(500_000) as i128
				));

				assert_ok!(CdpEngineModule::adjust_position(
					&AccountId::from(BOB),
					XBTC,
					amount(1) as i128,
					amount(50_000) as i128
				));

				assert_eq!(LoansModule::debits(XBTC, AccountId::from(ALICE)), amount(500_000));
				assert_eq!(LoansModule::collaterals(AccountId::from(ALICE), XBTC), amount(10));
				assert_eq!(LoansModule::debits(XBTC, AccountId::from(BOB)), amount(50_000));
				assert_eq!(LoansModule::collaterals(AccountId::from(BOB), XBTC), amount(1));
				assert_eq!(CdpTreasuryModule::debit_pool(), 0);
				assert_eq!(AuctionManagerModule::collateral_auctions(0), None);

				assert_ok!(CdpEngineModule::set_collateral_params(
					<acala_runtime::Runtime as system::Trait>::Origin::ROOT,
					XBTC,
					None,
					Some(Some(Ratio::from_rational(400, 100))),
					None,
					Some(Some(Ratio::from_rational(400, 100))),
					None,
				));

				assert_ok!(CdpEngineModule::liquidate_unsafe_cdp(AccountId::from(ALICE), XBTC));

				let liquidate_alice_xbtc_cdp_event =
					acala_runtime::Event::module_cdp_engine(module_cdp_engine::RawEvent::LiquidateUnsafeCDP(
						XBTC,
						AccountId::from(ALICE),
						amount(10),
						amount(50_000),
						LiquidationStrategy::Auction,
					));
				assert!(SystemModule::events()
					.iter()
					.any(|record| record.event == liquidate_alice_xbtc_cdp_event));

				assert_eq!(LoansModule::debits(XBTC, AccountId::from(ALICE)), 0);
				assert_eq!(LoansModule::collaterals(AccountId::from(ALICE), XBTC), 0);
				assert_eq!(AuctionManagerModule::collateral_auctions(0).is_some(), true);
				assert_eq!(CdpTreasuryModule::debit_pool(), amount(50_000));

				assert_ok!(CdpEngineModule::liquidate_unsafe_cdp(AccountId::from(BOB), XBTC));

				let liquidate_bob_xbtc_cdp_event =
					acala_runtime::Event::module_cdp_engine(module_cdp_engine::RawEvent::LiquidateUnsafeCDP(
						XBTC,
						AccountId::from(BOB),
						amount(1),
						amount(5_000),
						LiquidationStrategy::Exchange,
					));
				assert!(SystemModule::events()
					.iter()
					.any(|record| record.event == liquidate_bob_xbtc_cdp_event));

				assert_eq!(LoansModule::debits(XBTC, AccountId::from(BOB)), 0);
				assert_eq!(LoansModule::collaterals(AccountId::from(BOB), XBTC), 0);
				assert_eq!(CdpTreasuryModule::debit_pool(), amount(55_000));
				assert!(CdpTreasuryModule::surplus_pool() >= amount(5_000));
			});
	}

	#[test]
	fn test_dex_module() {
		ExtBuilder::default()
			.balances(vec![
				(AccountId::from(ALICE), AUSD, (1_000_000_000_000_000_000u128)),
				(AccountId::from(ALICE), XBTC, (1_000_000_000_000_000_000u128)),
				(AccountId::from(BOB), AUSD, (1_000_000_000_000_000_000u128)),
				(AccountId::from(BOB), XBTC, (1_000_000_000_000_000_000u128)),
			])
			.build()
			.execute_with(|| {
				SystemModule::set_block_number(1);
				assert_eq!(DexModule::calculate_swap_target_amount(10000, 10000, 10000), 4995);
				assert!(DexModule::calculate_swap_supply_amount(10000, 10000, 4995) >= 9996);

				assert_eq!(DexModule::liquidity_pool(XBTC), (0, 0));
				assert_eq!(DexModule::total_shares(XBTC), 0);
				assert_eq!(DexModule::shares(XBTC, AccountId::from(ALICE)), 0);

				assert_noop!(
					DexModule::add_liquidity(origin_of(AccountId::from(ALICE)), XBTC, 0, 10000000),
					module_dex::Error::<Runtime>::InvalidBalance,
				);

				assert_ok!(DexModule::add_liquidity(
					origin_of(AccountId::from(ALICE)),
					XBTC,
					10000,
					10000000
				));

				let add_liquidity_event = acala_runtime::Event::module_dex(module_dex::RawEvent::AddLiquidity(
					AccountId::from(ALICE),
					XBTC,
					10000,
					10000000,
					10000000,
				));
				assert!(SystemModule::events()
					.iter()
					.any(|record| record.event == add_liquidity_event));

				assert_eq!(DexModule::liquidity_pool(XBTC), (10000, 10000000));
				assert_eq!(DexModule::total_shares(XBTC), 10000000);
				assert_eq!(DexModule::shares(XBTC, AccountId::from(ALICE)), 10000000);
				assert_ok!(DexModule::add_liquidity(origin_of(AccountId::from(BOB)), XBTC, 1, 1000));
				assert_eq!(DexModule::liquidity_pool(XBTC), (10001, 10001000));
				assert_eq!(DexModule::total_shares(XBTC), 10001000);
				assert_eq!(DexModule::shares(XBTC, AccountId::from(BOB)), 1000);
				assert_noop!(
					DexModule::add_liquidity(origin_of(AccountId::from(BOB)), XBTC, 1, 999),
					module_dex::Error::<Runtime>::InvalidLiquidityIncrement,
				);
				assert_eq!(DexModule::liquidity_pool(XBTC), (10001, 10001000));
				assert_eq!(DexModule::total_shares(XBTC), 10001000);
				assert_eq!(DexModule::shares(XBTC, AccountId::from(BOB)), 1000);
				assert_ok!(DexModule::add_liquidity(origin_of(AccountId::from(BOB)), XBTC, 2, 1000));
				assert_eq!(DexModule::liquidity_pool(XBTC), (10002, 10002000));
				assert_ok!(DexModule::add_liquidity(origin_of(AccountId::from(BOB)), XBTC, 1, 1001));
				assert_eq!(DexModule::liquidity_pool(XBTC), (10003, 10003000));

				assert_eq!(DexModule::total_shares(XBTC), 10002998);
				assert_eq!(DexModule::total_interest(XBTC), 0);
				DexModule::on_initialize(0);
				assert_eq!(DexModule::total_interest(XBTC), 100030);
				DexModule::on_initialize(0);
				assert_eq!(DexModule::total_interest(XBTC), 200060);
			});
	}

	#[test]
	fn test_honzon_module() {
		ExtBuilder::default()
			.balances(vec![(AccountId::from(ALICE), XBTC, amount(1_000))])
			.build()
			.execute_with(|| {
				assert_ok!(set_oracle_price(vec![(XBTC, Price::from_rational(1, 1))]));

				assert_ok!(CdpEngineModule::set_collateral_params(
					<acala_runtime::Runtime as system::Trait>::Origin::ROOT,
					XBTC,
					Some(Some(Rate::from_rational(1, 100000))),
					Some(Some(Ratio::from_rational(3, 2))),
					Some(Some(Rate::from_rational(2, 10))),
					Some(Some(Ratio::from_rational(9, 5))),
					Some(amount(10000)),
				));
				assert_ok!(CdpEngineModule::adjust_position(
					&AccountId::from(ALICE),
					XBTC,
					amount(100) as i128,
					amount(500) as i128
				));
				assert_eq!(Currencies::free_balance(XBTC, &AccountId::from(ALICE)), amount(900));
				assert_eq!(Currencies::free_balance(AUSD, &AccountId::from(ALICE)), amount(50));
				assert_eq!(LoansModule::debits(XBTC, AccountId::from(ALICE)), amount(500));
				assert_eq!(LoansModule::collaterals(AccountId::from(ALICE), XBTC), amount(100));
				assert_eq!(
					CdpEngineModule::liquidate(<Runtime as system::Trait>::Origin::NONE, XBTC, AccountId::from(ALICE))
						.is_ok(),
					false
				);
				assert_ok!(CdpEngineModule::set_collateral_params(
					<acala_runtime::Runtime as system::Trait>::Origin::ROOT,
					XBTC,
					None,
					Some(Some(Ratio::from_rational(3, 1))),
					None,
					None,
					None
				));
				assert_ok!(CdpEngineModule::liquidate(
					<Runtime as system::Trait>::Origin::NONE,
					XBTC,
					AccountId::from(ALICE)
				));

				assert_eq!(Currencies::free_balance(XBTC, &AccountId::from(ALICE)), amount(900));
				assert_eq!(Currencies::free_balance(AUSD, &AccountId::from(ALICE)), amount(50));
				assert_eq!(LoansModule::debits(XBTC, AccountId::from(ALICE)), 0);
				assert_eq!(LoansModule::collaterals(AccountId::from(ALICE), XBTC), 0);
			});
	}

	#[test]
	fn test_cdp_engine_module() {
		ExtBuilder::default()
			.balances(vec![
				(AccountId::from(ALICE), AUSD, amount(1000)),
				(AccountId::from(ALICE), XBTC, amount(1000)),
			])
			.build()
			.execute_with(|| {
				SystemModule::set_block_number(1);
				assert_ok!(CdpEngineModule::set_collateral_params(
					<acala_runtime::Runtime as system::Trait>::Origin::ROOT,
					XBTC,
					Some(Some(Rate::from_rational(1, 100000))),
					Some(Some(Ratio::from_rational(3, 2))),
					Some(Some(Rate::from_rational(2, 10))),
					Some(Some(Ratio::from_rational(9, 5))),
					Some(amount(10000)),
				));

				assert_eq!(
					CdpEngineModule::stability_fee(XBTC),
					Some(Rate::from_rational(1, 100000))
				);
				assert_eq!(
					CdpEngineModule::liquidation_ratio(XBTC),
					Some(Ratio::from_rational(3, 2))
				);
				assert_eq!(
					CdpEngineModule::liquidation_penalty(XBTC),
					Some(Rate::from_rational(2, 10))
				);
				assert_eq!(
					CdpEngineModule::required_collateral_ratio(XBTC),
					Some(Ratio::from_rational(9, 5))
				);
				assert_eq!(CdpEngineModule::maximum_total_debit_value(XBTC), amount(10000));

				assert_eq!(
					CdpEngineModule::calculate_collateral_ratio(XBTC, 100, 50, Price::from_rational(1, 1)),
					Ratio::from_rational(100 * 10, 50)
				);

				assert_ok!(CdpEngineModule::check_debit_cap(XBTC, amount(99999)));
				assert_eq!(CdpEngineModule::check_debit_cap(XBTC, amount(100001)).is_ok(), false);

				assert_ok!(CdpEngineModule::adjust_position(
					&AccountId::from(ALICE),
					XBTC,
					amount(100) as i128,
					0
				));
				assert_eq!(Currencies::free_balance(XBTC, &AccountId::from(ALICE)), amount(900));
				assert_eq!(LoansModule::debits(XBTC, AccountId::from(ALICE)), 0);
				assert_eq!(LoansModule::collaterals(AccountId::from(ALICE), XBTC), amount(100));

				assert_noop!(
					CdpEngineModule::settle_cdp_has_debit(AccountId::from(ALICE), XBTC),
					module_cdp_engine::Error::<Runtime>::AlreadyNoDebit,
				);

				assert_ok!(set_oracle_price(vec![
					(AUSD, Price::from_rational(1, 1)),
					(XBTC, Price::from_rational(3, 1))
				]));

				assert_ok!(CdpEngineModule::adjust_position(
					&AccountId::from(ALICE),
					XBTC,
					0,
					amount(100) as i128
				));
				assert_eq!(LoansModule::debits(XBTC, AccountId::from(ALICE)), amount(100));
				assert_eq!(CdpTreasuryModule::debit_pool(), 0);
				assert_eq!(CdpTreasuryModule::total_collaterals(XBTC), 0);
				assert_ok!(CdpEngineModule::settle_cdp_has_debit(AccountId::from(ALICE), XBTC));

				let settle_cdp_in_debit_event = acala_runtime::Event::module_cdp_engine(
					module_cdp_engine::RawEvent::SettleCDPInDebit(XBTC, AccountId::from(ALICE)),
				);
				assert!(SystemModule::events()
					.iter()
					.any(|record| record.event == settle_cdp_in_debit_event));

				assert_eq!(LoansModule::debits(XBTC, AccountId::from(ALICE)), 0);
				assert_eq!(CdpTreasuryModule::debit_pool(), amount(10));
				assert_eq!(CdpTreasuryModule::total_collaterals(XBTC), 3333333333333333330);
			});
	}
}
