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

//! Tests the Homa and XcmInterface module - cross-chain functionalities for the Homa module.

use crate::relaychain::kusama_test_net::*;
use crate::setup::*;
use frame_support::{assert_ok, weights::Weight};
use module_homa::UnlockChunk;
use module_xcm_interface::XcmInterfaceOperation;
use pallet_staking::StakingLedger;
use sp_runtime::MultiAddress;
use xcm_emulator::TestExt;

// Weight and fee cost is related to the XCM_WEIGHT passed in.
const XCM_WEIGHT: Weight = 20_000_000_000;
const XCM_FEE: Balance = 10_000_000_000;
const ACTUAL_XCM_FEE: Balance = 639_999_960;

fn get_xcm_weight() -> Vec<(XcmInterfaceOperation, Option<Weight>, Option<Balance>)> {
	vec![
		// Xcm weight = 400_000_000, fee = ACTUAL_XCM_FEE
		(XcmInterfaceOperation::XtokensTransfer, Some(XCM_WEIGHT), Some(XCM_FEE)),
		// Xcm weight = 14_000_000_000, fee = ACTUAL_XCM_FEE
		(
			XcmInterfaceOperation::HomaWithdrawUnbonded,
			Some(XCM_WEIGHT),
			Some(XCM_FEE),
		),
		// Xcm weight = 14_000_000_000, fee = ACTUAL_XCM_FEE
		(XcmInterfaceOperation::HomaBondExtra, Some(XCM_WEIGHT), Some(XCM_FEE)),
		// Xcm weight = 14_000_000_000, fee = ACTUAL_XCM_FEE
		(XcmInterfaceOperation::HomaUnbond, Some(XCM_WEIGHT), Some(XCM_FEE)),
	]
}

struct HomaParams {
	pub soft_bonded_cap_per_sub_account: Option<Balance>,
	pub estimated_reward_rate_per_era: Option<Rate>,
	pub commission_rate: Option<Rate>,
	pub fast_match_fee_rate: Option<Rate>,
}
impl Default for HomaParams {
	fn default() -> Self {
		HomaParams {
			soft_bonded_cap_per_sub_account: Some(1_000_000_000 * dollar(RELAY_CHAIN_CURRENCY)),
			estimated_reward_rate_per_era: None,
			commission_rate: None,
			fast_match_fee_rate: None,
		}
	}
}

// Helper function to setup config. Called within Karura Externalities.
fn configure_homa_and_xcm_interface() {
	// Configure Homa and XcmInterface
	assert_ok!(XcmInterface::update_xcm_dest_weight_and_fee(
		Origin::root(),
		get_xcm_weight()
	));
	let param = HomaParams::default();
	assert_ok!(Homa::update_homa_params(
		Origin::root(),
		param.soft_bonded_cap_per_sub_account,
		param.estimated_reward_rate_per_era,
		param.commission_rate,
		param.fast_match_fee_rate,
	));
}

#[test]
fn xcm_interface_transfer_staking_to_sub_account_works() {
	let homa_lite_sub_account: AccountId =
		hex_literal::hex!["d7b8926b326dd349355a9a7cca6606c1e0eb6fd2b506066b518c7155ff0d8297"].into();
	let mut parachain_account: AccountId = AccountId::new([0u8; 32]);
	Karura::execute_with(|| {
		parachain_account = ParachainAccount::get();
	});
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
			Box::new((Here, 2001 * dollar(RELAY_CHAIN_CURRENCY)).into()),
			0
		));

		assert_eq!(kusama_runtime::Balances::free_balance(&homa_lite_sub_account), 0);
		assert_eq!(
			kusama_runtime::Balances::free_balance(&parachain_account),
			2003 * dollar(RELAY_CHAIN_CURRENCY)
		);
	});

	Karura::execute_with(|| {
		assert_ok!(Tokens::set_balance(
			Origin::root(),
			MultiAddress::Id(AccountId::from(bob())),
			RELAY_CHAIN_CURRENCY,
			1_000_000 * dollar(RELAY_CHAIN_CURRENCY),
			0
		));

		configure_homa_and_xcm_interface();

		// Transfer fund via XCM by Mint
		assert_ok!(Homa::mint(Origin::signed(bob()), 1_000 * dollar(RELAY_CHAIN_CURRENCY)));
		assert_ok!(Homa::process_to_bond_pool());
	});

	KusamaNet::execute_with(|| {
		// 1000 dollars (minus fee) are transferred into the Kusama chain
		assert_eq!(
			kusama_runtime::Balances::free_balance(&homa_lite_sub_account),
			999_999_893_333_340
		);
		// XCM fee is paid by the parachain account.
		assert_eq!(
			kusama_runtime::Balances::free_balance(&parachain_account),
			1003 * dollar(RELAY_CHAIN_CURRENCY) - ACTUAL_XCM_FEE
		);
	});
}

