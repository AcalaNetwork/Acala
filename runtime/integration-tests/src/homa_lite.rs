// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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

//! Tests the Homa-lite module, and its cross-chain functionalities.

use crate::setup::*;
use frame_support::{assert_noop, assert_ok, traits::Hooks};
use module_support::ExchangeRateProvider;
use orml_traits::{MultiCurrency, MultiReservableCurrency};

#[test]
fn homa_lite_mint_works() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), RELAY_CHAIN_CURRENCY, 5_000 * dollar(RELAY_CHAIN_CURRENCY)),
			(bob(), RELAY_CHAIN_CURRENCY, 5_000 * dollar(RELAY_CHAIN_CURRENCY)),
			(bob(), LIQUID_CURRENCY, 1_000_000 * dollar(LIQUID_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			let amount = 1000 * dollar(RELAY_CHAIN_CURRENCY);

			assert_noop!(
				HomaLite::mint(Origin::signed(alice()), amount),
				module_homa_lite::Error::<Runtime>::ExceededStakingCurrencyMintCap
			);

			// Set the total staking amount
			let liquid_issuance = Currencies::total_issuance(LIQUID_CURRENCY);
			assert_eq!(liquid_issuance, 1_000_000 * dollar(LIQUID_CURRENCY));

			let staking_total = liquid_issuance / 5;

			// Set the exchange rate to 1(S) : 5(L)
			assert_ok!(HomaLite::set_total_staking_currency(Origin::root(), staking_total));

			assert_ok!(HomaLite::set_minting_cap(Origin::root(), 10 * staking_total));

			// Exchange rate set to 1(Staking) : 5(Liquid) ratio
			// liquid = (amount - MintFee) * exchange_rate * (1 - MaxRewardPerEra)
			#[cfg(feature = "with-karura-runtime")]
			let liquid_amount_1 = 4_997_499_000_500_000;
			#[cfg(feature = "with-mandala-runtime")]
			let liquid_amount_1 = 49_974_990_005_000;
			#[cfg(feature = "with-acala-runtime")]
			let liquid_amount_1 = 49_974_990_005_000;

			assert_ok!(HomaLite::mint(Origin::signed(alice()), amount));
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &alice()), liquid_amount_1);
			System::assert_last_event(Event::HomaLite(module_homa_lite::Event::Minted {
				who: alice(),
				amount_staked: amount,
				amount_minted: liquid_amount_1,
			}));

			// Total issuance for liquid currnecy increased.
			let new_liquid_issuance = Currencies::total_issuance(LIQUID_CURRENCY);
			#[cfg(feature = "with-karura-runtime")]
			assert_eq!(new_liquid_issuance, 1_004_997_499_000_500_000);
			#[cfg(feature = "with-mandala-runtime")]
			assert_eq!(new_liquid_issuance, 10_049_974_990_005_000);
			#[cfg(feature = "with-acala-runtime")]
			assert_eq!(new_liquid_issuance, 10_049_974_990_005_000);

			// liquid = (amount - MintFee) * (new_liquid_issuance / new_staking_total) * (1 - MaxRewardPerEra)
			#[cfg(feature = "with-karura-runtime")] // Karura uses KSM, which has 12 d.p. accuracy.
			let liquid_amount_2 = 4_997_486_563_940_292;
			#[cfg(feature = "with-mandala-runtime")] // Mandala uses DOT, which has 10 d.p. accuracy.
			let liquid_amount_2 = 49_974_865_639_397;
			#[cfg(feature = "with-acala-runtime")] // Acala uses DOT, which has 10 d.p. accuracy.
			let liquid_amount_2 = 49_974_865_639_397;

			assert_ok!(HomaLite::mint(Origin::signed(alice()), amount));
			System::assert_last_event(Event::HomaLite(module_homa_lite::Event::Minted {
				who: alice(),
				amount_staked: amount,
				amount_minted: liquid_amount_2,
			}));
			#[cfg(feature = "with-karura-runtime")]
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY, &alice()),
				9_994_985_564_440_292
			);
			#[cfg(feature = "with-mandala-runtime")]
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &alice()), 99_949_855_644_397);
			#[cfg(feature = "with-acala-runtime")]
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &alice()), 99_949_855_644_397);
		});
}

