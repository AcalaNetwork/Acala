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
use node_primitives::AccountId;
use sp_keyring::Sr25519Keyring;
use sp_runtime::{traits::IdentifyAccount, MultiAddress, MultiSigner};
use test_service::{initial_head_data, run_relay_chain_validator_node, Keyring::*};

#[substrate_test_utils::test]
#[ignore]
async fn test_full_node_catching_up() {
	let mut builder = sc_cli::LoggerBuilder::new("");
	builder.with_colors(true);
	let _ = builder.init();

	let para_id = ParaId::from(2000);

	let tokio_handle = tokio::runtime::Handle::current();

	// start alice
	let alice = run_relay_chain_validator_node(tokio_handle.clone(), Alice, || {}, Vec::new());

	// start bob
	let bob = run_relay_chain_validator_node(tokio_handle.clone(), Bob, || {}, vec![alice.addr.clone()]);

	// register parachain
	alice
		.register_parachain(
			para_id,
			node_runtime::WASM_BINARY
				.expect("You need to build the WASM binary to run this test!")
				.to_vec(),
			initial_head_data(),
		)
		.await
		.unwrap();

	// run cumulus charlie (a parachain collator)
	let para_collator = test_service::TestNodeBuilder::new(para_id, tokio_handle.clone(), Alice)
		.enable_collator()
		.connect_to_relay_chain_nodes(vec![&alice, &bob])
		.build()
		.await;
	para_collator.wait_for_blocks(5).await;

	// run cumulus dave (a parachain full node) and wait for it to sync some blocks
	let para_full = test_service::TestNodeBuilder::new(para_id, tokio_handle, Dave)
		.connect_to_parachain_node(&para_collator)
		.connect_to_relay_chain_nodes(vec![&alice, &bob])
		.build()
		.await;
	para_full.wait_for_blocks(7).await;
}

#[substrate_test_utils::test]
#[ignore]
async fn simple_balances_test() {
	let mut builder = sc_cli::LoggerBuilder::new("");
	builder.with_colors(true);
	let _ = builder.init();

	let para_id = ParaId::from(2000);

	let tokio_handle = tokio::runtime::Handle::current();

	// start alice
	let alice = run_relay_chain_validator_node(tokio_handle.clone(), Alice, || {}, Vec::new());

	// start bob
	let bob = run_relay_chain_validator_node(tokio_handle.clone(), Bob, || {}, vec![alice.addr.clone()]);

	// register parachain
	alice
		.register_parachain(
			para_id,
			node_runtime::WASM_BINARY
				.expect("You need to build the WASM binary to run this test!")
				.to_vec(),
			initial_head_data(),
		)
		.await
		.unwrap();

	// run cumulus charlie (a parachain collator)
	let node = test_service::TestNodeBuilder::new(para_id, tokio_handle.clone(), Alice)
		.enable_collator()
		.connect_to_relay_chain_nodes(vec![&alice, &bob])
		.build()
		.await;
	node.wait_for_blocks(2).await;

	let (alice, bob) = (MultiSigner::from(Alice.public()), MultiSigner::from(Bob.public()));
	let (alice_account_id, bob_account_id) = (alice.into_account(), bob.into_account());
	let amount = 1_000_000_000_000;

	type Balances = pallet_balances::Pallet<node_runtime::Runtime>;

	// the function with_state allows us to read state, pretty cool right? :D
	let old_balance = node.with_state(|| Balances::free_balance(bob_account_id.clone()));

	node.transfer(Alice, Bob, amount).await.unwrap();

	node.wait_for_blocks(3).await;
	// we can check the new state :D
	let new_balance = node.with_state(|| Balances::free_balance(bob_account_id));
	assert_eq!(old_balance + amount, new_balance);
}
