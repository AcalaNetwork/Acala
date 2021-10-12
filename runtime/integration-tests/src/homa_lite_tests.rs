// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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

#[cfg(any(feature = "with-mandala-runtime", feature = "with-karura-runtime"))]
mod common_tests {
	use crate::integration_tests::*;
	use frame_support::{assert_noop, assert_ok};
	use orml_traits::MultiCurrency;

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
				#[cfg(feature = "with-mandala-runtime")]
				let liquid_amount_1 = 49_974_999_500_250;
				#[cfg(feature = "with-karura-runtime")]
				let liquid_amount_1 = 4_997_499_000_500_000;

				assert_ok!(HomaLite::mint(Origin::signed(alice()), amount));
				assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &alice()), liquid_amount_1);
				System::assert_last_event(Event::HomaLite(module_homa_lite::Event::Minted(
					alice(),
					amount,
					liquid_amount_1,
				)));

				// Total issuance for liquid currnecy increased.
				let new_liquid_issuance = Currencies::total_issuance(LIQUID_CURRENCY);
				#[cfg(feature = "with-mandala-runtime")]
				assert_eq!(new_liquid_issuance, 10_049_974_999_500_250);
				#[cfg(feature = "with-karura-runtime")]
				assert_eq!(new_liquid_issuance, 1_004_997_499_000_500_000);

				// liquid = (amount - MintFee) * (new_liquid_issuance / new_staking_total) * (1 - MaxRewardPerEra)
				#[cfg(feature = "with-mandala-runtime")] // Mandala uses DOT, which has 10 d.p. accuracy.
				let liquid_amount_2 = 49_974_875_181_840;
				#[cfg(feature = "with-karura-runtime")] // Karura uses KSM, which has 12 d.p. accuracy.
				let liquid_amount_2 = 4_997_486_563_940_292;

				assert_ok!(HomaLite::mint(Origin::signed(alice()), amount));
				System::assert_last_event(Event::HomaLite(module_homa_lite::Event::Minted(
					alice(),
					amount,
					liquid_amount_2,
				)));

				#[cfg(feature = "with-mandala-runtime")]
				assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &alice()), 99_949_874_682_090);
				#[cfg(feature = "with-karura-runtime")]
				assert_eq!(
					Currencies::free_balance(LIQUID_CURRENCY, &alice()),
					9_994_985_564_440_292
				);
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
					2_000 * dollar(LIQUID_CURRENCY),
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
					2_000 * dollar(LIQUID_CURRENCY),
					Permill::from_percent(1)
				));

				// Minter pays no fee if minted via matching redeem requests, and no XCM transfer is needed.
				assert_ok!(HomaLite::mint(
					Origin::signed(AccountId::from(DAVE)),
					1_200 * dollar(RELAY_CHAIN_CURRENCY)
				));

				#[cfg(feature = "with-mandala-runtime")]
				{
					// Base withdraw fee = 0.014085
					// for ALICE:  staking_amount = +500 - redeem_fee = 500 - 7.0425 = 492.9575
					//             liquid_amount  = -5_000
					// for BOB:    staking_amount = +500 - redeem_fee - extra_fee(10%) = 500 - 7.0425  - 50 = 442.9575
					//             liquid_amount  = -5_000
					// for CHARlIE:staking_amount = +200 - redeem_fee - extra_fee(1%) = 200 - 2.817 - 2 = 195.183
					//             liquid_amount  = -5_000
					// for minter: staking_amount = 1200 -1_200 + redeem_fee * 3 + extra_fee =
					// 						      = 7.0425 + 7.0425 + 50 + 2.817 + 2 = 68.902
					//             liquid_amount  = +12_000
					assert_eq!(
						Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(ALICE)),
						4_929_575_000_000
					);
					assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(ALICE)), 0);
					assert_eq!(
						Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(BOB)),
						4_429_575_000_000
					);
					assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(BOB)), 0);
					assert_eq!(
						Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(CHARLIE)),
						1_951_830_000_000
					);
					assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(CHARLIE)), 0);
					assert_eq!(
						Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(DAVE)),
						689_020_000_000
					);
					assert_eq!(
						Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(DAVE)),
						12_000 * dollar(LIQUID_CURRENCY)
					);
				}
				#[cfg(feature = "with-karura-runtime")]
				{
					// Base redeem fee: 0.0035
					// for ALICE:  staking_amount = +500 - redeem_fee = 500 - 1.75 = 498.25
					//             liquid_amount  = -5_000
					// for BOB:    staking_amount = +500 - redeem_fee - extra_fee(10%) = 500 - 1.75 - 50 = 448.25
					//             liquid_amount  = -5_000
					// for CHARlIE:staking_amount = +200 - redeem_fee - extra_fee(1%) = 200 - 0.7 - 2 = 197.3
					//             liquid_amount  = -5_000
					// for minter: staking_amount = 1200 -1_200 + redeem_fee + extra_fee =
					//                            = 1.75 + 1.75 + 50 + 0.7 + 2 = 56.2
					//             liquid_amount  = +12_000
					assert_eq!(
						Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(ALICE)),
						498_250_000_000_000
					);
					assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(ALICE)), 0);
					assert_eq!(
						Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(BOB)),
						448_250_000_000_000
					);
					assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(BOB)), 0);
					assert_eq!(
						Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(CHARLIE)),
						197_300_000_000_000
					);
					assert_eq!(Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(CHARLIE)), 0);
					assert_eq!(
						Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(DAVE)),
						56_200_000_000_000
					);
					assert_eq!(
						Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(DAVE)),
						12_000 * dollar(LIQUID_CURRENCY)
					);
				}
			});
	}
}