#[test]
fn homa_lite_mint_can_match_redeem_requests() {
	ExtBuilder::default()
		.balances(vec![
			(AccountId::from(ALICE), LIQUID_CURRENCY, 5_000 * dollar(LIQUID_CURRENCY)),
			(AccountId::from(BOB), LIQUID_CURRENCY, 5_000 * dollar(LIQUID_CURRENCY)),
			(
				AccountId::from(CHARLIE),
				LIQUID_CURRENCY,
				5_000 * dollar(LIQUID_CURRENCY),
			),
			(
				AccountId::from(DAVE),
				RELAY_CHAIN_CURRENCY,
				1_200 * dollar(RELAY_CHAIN_CURRENCY),
			),
		])
		.build()
		.execute_with(|| {
			// Default exchange rate is 1S : 10L
			assert_ok!(HomaLite::set_minting_cap(
				Origin::root(),
				20_000 * dollar(RELAY_CHAIN_CURRENCY)
			));

			// insert redeem requests
			assert_ok!(HomaLite::request_redeem(
				Origin::signed(AccountId::from(ALICE)),
				5_000 * dollar(LIQUID_CURRENCY),
				Permill::zero()
			));
			assert_ok!(HomaLite::request_redeem(
				Origin::signed(AccountId::from(BOB)),
				5_000 * dollar(LIQUID_CURRENCY),
				Permill::from_percent(10)
			));
			assert_ok!(HomaLite::request_redeem(
				Origin::signed(AccountId::from(CHARLIE)),
				5_000 * dollar(LIQUID_CURRENCY),
				Permill::from_percent(1)
			));

			// Minter pays no fee if minted via matching redeem requests, since no XCM transfer is needed.
			assert_ok!(HomaLite::mint_for_requests(
				Origin::signed(AccountId::from(DAVE)),
				1_200 * dollar(RELAY_CHAIN_CURRENCY),
				vec![AccountId::from(ALICE), AccountId::from(BOB)]
			));

			#[cfg(feature = "with-mandala-runtime")]
			{
				// Base withdraw fee = 0.014085
				// for ALICE:  liquid_amount  = +5000 - 4929.575 (redeem) - 70.425(fee) = 0
				//             staking_amount = +492.9575
				//
				// for BOB:    liquid_amount  = +5000 - 4929.575 (redeem) - 70.425(fee) = 0
				// 			   staking_amount = -492.9575 - extra_fee(10%)
				//                            = -492.9575 - 49.29575 = +443.66175
				//
				// for CHARlIE:liquid_amount  = +5000 - 2140.85 (redeem) - 70.425(fee) = 2788.725
				//             staking_amount = +214.085 - extra_fee(1%)
				//   						  = +214.085 - 2.14085 = +211.94415
				//
				// for minter: liquid_amount  = +12_000
				//			   staking_amount = 1200(initial) - 1_200(mint) + extra_fee =
				// 						      = 49.29575 + 2.14085 = 51.4366
				assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(ALICE)), 0);
				assert_eq!(
					Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(ALICE)),
					4_929_575_000_000
				);

				assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(BOB)), 0);
				assert_eq!(
					Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(BOB)),
					4_436_617_500_000
				);

				assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(CHARLIE)), 0);
				assert_eq!(
					Currencies::reserved_balance(LIQUID_CURRENCY, &AccountId::from(CHARLIE)),
					27_887_250_000_000
				);
				assert_eq!(
					Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(CHARLIE)),
					2_119_441_500_000
				);

				assert_eq!(
					Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(DAVE)),
					12_000 * dollar(LIQUID_CURRENCY)
				);
				assert_eq!(
					Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(DAVE)),
					514_366_000_000
				);
			}
			#[cfg(feature = "with-karura-runtime")]
			{
				// Base withdraw fee = 0.0035
				// for ALICE:  liquid_amount  = +5000 - 4982.5 (redeem) - 17.5(fee) = 0
				//             staking_amount = +498.25
				//
				// for BOB:    liquid_amount  = +5000 - 4982.5 (redeem) - 17.5(fee) = 0
				// 			   staking_amount = +498.25 - extra_fee(10%)
				//                            = +498.25 - 49.825 = -448.425
				//
				// for CHARlIE:liquid_amount  = +5000 -2035 (redeem) - 17.5(fee) = 2947.5
				//             staking_amount = +203.5 - extra_fee(1%)
				//   						  = +203.5 s- 2.035 = +201.465
				//
				// for minter: liquid_amount  = +12_000
				//             staking_amount = 1200(initial) -1_200(mint) + extra_fee =
				// 						      = 49.825 + 2.035 = 51.86
				assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(ALICE)), 0);
				assert_eq!(
					Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(ALICE)),
					498_250_000_000_000
				);

				assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(BOB)), 0);
				assert_eq!(
					Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(BOB)),
					448_425_000_000_000
				);

				assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(CHARLIE)), 0);
				assert_eq!(
					Currencies::reserved_balance(LIQUID_CURRENCY, &AccountId::from(CHARLIE)),
					2_947_500_000_000_000
				);
				assert_eq!(
					Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(CHARLIE)),
					201_465_000_000_000
				);

				assert_eq!(
					Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(DAVE)),
					12_000 * dollar(LIQUID_CURRENCY)
				);
				assert_eq!(
					Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(DAVE)),
					51_860_000_000_000
				);
			}
		});
}

