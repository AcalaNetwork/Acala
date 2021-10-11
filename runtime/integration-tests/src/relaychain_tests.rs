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

//! Tests RelayChain related things.
//! Currently only Karura XCM is tested.

#[cfg(feature = "with-karura-runtime")]
mod karura_tests {
	use crate::integration_tests::*;
	use crate::kusama_test_net::*;

	use frame_support::{assert_noop, assert_ok};

	use module_relaychain::RelayChainCallBuilder;
	use module_support::CallBuilder;
	use xcm::{latest::prelude::*, DoubleEncoded};
	use xcm_emulator::TestExt;

	type KusamaCallBuilder = RelayChainCallBuilder<Runtime, ParachainInfo>;

	#[test]
	/// Tests the staking_withdraw_unbonded call.
	/// Also tests utility_as_derivative call.
	fn relaychain_staking_withdraw_unbonded_works() {
		let homa_lite_sub_account: AccountId =
			hex_literal::hex!["d7b8926b326dd349355a9a7cca6606c1e0eb6fd2b506066b518c7155ff0d8297"].into();
		KusamaNet::execute_with(|| {
			kusama_runtime::Staking::trigger_new_era(0, vec![]);

			// Transfer some KSM into the parachain.
			assert_ok!(kusama_runtime::Balances::transfer(
				kusama_runtime::Origin::signed(ALICE.into()),
				MultiAddress::Id(homa_lite_sub_account.clone()),
				1_001_000_000_000_000
			));

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

			// Kusama's unbonding period is 7 days = 7 * 3600 / 6 = 100_800 blocks
			kusama_runtime::System::set_block_number(101_000);
			// Kusama: 6 hours per era. 7 days = 4 * 7 = 28 eras.
			for _i in 0..29 {
				kusama_runtime::Staking::trigger_new_era(0, vec![]);
			}

			assert_eq!(
				kusama_runtime::Balances::free_balance(&homa_lite_sub_account.clone()),
				1_001_000_000_000_000
			);

			// Transfer fails because liquidity is locked.
			assert_noop!(
				kusama_runtime::Balances::transfer(
					kusama_runtime::Origin::signed(homa_lite_sub_account.clone()),
					MultiAddress::Id(ALICE.into()),
					1_000_000_000_000_000
				),
				pallet_balances::Error::<kusama_runtime::Runtime>::LiquidityRestrictions
			);
			// assert_ok!(kusama_runtime::Staking::withdraw_unbonded(
			// 	kusama_runtime::Origin::signed(homa_lite_sub_account.clone()),
			// 	5
			// ));
		});

		Karura::execute_with(|| {
			// Call withdraw_unbonded as the homa-lite subaccount
			let xcm_message =
				KusamaCallBuilder::utility_as_derivative_call(KusamaCallBuilder::staking_withdraw_unbonded(5), 0);

			let msg = KusamaCallBuilder::finalize_call_into_xcm_message(
				xcm_message,
				XcmUnbondFee::get(),
				10_000_000_000,
				10_000_000_000,
			);

			// Withdraw unbonded
			assert_ok!(pallet_xcm::Pallet::<Runtime>::send_xcm(Here, Parent.into(), msg));
		});

		KusamaNet::execute_with(|| {
			assert_eq!(
				kusama_runtime::Balances::free_balance(&homa_lite_sub_account.clone()),
				1_001_000_000_000_000
			);

			// Transfer fails because liquidity is locked.
			assert_ok!(
				kusama_runtime::Balances::transfer(
					kusama_runtime::Origin::signed(homa_lite_sub_account.clone()),
					MultiAddress::Id(ALICE.into()),
					1_000_000_000_000_000
				) //kusama_runtime::Balances::Error::<Runtime>::LiquidityLocked,
			);
			assert_eq!(
				kusama_runtime::Balances::free_balance(&homa_lite_sub_account.clone()),
				1_000_000_000_000
			);
		});
	}

	#[test]
	/// Tests transfer_keep_alive call
	fn relaychain_transfer_keep_alive_works() {
		let mut parachain_account: AccountId = AccountId::default();
		Karura::execute_with(|| {
			parachain_account = ParachainAccount::get();
		});
		KusamaNet::execute_with(|| {
			assert_eq!(
				kusama_runtime::Balances::free_balance(AccountId::from(ALICE)),
				2_002_000_000_000_000
			);
			assert_eq!(
				kusama_runtime::Balances::free_balance(&parachain_account.clone()),
				2_000_000_000_000
			);
		});

		Karura::execute_with(|| {
			// Transfer all remaining, but leave enough fund to pay for the XCM transaction.
			let xcm_message = KusamaCallBuilder::balances_transfer_keep_alive(ALICE.into(), 1_990_000_000_000);

			let msg = KusamaCallBuilder::finalize_call_into_xcm_message(
				xcm_message,
				XcmUnbondFee::get(),
				10_000_000_000,
				10_000_000_000,
			);

			// Withdraw unbonded
			assert_ok!(pallet_xcm::Pallet::<Runtime>::send_xcm(Here, Parent.into(), msg));
		});

		KusamaNet::execute_with(|| {
			assert_eq!(
				kusama_runtime::Balances::free_balance(AccountId::from(ALICE)),
				2_003_990_000_000_000
			);
			// Only leftover XCM fee remains in the account
			assert_eq!(
				kusama_runtime::Balances::free_balance(&parachain_account.clone()),
				9_400_000_000
			);
		});
	}

	#[test]
	/// Tests the calls built by the call builder are encoded and decoded correctly
	fn relaychain_call_codec_works() {
		KusamaNet::execute_with(|| {
			let mut msg: DoubleEncoded<kusama_runtime::Call> =
				KusamaCallBuilder::staking_withdraw_unbonded(5).encode().into();
			assert_ok!(msg.ensure_decoded());
			let staking_call = msg.take_decoded().unwrap();
			assert_eq!(msg, vec![6, 3, 5, 0, 0, 0]);
			assert_eq!(call, Call::Staking(Call::withdraw_unbonded(5)));
			println!("Encoded:{:?} \nDecoded:{:?}", msg, call);

			let mut msg: DoubleEncoded<kusama_runtime::Call> =
				KusamaCallBuilder::utility_batch_call(vec![staking_call])
					.encode()
					.into();
			assert_ok!(msg.ensure_decoded());
			let call = msg.take_decoded().unwrap();
			assert_eq!(call, Call::Staking(Call::withdraw_unbonded(5)));
			println!("Encoded:{:?} \nDecoded:{:?}", msg, call);
		});
	}
}

// fn utility_batch_call(calls: Vec<Self::RelayChainCall>) -> Self::RelayChainCall;

// fn utility_as_derivative_call(call: Self::RelayChainCall, index: u16) -> Self::RelayChainCall;

// fn staking_withdraw_unbonded(num_slashing_spans: u32) -> Self::RelayChainCall;

// fn balances_transfer_keep_alive(to: Self::AccountId, amount: Self::Balance) ->
// Self::RelayChainCall;