#[test]
fn xcm_interface_withdraw_unbonded_from_sub_account_works() {
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
			1_001 * dollar(RELAY_CHAIN_CURRENCY)
		));

		assert_eq!(
			kusama_runtime::Balances::free_balance(&homa_lite_sub_account.clone()),
			1_001 * dollar(RELAY_CHAIN_CURRENCY)
		);

		// bond and unbond some fund for staking
		assert_ok!(kusama_runtime::Staking::bond(
			kusama_runtime::Origin::signed(homa_lite_sub_account.clone()),
			MultiAddress::Id(homa_lite_sub_account.clone()),
			1_000 * dollar(RELAY_CHAIN_CURRENCY),
			pallet_staking::RewardDestination::<AccountId>::Staked,
		));

		kusama_runtime::System::set_block_number(100);
		assert_ok!(kusama_runtime::Staking::unbond(
			kusama_runtime::Origin::signed(homa_lite_sub_account.clone()),
			1_000 * dollar(RELAY_CHAIN_CURRENCY)
		));

		// Kusama's unbonding period is 27 days = 100_800 blocks
		kusama_runtime::System::set_block_number(101_000);
		for _i in 0..29 {
			kusama_runtime::Staking::trigger_new_era(0, vec![]);
		}

		// Endowed from kusama_ext()
		assert_eq!(
			kusama_runtime::Balances::free_balance(&parachain_account.clone()),
			2 * dollar(RELAY_CHAIN_CURRENCY)
		);
		assert_eq!(
			kusama_runtime::Balances::free_balance(&homa_lite_sub_account.clone()),
			1_001 * dollar(RELAY_CHAIN_CURRENCY)
		);
	});

	Karura::execute_with(|| {
		assert_ok!(Tokens::set_balance(
			Origin::root(),
			MultiAddress::Id(AccountId::from(bob())),
			LIQUID_CURRENCY,
			1_000_000 * dollar(LIQUID_CURRENCY),
			0
		));

		configure_homa_and_xcm_interface();

		// Add an unlock chunk to the ledger
		assert_ok!(Homa::reset_ledgers(
			Origin::root(),
			vec![(
				0,
				Some(1_000 * dollar(RELAY_CHAIN_CURRENCY)),
				Some(vec![UnlockChunk {
					value: 1000 * dollar(RELAY_CHAIN_CURRENCY),
					era: 0
				},])
			),]
		));

		// Process the unlocking and withdraw unbonded.
		assert_ok!(Homa::process_scheduled_unbond(0));
	});

	KusamaNet::execute_with(|| {
		// Fund has been withdrew and transferred.
		assert_eq!(
			kusama_runtime::Balances::free_balance(&homa_lite_sub_account),
			dollar(RELAY_CHAIN_CURRENCY)
		);
		// Final parachain balance is: unbond_withdrew($1000) + initial_endowment($2) - xcm_fee
		assert_eq!(
			kusama_runtime::Balances::free_balance(&parachain_account.clone()),
			1002 * dollar(RELAY_CHAIN_CURRENCY) - ACTUAL_XCM_FEE
		);
	});
}