#[test]
fn homa_lite_mint_and_redeem() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), RELAY_CHAIN_CURRENCY, 200 * dollar(RELAY_CHAIN_CURRENCY)),
			(bob(), RELAY_CHAIN_CURRENCY, 100 * dollar(RELAY_CHAIN_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			let rate1 = DefaultExchangeRate::get();
			assert_eq!(rate1, HomaLite::get_exchange_rate());

			assert_ok!(HomaLite::set_minting_cap(
				Origin::root(),
				300 * dollar(RELAY_CHAIN_CURRENCY)
			));

			assert_ok!(HomaLite::mint(
				Origin::signed(alice()),
				100 * dollar(RELAY_CHAIN_CURRENCY)
			));

			let rate2 = HomaLite::get_exchange_rate();
			assert!(rate1 < rate2);

			assert_ok!(HomaLite::adjust_total_staking_currency(
				Origin::root(),
				10i128 * dollar(RELAY_CHAIN_CURRENCY) as i128
			));

			let rate3 = HomaLite::get_exchange_rate();
			assert!(rate2 < rate3);
			assert!(Ratio::saturating_from_rational(110, 1000) < rate3);

			assert_ok!(HomaLite::mint(
				Origin::signed(bob()),
				100 * dollar(RELAY_CHAIN_CURRENCY)
			));

			let rate4 = HomaLite::get_exchange_rate();
			assert!(rate3 < rate4);

			assert_ok!(HomaLite::request_redeem(
				Origin::signed(bob()),
				100 * dollar(RELAY_CHAIN_CURRENCY),
				Permill::from_percent(0)
			));

			let rate5 = HomaLite::get_exchange_rate();
			assert!(rate4 < rate5);

			assert_ok!(HomaLite::mint(
				Origin::signed(alice()),
				100 * dollar(RELAY_CHAIN_CURRENCY)
			));

			let rate6 = HomaLite::get_exchange_rate();
			assert!(rate5 < rate6);
		});
}

