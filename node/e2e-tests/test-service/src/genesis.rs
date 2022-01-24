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

use codec::Encode;
use cumulus_client_service::genesis::generate_genesis_block;
use cumulus_primitives_core::ParaId;
use node_runtime::Block;
use node_service::chain_spec::mandala::dev_testnet_config;
use polkadot_primitives::v0::HeadData;
use sp_runtime::traits::Block as BlockT;

/// Returns the initial head data for a parachain ID.
pub fn initial_head_data(para_id: ParaId) -> HeadData {
	let spec = Box::new(dev_testnet_config(None).unwrap());
	let block: Block = generate_genesis_block(&(spec as Box<_>), sp_runtime::StateVersion::V1).unwrap();
	let genesis_state = block.header().encode();
	genesis_state.into()
}