#[test]
fn xcm_interface_bond_extra_on_sub_account_works() {
	let homa_lite_sub_account: AccountId =
		hex_literal::hex!["d7b8926b326dd349355a9a7cca6606c1e0eb6fd2b506066b518c7155ff0d8297"].into();
	let mut parachain_account: AccountId = AccountId::new([0u8; 32]);
	Karura::execute_with(|| {
		parachain_account = ParachainAccount::get();
	});
	KusamaNet::execute_with(|| {
		assert_ok!(kusama_runtime::Balances::transfer(
			kusama_runtime::Origin::signed(ALICE.into()),
			MultiAddress::Id(homa_lite_sub_account.clone()),
			1_001 * dollar(RELAY_CHAIN_CURRENCY)
		));

		// Bond some money
		assert_ok!(kusama_runtime::Staking::bond(
			kusama_runtime::Origin::signed(homa_lite_sub_account.clone()),
			MultiAddress::Id(homa_lite_sub_account.clone()),
			500 * dollar(RELAY_CHAIN_CURRENCY),
			pallet_staking::RewardDestination::<AccountId>::Staked,
		));

		assert_eq!(
			kusama_runtime::Staking::ledger(&homa_lite_sub_account),
			Some(StakingLedger {
				stash: homa_lite_sub_account.clone(),
				total: 500 * dollar(RELAY_CHAIN_CURRENCY),
				active: 500 * dollar(RELAY_CHAIN_CURRENCY),
				unlocking: vec![],
				claimed_rewards: vec![],
			})
		);

		assert_eq!(
			kusama_runtime::Balances::free_balance(&homa_lite_sub_account),
			1001 * dollar(RELAY_CHAIN_CURRENCY)
		);
		assert_eq!(
			kusama_runtime::Balances::free_balance(&parachain_account),
			2 * dollar(RELAY_CHAIN_CURRENCY)
		);
	});

	Karura::execute_with(|| {
		assert_ok!(Tokens::set_balance(
			Origin::root(),
			MultiAddress::Id(AccountId::from(bob())),
			RELAY_CHAIN_CURRENCY,
			501 * dollar(RELAY_CHAIN_CURRENCY),
			0
		));

		configure_homa_and_xcm_interface();

		// Use Mint to bond more.
		assert_ok!(Homa::mint(Origin::signed(bob()), 500 * dollar(RELAY_CHAIN_CURRENCY)));
		assert_ok!(Homa::process_to_bond_pool());
	});

	KusamaNet::execute_with(|| {
		assert_eq!(
			kusama_runtime::Staking::ledger(&homa_lite_sub_account),
			Some(StakingLedger {
				stash: homa_lite_sub_account.clone(),
				total: 1000 * dollar(RELAY_CHAIN_CURRENCY) - XCM_FEE,
				active: 1000 * dollar(RELAY_CHAIN_CURRENCY) - XCM_FEE,
				unlocking: vec![],
				claimed_rewards: vec![],
			})
		);
		assert_eq!(
			kusama_runtime::Balances::free_balance(&homa_lite_sub_account),
			1001 * dollar(RELAY_CHAIN_CURRENCY)
		);
		// XCM fee is paid by the sovereign account.
		assert_eq!(
			kusama_runtime::Balances::free_balance(&parachain_account),
			2 * dollar(RELAY_CHAIN_CURRENCY) - ACTUAL_XCM_FEE
		);
	});
}