#[cfg(feature = "with-karura-runtime")]
mod karura_only_tests {
	use crate::integration_tests::*;
	use crate::kusama_test_net::*;

	use frame_support::{assert_ok, traits::Hooks};
	use orml_traits::MultiCurrency;
	use sp_runtime::{traits::BlockNumberProvider, MultiAddress};

	use xcm::{latest::prelude::*, VersionedMultiAssets, VersionedMultiLocation};
	use xcm_emulator::TestExt;

	#[test]
	fn homa_lite_xcm_transfer() {
		let homa_lite_sub_account: AccountId =
			hex_literal::hex!["d7b8926b326dd349355a9a7cca6606c1e0eb6fd2b506066b518c7155ff0d8297"].into();
		KusamaNet::execute_with(|| {
			// Transfer some KSM into the parachain.
			assert_ok!(kusama_runtime::XcmPallet::reserve_transfer_assets(
				kusama_runtime::Origin::signed(ALICE.into()),
				Box::new(VersionedMultiLocation::V1(X1(Parachain(2000)).into())),
				Box::new(VersionedMultiLocation::V1(
					X1(Junction::AccountId32 {
						id: alice().into(),
						network: NetworkId::Any
					})
					.into()
				)),
				Box::new(VersionedMultiAssets::V1((Here, 2001 * dollar(KSM)).into())),
				0,
				600_000_000
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
			assert_eq!(Tokens::free_balance(RELAY_CHAIN_CURRENCY, &alice()), 999_952_000_001);
		});

		KusamaNet::execute_with(|| {
			// Check of 2000 dollars (minus some fee) are transferred into the Kusama chain.
			assert_eq!(
				kusama_runtime::Balances::free_balance(&homa_lite_sub_account),
				1_999_946_666_669_999
			);
		});
	}

	#[test]
	fn homa_lite_xcm_unbonding_works() {
		let homa_lite_sub_account: AccountId =
			hex_literal::hex!["d7b8926b326dd349355a9a7cca6606c1e0eb6fd2b506066b518c7155ff0d8297"].into();
		let mut parachain_account: AccountId = AccountId::default();
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
				RelayChainBlockNumberProvider::<Runtime>::current_block_number(),
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
				1_001_999_400_000_000
			);
		});
	}
}
