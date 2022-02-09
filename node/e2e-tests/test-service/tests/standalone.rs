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

use cumulus_primitives_core::ParaId;
use ecosystem_renvm_bridge::EcdsaSignature;
use hex_literal::hex;
use sc_transaction_pool_api::TransactionPool;
use sp_core::crypto::AccountId32;
use sp_keyring::Sr25519Keyring::*;
use sp_runtime::{traits::IdentifyAccount, MultiAddress, MultiSigner};
use test_service::SealMode;

#[substrate_test_utils::test]
#[ignore] // TODO: Wasm binary must be built for testing, polkadot/node/test/service/src/chain_spec.rs:117:40
async fn simple_balances_dev_test() {
	let mut builder = sc_cli::LoggerBuilder::new("");
	builder.with_colors(true);
	let _ = builder.init();

	let para_id = ParaId::from(2000);
	let tokio_handle = tokio::runtime::Handle::current();

	let node = test_service::TestNodeBuilder::new(para_id, tokio_handle.clone(), Alice)
		.with_seal_mode(SealMode::DevAuraSeal)
		.enable_collator()
		.build()
		.await;

	let bob = MultiSigner::from(Bob.public());
	let bob_account_id = bob.into_account();
	let amount = 1_000_000_000_000;

	type Balances = pallet_balances::Pallet<node_runtime::Runtime>;

	// the function with_state allows us to read state, pretty cool right? :D
	let old_balance = node.with_state(|| Balances::free_balance(bob_account_id.clone()));

	node.transfer(Alice, Bob, amount).await.unwrap();

	node.wait_for_blocks(1).await;

	// we can check the new state :D
	let new_balance = node.with_state(|| Balances::free_balance(bob_account_id));
	assert_eq!(old_balance + amount, new_balance);
}

#[substrate_test_utils::test]
#[ignore]
async fn transaction_pool_priority_order_test() {
	let mut builder = sc_cli::LoggerBuilder::new("");
	builder.with_colors(true);
	let _ = builder.init();

	let para_id = ParaId::from(2000);
	let tokio_handle = tokio::runtime::Handle::current();

	let node = test_service::TestNodeBuilder::new(para_id, tokio_handle.clone(), Alice)
		.with_seal_mode(SealMode::DevAuraSeal)
		.enable_collator()
		.build()
		.await;

	let bob = MultiSigner::from(Bob.public());
	let bob_account_id = bob.into_account();

	// send operational extrinsic
	let operational_tx_hash = node
		.submit_extrinsic(
			pallet_sudo::Call::sudo {
				call: Box::new(module_emergency_shutdown::Call::emergency_shutdown {}.into()),
			},
			Some(Alice),
		)
		.await
		.unwrap();

	// send normal extrinsic
	let normal_tx_hash = node
		.submit_extrinsic(
			pallet_balances::Call::transfer {
				dest: MultiAddress::from(bob_account_id.clone()),
				value: 80_000,
			},
			Some(Bob),
		)
		.await
		.unwrap();

	// send unsigned extrinsic
	let to: AccountId32 = hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into();
	let unsigned_tx_hash = node.submit_extrinsic(
		ecosystem_renvm_bridge::Call::mint {
			who: to,
			p_hash: hex!["67028f26328144de6ef80b8cd3b05e0cefb488762c340d1574c0542f752996cb"],
			amount: 93963,
			n_hash: hex!["f6a75cc370a2dda6dfc8d016529766bb6099d7fa0d787d9fe5d3a7e60c9ac2a0"],
			sig: EcdsaSignature::from_slice(&hex!["defda6eef01da2e2a90ce30ba73e90d32204ae84cae782b485f01d16b69061e0381a69cafed3deb6112af044c42ed0f7c73ee0eec7b533334d31a06db50fc40e1b"]),
		},
		None,
	).await.unwrap();

	assert_eq!(node.transaction_pool.ready().count(), 3);

	// Ensure tx priority order:
	// Inherent -> Operational tx -> Unsigned tx -> Signed normal tx
	let mut txs = node.transaction_pool.ready();
	let tx1 = txs.next().unwrap();
	let tx2 = txs.next().unwrap();
	let tx3 = txs.next().unwrap();

	assert_eq!(tx1.hash, operational_tx_hash);
	assert_eq!(tx2.hash, unsigned_tx_hash);
	assert_eq!(tx3.hash, normal_tx_hash);

	assert!(tx1.priority > tx2.priority);
	assert!(tx2.priority > tx3.priority);
}