#[test]
fn xcm_interface_unbond_on_sub_account_works() {
	let homa_lite_sub_account: AccountId =
		hex_literal::hex!["d7b8926b326dd349355a9a7cca6606c1e0eb6fd2b506066b518c7155ff0d8297"].into();
	let mut parachain_account: AccountId = AccountId::new([0u8; 32]);
	Karura::execute_with(|| {
		parachain_account = ParachainAccount::get();
	});
	KusamaNet::execute_with(|| {
		assert_ok!(kusama_runtime::Balances::transfer(
			kusama_runtime::Origin::signed(ALICE.into()),
			MultiAddress::Id(homa_lite_sub_account.clone()),
			1_001 * dollar(RELAY_CHAIN_CURRENCY)
		));

		// Bond some tokens.
		assert_ok!(kusama_runtime::Staking::bond(
			kusama_runtime::Origin::signed(homa_lite_sub_account.clone()),
			MultiAddress::Id(homa_lite_sub_account.clone()),
			dollar(RELAY_CHAIN_CURRENCY),
			pallet_staking::RewardDestination::<AccountId>::Staked,
		));

		assert_eq!(
			kusama_runtime::Staking::ledger(&homa_lite_sub_account),
			Some(StakingLedger {
				stash: homa_lite_sub_account.clone(),
				total: dollar(RELAY_CHAIN_CURRENCY),
				active: dollar(RELAY_CHAIN_CURRENCY),
				unlocking: vec![],
				claimed_rewards: vec![],
			})
		);

		assert_eq!(
			kusama_runtime::Balances::free_balance(&homa_lite_sub_account),
			1_001 * dollar(RELAY_CHAIN_CURRENCY)
		);
		assert_eq!(
			kusama_runtime::Balances::free_balance(&parachain_account),
			2 * dollar(RELAY_CHAIN_CURRENCY)
		);
	});

	Karura::execute_with(|| {
		assert_ok!(Tokens::set_balance(
			Origin::root(),
			MultiAddress::Id(AccountId::from(bob())),
			RELAY_CHAIN_CURRENCY,
			1_001 * dollar(RELAY_CHAIN_CURRENCY),
			0
		));

		configure_homa_and_xcm_interface();

		// Bond more using Mint
		// Amount bonded = $1000 - XCM_FEE = 999_990_000_000_000
		assert_ok!(Homa::mint(Origin::signed(bob()), 1_000 * dollar(RELAY_CHAIN_CURRENCY),));
		// Update internal storage in Homa
		assert_ok!(Homa::bump_current_era(1));

		// Put in redeem request
		assert_ok!(Homa::request_redeem(
			Origin::signed(bob()),
			10_000 * dollar(LIQUID_CURRENCY),
			false,
		));
		// Process the redeem request and unbond funds on the relaychain.
		assert_ok!(Homa::process_redeem_requests(1));
	});

	KusamaNet::execute_with(|| {
		// Ensure the correct amount of fund is unbonded
		let ledger = kusama_runtime::Staking::ledger(&homa_lite_sub_account).expect("record should exist");
		assert_eq!(ledger.total, 1001 * dollar(RELAY_CHAIN_CURRENCY) - XCM_FEE);
		assert_eq!(ledger.active, dollar(RELAY_CHAIN_CURRENCY));

		assert_eq!(
			kusama_runtime::Balances::free_balance(&homa_lite_sub_account),
			1_001 * dollar(RELAY_CHAIN_CURRENCY)
		);

		// 2 x XCM fee is paid: for Mint and Redeem
		assert_eq!(
			kusama_runtime::Balances::free_balance(&parachain_account),
			2 * dollar(RELAY_CHAIN_CURRENCY) - ACTUAL_XCM_FEE * 2
		);
	});
}

