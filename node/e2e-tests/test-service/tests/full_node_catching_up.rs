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
use test_service::{initial_head_data, run_relay_chain_validator_node, Keyring::*};

#[substrate_test_utils::test]
#[ignore]
async fn test_full_node_catching_up() {
	// let mut builder =
	// sc_cli::LoggerBuilder::new("parachain::candidate-selection=trace,parachain::pvf=trace,parachain::
	// collator-protocol=trace,parachain::provisioner=trace,test-service=info"); let mut builder =
	// sc_cli::LoggerBuilder::new("debug,wasmtime=info,afg=info,db=info,wasm_overrides=info,sync=info");
	let mut builder = sc_cli::LoggerBuilder::new("parachain=debug");
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
			initial_head_data(para_id),
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
	let dave = test_service::TestNodeBuilder::new(para_id, tokio_handle, Dave)
		.connect_to_parachain_node(&charlie)
		.connect_to_relay_chain_nodes(vec![&alice, &bob])
		.build()
		.await;
	dave.wait_for_blocks(7).await;
}