#[test]
fn liquid_value_goes_up_periodically() {
	ExtBuilder::default()
		.balances(vec![(alice(), LIQUID_CURRENCY, 10_000_000 * dollar(LIQUID_CURRENCY))])
		.build()
		.execute_with(|| {
			let one_day = OneDay::get();
			assert_ok!(HomaLite::set_total_staking_currency(
				Origin::root(),
				1_000_000 * dollar(RELAY_CHAIN_CURRENCY)
			));
			assert_ok!(HomaLite::set_staking_interest_rate_per_update(
				Origin::root(),
				Permill::from_rational(383u32, 1_000_000u32)
			));

			let rate1 = HomaLite::get_exchange_rate();

			HomaLite::on_initialize(0);
			// Inflate by 1.000383 every 1 day (14400 blocks)
			// 1_000_000 * 1.000383 = 1_000_383
			assert_eq!(
				HomaLite::total_staking_currency(),
				1_000_383 * dollar(RELAY_CHAIN_CURRENCY)
			);
			let rate2 = HomaLite::get_exchange_rate();
			assert!(rate2 > rate1);

			for i in 1..one_day * 2 + 1 {
				HomaLite::on_initialize(i);
			}
			// Karura is 12 sec block time
			// 1_000_383 * 1.000383 * 1.000383 = 1001149.440123181887
			#[cfg(feature = "with-karura-runtime")]
			assert_eq!(HomaLite::total_staking_currency(), 1_001_149_440_123_181_887);

			#[cfg(any(feature = "with-mandala-runtime", feature = "with-acala-runtime"))]
			assert_eq!(HomaLite::total_staking_currency(), 10_011_494_401_231_819);

			let rate3 = HomaLite::get_exchange_rate();
			assert!(rate3 > rate2);

			for i in one_day * 2 + 1..one_day * 4 + 1 {
				HomaLite::on_initialize(i);
			}
			// 1001149.440123181887 * 1.000383 * 1.000383 = 1001916.46745192646655
			#[cfg(feature = "with-karura-runtime")]
			assert_eq!(HomaLite::total_staking_currency(), 1_001_916_467_451_926_467);

			#[cfg(any(feature = "with-mandala-runtime", feature = "with-acala-runtime"))]
			assert_eq!(HomaLite::total_staking_currency(), 10_019_164_674_519_265);

			let rate4 = HomaLite::get_exchange_rate();
			assert!(rate4 > rate3);
		});
}

#[test]
fn cannot_mint_below_minimum_threshold() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), RELAY_CHAIN_CURRENCY, 10_000_000 * dollar(RELAY_CHAIN_CURRENCY)),
			(bob(), LIQUID_CURRENCY, 10_000_000 * dollar(LIQUID_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			assert_ok!(HomaLite::set_minting_cap(
				Origin::root(),
				1_000_000_000_000 * dollar(RELAY_CHAIN_CURRENCY)
			));

			// sets the staking total so the exchange rate is 1S : 10L
			assert_ok!(HomaLite::set_total_staking_currency(
				Origin::root(),
				1_000_000 * dollar(RELAY_CHAIN_CURRENCY)
			));

			#[cfg(feature = "with-karura-runtime")]
			{
				// Minimum mint threshold + mint fee
				let threshold = 50 * cent(RELAY_CHAIN_CURRENCY) + 20 * millicent(RELAY_CHAIN_CURRENCY);
				assert_noop!(
					HomaLite::mint(Origin::signed(alice()), threshold),
					module_homa_lite::Error::<Runtime>::AmountBelowMinimumThreshold
				);

				assert_ok!(HomaLite::mint(Origin::signed(alice()), threshold + 1));
				assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &alice()), 4_997_500_000_010);
			}

			#[cfg(any(feature = "with-mandala-runtime", feature = "with-acala-runtime"))]
			{
				// // Minimum mint threshold + mint fee
				let threshold = 5 * dollar(RELAY_CHAIN_CURRENCY) + 20 * millicent(RELAY_CHAIN_CURRENCY);
				assert_noop!(
					HomaLite::mint(Origin::signed(alice()), threshold),
					module_homa_lite::Error::<Runtime>::AmountBelowMinimumThreshold
				);

				assert_ok!(HomaLite::mint(Origin::signed(alice()), threshold + 1));
				assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &alice()), 499_750_000_010);
			}
		});
}