// Test the entire process from Mint to Redeem.
#[test]
fn homa_mint_and_redeem_works() {
	let homa_lite_sub_account: AccountId =
		hex_literal::hex!["d7b8926b326dd349355a9a7cca6606c1e0eb6fd2b506066b518c7155ff0d8297"].into();
	let mut parachain_account: AccountId = AccountId::new([0u8; 32]);
	let bonding_duration = BondingDuration::get();

	Karura::execute_with(|| {
		parachain_account = ParachainAccount::get();
	});
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
			Box::new((Here, 2001 * dollar(RELAY_CHAIN_CURRENCY)).into()),
			0
		));

		// Transfer some KSM into the parachain.
		assert_ok!(kusama_runtime::Balances::transfer(
			kusama_runtime::Origin::signed(ALICE.into()),
			MultiAddress::Id(homa_lite_sub_account.clone()),
			dollar(RELAY_CHAIN_CURRENCY)
		));

		assert_ok!(kusama_runtime::Staking::bond(
			kusama_runtime::Origin::signed(homa_lite_sub_account.clone()),
			MultiAddress::Id(homa_lite_sub_account.clone()),
			dollar(RELAY_CHAIN_CURRENCY),
			pallet_staking::RewardDestination::<AccountId>::Staked,
		));
		assert_eq!(
			kusama_runtime::Balances::free_balance(&parachain_account),
			2003 * dollar(RELAY_CHAIN_CURRENCY)
		);
		assert_eq!(
			kusama_runtime::Balances::free_balance(&homa_lite_sub_account),
			dollar(RELAY_CHAIN_CURRENCY),
		);
	});

	Karura::execute_with(|| {
		assert_ok!(Tokens::set_balance(
			Origin::root(),
			MultiAddress::Id(AccountId::from(alice())),
			RELAY_CHAIN_CURRENCY,
			1_000 * dollar(RELAY_CHAIN_CURRENCY),
			0
		));
		assert_ok!(Tokens::set_balance(
			Origin::root(),
			MultiAddress::Id(AccountId::from(bob())),
			RELAY_CHAIN_CURRENCY,
			1_000 * dollar(RELAY_CHAIN_CURRENCY),
			0
		));

		configure_homa_and_xcm_interface();

		// Test mint works
		// Amount bonded = $1000 - XCM_FEE = 999_990_000_000_000
		assert_ok!(Homa::mint(
			Origin::signed(alice()),
			1_000 * dollar(RELAY_CHAIN_CURRENCY)
		));
		assert_ok!(Homa::mint(Origin::signed(bob()), 1_000 * dollar(RELAY_CHAIN_CURRENCY)));

		assert_eq!(Homa::get_total_bonded(), 0);
		assert_eq!(Homa::get_total_staking_currency(), 2_000 * dollar(RELAY_CHAIN_CURRENCY));

		// Synchronize with Relay chain via Xcm messages. Also update internal storage.
		assert_ok!(Homa::bump_current_era(1));

		assert_eq!(
			Tokens::free_balance(LIQUID_CURRENCY, &AccountId::from(alice())),
			10_000 * dollar(LIQUID_CURRENCY)
		);
		assert_eq!(
			Tokens::free_balance(LIQUID_CURRENCY, &AccountId::from(bob())),
			10_000 * dollar(LIQUID_CURRENCY)
		);
		assert_eq!(Tokens::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(alice())), 0);
		assert_eq!(Tokens::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(bob())), 0);

		assert_eq!(Homa::get_total_bonded(), 2_000 * dollar(RELAY_CHAIN_CURRENCY) - XCM_FEE);
		assert_eq!(
			Homa::get_total_staking_currency(),
			2_000 * dollar(RELAY_CHAIN_CURRENCY) - XCM_FEE
		);
	});

	KusamaNet::execute_with(|| {
		// Ensure the correct amount is bonded.
		let ledger = kusama_runtime::Staking::ledger(&homa_lite_sub_account).expect("record should exist");
		assert_eq!(ledger.total, 2001 * dollar(RELAY_CHAIN_CURRENCY) - XCM_FEE);
		assert_eq!(ledger.active, 2001 * dollar(RELAY_CHAIN_CURRENCY) - XCM_FEE);

		// 2 x XCM fee is paid: for Mint and Redeem
		assert_eq!(
			kusama_runtime::Balances::free_balance(&parachain_account),
			3 * dollar(RELAY_CHAIN_CURRENCY) - ACTUAL_XCM_FEE
		);
	});

	Karura::execute_with(|| {
		assert_ok!(Tokens::set_balance(
			Origin::root(),
			MultiAddress::Id(AccountId::from(alice())),
			RELAY_CHAIN_CURRENCY,
			0,
			0
		));
		assert_ok!(Tokens::set_balance(
			Origin::root(),
			MultiAddress::Id(AccountId::from(bob())),
			RELAY_CHAIN_CURRENCY,
			0,
			0
		));

		// Redeem the liquid currency.
		assert_ok!(Homa::request_redeem(
			Origin::signed(alice()),
			10_000 * dollar(LIQUID_CURRENCY),
			false,
		));
		assert_ok!(Homa::request_redeem(
			Origin::signed(bob()),
			10_000 * dollar(LIQUID_CURRENCY),
			false,
		));

		// Unbonds the tokens on the Relay chain.
		assert_ok!(Homa::bump_current_era(1));
		let unbonding_era = Homa::relay_chain_current_era() + bonding_duration;
		assert_eq!(unbonding_era, 30);

		assert_eq!(Homa::unbondings(&alice(), unbonding_era), 999_995_000_000_000);
		assert_eq!(Homa::unbondings(&bob(), unbonding_era), 999_995_000_000_000);

		assert_eq!(Homa::get_total_bonded(), 0);
		assert_eq!(Homa::get_total_staking_currency(), 0);
		assert_eq!(Tokens::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(alice())), 0);
		assert_eq!(Tokens::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(bob())), 0);
	});

	KusamaNet::execute_with(|| {
		// Some bonds are being unlocked via Xcm from the parachain.
		let ledger = kusama_runtime::Staking::ledger(&homa_lite_sub_account).expect("record should exist");
		assert_eq!(ledger.total, 2001 * dollar(RELAY_CHAIN_CURRENCY) - XCM_FEE);
		assert_eq!(ledger.active, dollar(RELAY_CHAIN_CURRENCY));

		// Fast forward the era until unlocking period ends.
		kusama_runtime::System::set_block_number(101_000);
		for _i in 0..29 {
			kusama_runtime::Staking::trigger_new_era(0, vec![]);
		}
	});

	Karura::execute_with(|| {
		assert_ok!(Tokens::set_balance(
			Origin::root(),
			MultiAddress::Id(AccountId::from(alice())),
			RELAY_CHAIN_CURRENCY,
			0,
			0
		));
		assert_ok!(Tokens::set_balance(
			Origin::root(),
			MultiAddress::Id(AccountId::from(bob())),
			RELAY_CHAIN_CURRENCY,
			0,
			0
		));

		// Wait for the chunk to unlock
		for _ in 0..bonding_duration + 1 {
			assert_ok!(Homa::bump_current_era(1));
		}

		// Claim the unlocked chunk
		assert_ok!(Homa::claim_redemption(Origin::signed(alice()), alice(),));
		assert_ok!(Homa::claim_redemption(Origin::signed(alice()), bob(),));

		// Redeem process is completed.
		assert_eq!(Homa::get_total_bonded(), 0);
		assert_eq!(Homa::get_total_staking_currency(), 0);
		assert_eq!(
			Tokens::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(alice())),
			999_995_000_000_000
		);
		assert_eq!(
			Tokens::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(bob())),
			999_995_000_000_000
		);
		assert_eq!(Tokens::free_balance(LIQUID_CURRENCY, &AccountId::from(alice())), 0);
		assert_eq!(Tokens::free_balance(LIQUID_CURRENCY, &AccountId::from(bob())), 0);
	});

	KusamaNet::execute_with(|| {
		// Unbonded chunks are withdrew.
		let ledger = kusama_runtime::Staking::ledger(&homa_lite_sub_account).expect("record should exist");
		assert_eq!(ledger.total, dollar(RELAY_CHAIN_CURRENCY));
		assert_eq!(ledger.active, dollar(RELAY_CHAIN_CURRENCY));
	});
}
