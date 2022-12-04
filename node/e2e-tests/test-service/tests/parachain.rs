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
use futures::join;
use sp_keyring::Sr25519Keyring::*;
use sp_runtime::{traits::IdentifyAccount, MultiSigner};
use test_service::{initial_head_data, run_relay_chain_validator_node};

/// this testcase will take too long to running, test with command:
/// cargo test --package test-service -- test_full_node_catching_up --nocapture --include-ignored
#[substrate_test_utils::test(flavor = "multi_thread")]
#[ignore]
async fn test_full_node_catching_up() {
	let mut builder = sc_cli::LoggerBuilder::new("");
	builder.with_colors(true);
	let _ = builder.init();

	let para_id = ParaId::from(2000);

	let tokio_handle = tokio::runtime::Handle::current();

	let ws_port = portpicker::pick_unused_port().expect("No free ports");

	// start relay chain node: alice
	let alice = run_relay_chain_validator_node(tokio_handle.clone(), Alice, || {}, Vec::new(), Some(ws_port));

	// start relay chain node: bob
	let bob = run_relay_chain_validator_node(tokio_handle.clone(), Bob, || {}, vec![alice.addr.clone()], None);

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
	let charlie = test_service::TestNodeBuilder::new(para_id, tokio_handle.clone(), Charlie)
		.enable_collator()
		.connect_to_relay_chain_nodes(vec![&alice, &bob])
		.build()
		.await;
	charlie.wait_for_blocks(5).await;

	// run cumulus dave (a parachain full node) and wait for it to sync some blocks
	let dave = test_service::TestNodeBuilder::new(para_id, tokio_handle.clone(), Dave)
		.connect_to_parachain_node(&charlie)
		.connect_to_relay_chain_nodes(vec![&alice, &bob])
		.build()
		.await;

	// run cumulus dave (a parachain full node) and wait for it to sync some blocks
	let eve = test_service::TestNodeBuilder::new(para_id, tokio_handle, Eve)
		.connect_to_parachain_node(&charlie)
		.connect_to_relay_chain_nodes(vec![&alice, &bob])
		.use_external_relay_chain_node_at_port(ws_port)
		.build()
		.await;

	join!(dave.wait_for_blocks(7), eve.wait_for_blocks(7));
}

/// this testcase will take too long to running, test with command:
/// cargo test --package test-service -- simple_balances_test --nocapture --include-ignored
#[substrate_test_utils::test(flavor = "multi_thread")]
#[ignore]
async fn simple_balances_test() {
	let mut builder = sc_cli::LoggerBuilder::new("");
	builder.with_colors(true);
	let _ = builder.init();

	let para_id = ParaId::from(2000);

	let tokio_handle = tokio::runtime::Handle::current();

	let ws_port = portpicker::pick_unused_port().expect("No free ports");

	// start alice
	let alice = run_relay_chain_validator_node(tokio_handle.clone(), Alice, || {}, Vec::new(), Some(ws_port));

	// start bob
	let bob = run_relay_chain_validator_node(tokio_handle.clone(), Bob, || {}, vec![alice.addr.clone()], None);

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
	let charlie = test_service::TestNodeBuilder::new(para_id, tokio_handle.clone(), Charlie)
		.enable_collator()
		.connect_to_relay_chain_nodes(vec![&alice, &bob])
		.build()
		.await;

	// run cumulus dave (a parachain full node)
	let dave = test_service::TestNodeBuilder::new(para_id, tokio_handle, Dave)
		.connect_to_parachain_node(&charlie)
		.connect_to_relay_chain_nodes(vec![&alice, &bob])
		.build()
		.await;

	// Wait for 2 blocks to be produced
	dave.wait_for_blocks(2).await;

	let bob = MultiSigner::from(Bob.public());
	let bob_account_id = bob.into_account();
	let amount = 1_000_000_000_000;

	type Balances = pallet_balances::Pallet<node_runtime::Runtime>;

	// the function with_state allows us to read state, pretty cool right? :D
	let old_balance = dave.with_state(|| Balances::free_balance(bob_account_id.clone()));

	dave.transfer(Alice, Bob, amount, 0).await.unwrap();

	dave.wait_for_blocks(3).await;
	// we can check the new state :D
	let new_balance = dave.with_state(|| Balances::free_balance(bob_account_id));
	assert_eq!(old_balance + amount, new_balance);
}