#[test]
fn cannot_request_redeem_below_minimum_threshold() {
	ExtBuilder::default()
		.balances(vec![(alice(), LIQUID_CURRENCY, 10_000_000 * dollar(LIQUID_CURRENCY))])
		.build()
		.execute_with(|| {
			assert_ok!(HomaLite::set_total_staking_currency(
				Origin::root(),
				1_000_000 * dollar(RELAY_CHAIN_CURRENCY)
			));

			#[cfg(feature = "with-karura-runtime")]
			{
				// Redeem threshold is 5 * dollar(LIQUID_CURRENCY)
				assert_noop!(
					HomaLite::request_redeem(
						Origin::signed(alice()),
						5 * dollar(RELAY_CHAIN_CURRENCY),
						Permill::zero()
					),
					module_homa_lite::Error::<Runtime>::AmountBelowMinimumThreshold
				);

				assert_ok!(HomaLite::request_redeem(
					Origin::signed(alice()),
					5 * dollar(RELAY_CHAIN_CURRENCY) + 1,
					Permill::zero()
				));

				assert_eq!(
					HomaLite::redeem_requests(alice()),
					Some((4_982_500_000_001, Permill::zero()))
				);
			}

			#[cfg(any(feature = "with-mandala-runtime", feature = "with-acala-runtime"))]
			{
				// Redeem threshold is 50 * dollar(LIQUID_CURRENCY)
				assert_noop!(
					HomaLite::request_redeem(
						Origin::signed(alice()),
						50 * dollar(RELAY_CHAIN_CURRENCY),
						Permill::zero()
					),
					module_homa_lite::Error::<Runtime>::AmountBelowMinimumThreshold
				);

				assert_ok!(HomaLite::request_redeem(
					Origin::signed(alice()),
					50 * dollar(RELAY_CHAIN_CURRENCY) + 1,
					Permill::zero()
				));

				assert_eq!(
					HomaLite::redeem_requests(alice()),
					Some((492_957_500_001, Permill::zero()))
				);
			}
		});
}

#[cfg(feature = "with-karura-runtime")]
mod karura_only_tests {
	use crate::relaychain::kusama_test_net::*;
	use crate::setup::*;

	use frame_support::{assert_ok, traits::Hooks};
	use orml_traits::MultiCurrency;
	use sp_runtime::{traits::BlockNumberProvider, MultiAddress};

	use xcm_emulator::TestExt;

	#[test]
	fn homa_lite_xcm_transfer() {
		let homa_lite_sub_account: AccountId =
			hex_literal::hex!["d7b8926b326dd349355a9a7cca6606c1e0eb6fd2b506066b518c7155ff0d8297"].into();
		KusamaNet::execute_with(|| {
			// Transfer some KSM into the parachain.
			assert_ok!(kusama_runtime::XcmPallet::reserve_transfer_assets(
				kusama_runtime::Origin::signed(ALICE.into()),
				Box::new(Parachain(2000).into().into()),
				Box::new(
					Junction::AccountId32 {
						id: alice().into(),
						network: NetworkId::Any
					}
					.into()
					.into()
				),
				Box::new((Here, 2001 * dollar(KSM)).into()),
				0
			));

			// This account starts off with no fund.
			assert_eq!(kusama_runtime::Balances::free_balance(&homa_lite_sub_account), 0);
		});

		Karura::execute_with(|| {
			assert_ok!(Tokens::set_balance(
				Origin::root(),
				MultiAddress::Id(AccountId::from(bob())),
				LIQUID_CURRENCY,
				1_000_000 * dollar(LIQUID_CURRENCY),
				0
			));

			let amount = 1000 * dollar(RELAY_CHAIN_CURRENCY);

			// Set the total staking amount
			let liquid_issuance = Currencies::total_issuance(LIQUID_CURRENCY);
			assert_eq!(liquid_issuance, 1_000_000 * dollar(LIQUID_CURRENCY));

			let staking_total = 200_000 * dollar(LIQUID_CURRENCY);

			// Set the exchange rate to 1(S) : 5(L)
			assert_ok!(HomaLite::set_total_staking_currency(Origin::root(), staking_total));
			assert_ok!(HomaLite::set_xcm_dest_weight(Origin::root(), 1_000_000_000_000));

			assert_ok!(HomaLite::set_minting_cap(Origin::root(), 10 * staking_total));

			// Perform 2 mint actions, each 1000 dollars.
			assert_ok!(HomaLite::mint(Origin::signed(alice()), amount));
			assert_ok!(HomaLite::mint(Origin::signed(alice()), amount));

			// Most balances transferred into Kusama. Some extra fee is deducted as gas
			assert_eq!(Tokens::free_balance(RELAY_CHAIN_CURRENCY, &alice()), 999_872_000_001);
		});

		KusamaNet::execute_with(|| {
			// Check of 2000 dollars (minus some fee) are transferred into the Kusama chain.
			assert_eq!(
				kusama_runtime::Balances::free_balance(&homa_lite_sub_account),
				1_999_999_786_666_679
			);
		});
	}

	#[test]
	fn homa_lite_xcm_unbonding_works() {
		let homa_lite_sub_account: AccountId =
			hex_literal::hex!["d7b8926b326dd349355a9a7cca6606c1e0eb6fd2b506066b518c7155ff0d8297"].into();
		let mut parachain_account: AccountId = AccountId::new([0u8; 32]);
		Karura::execute_with(|| {
			parachain_account = ParachainAccount::get();
		});
		KusamaNet::execute_with(|| {
			kusama_runtime::Staking::trigger_new_era(0, vec![]);

			// Transfer some KSM into the parachain.
			assert_ok!(kusama_runtime::Balances::transfer(
				kusama_runtime::Origin::signed(ALICE.into()),
				MultiAddress::Id(homa_lite_sub_account.clone()),
				1_001_000_000_000_000
			));

			assert_eq!(
				kusama_runtime::Balances::free_balance(&homa_lite_sub_account.clone()),
				1_001_000_000_000_000
			);

			// bond and unbond some fund for staking
			assert_ok!(kusama_runtime::Staking::bond(
				kusama_runtime::Origin::signed(homa_lite_sub_account.clone()),
				MultiAddress::Id(homa_lite_sub_account.clone()),
				1_000_000_000_000_000,
				pallet_staking::RewardDestination::<AccountId>::Staked,
			));

			kusama_runtime::System::set_block_number(100);
			assert_ok!(kusama_runtime::Staking::unbond(
				kusama_runtime::Origin::signed(homa_lite_sub_account.clone()),
				1_000_000_000_000_000
			));

			// Kusama's unbonding period is 27 days = 100_800 blocks
			kusama_runtime::System::set_block_number(101_000);
			for _i in 0..29 {
				kusama_runtime::Staking::trigger_new_era(0, vec![]);
			}

			// Endowed from kusama_ext()
			assert_eq!(
				kusama_runtime::Balances::free_balance(&parachain_account.clone()),
				2_000_000_000_000
			);

			// Uncomment this to test if withdraw_unbonded and transfer_keep_alive
			// work without XCM. Used to isolate error when the test fails.
			// assert_ok!(kusama_runtime::Staking::withdraw_unbonded(
			// 	kusama_runtime::Origin::signed(homa_lite_sub_account.clone()),
			// 	5
			// ));
			// assert_ok!(kusama_runtime::Balances::transfer_keep_alive(
			// 	kusama_runtime::Origin::signed(homa_lite_sub_account.clone()),
			// 	MultiAddress::Id(ParachainAccount::get()),
			// 	1_000_000_000_000_000
			// ));
			// assert_eq!(kusama_runtime::Balances::free_balance(&ParachainAccount::get()),
			// 1_001_000_000_000_000);
		});

		Karura::execute_with(|| {
			assert_ok!(Tokens::set_balance(
				Origin::root(),
				MultiAddress::Id(AccountId::from(bob())),
				LIQUID_CURRENCY,
				1_000_000 * dollar(LIQUID_CURRENCY),
				0
			));

			// Weight is around 5_775_663_000
			assert_ok!(HomaLite::set_xcm_dest_weight(Origin::root(), 10_000_000_000));

			assert_ok!(HomaLite::schedule_unbond(
				Origin::root(),
				1000 * dollar(RELAY_CHAIN_CURRENCY),
				100_900
			));
			set_relaychain_block_number(101_000);
			run_to_block(5);
			assert_eq!(
				RelaychainBlockNumberProvider::<Runtime>::current_block_number(),
				101_000
			);
			HomaLite::on_idle(5, 1_000_000_000);
			assert_eq!(HomaLite::scheduled_unbond(), vec![]);
			assert_eq!(
				HomaLite::available_staking_balance(),
				1000 * dollar(RELAY_CHAIN_CURRENCY)
			);
		});

		KusamaNet::execute_with(|| {
			assert_eq!(
				kusama_runtime::Balances::free_balance(&homa_lite_sub_account),
				1_000_000_000_000
			);
			// Final parachain balance is: unbond_withdrew($1000) + initial_endowment($2) - xcm_fee
			assert_eq!(
				kusama_runtime::Balances::free_balance(&parachain_account.clone()),
				1_001_999_626_666_690
			);
		});
	}
}
