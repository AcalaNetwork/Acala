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

use primitives::evm::AccessListItem;
use serde::{Deserialize, Serialize};
use sp_core::{Bytes, H160, U256};
use sp_rpc::number::NumberOrHex;

/// Call request
#[derive(Debug, Default, PartialEq, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct CallRequest {
	/// From
	pub from: Option<H160>,
	/// To
	pub to: Option<H160>,
	/// Gas Limit
	pub gas_limit: Option<u64>,
	/// Storage Limit
	pub storage_limit: Option<u32>,
	/// Value
	pub value: Option<NumberOrHex>,
	/// Data
	pub data: Option<Bytes>,
	/// AccessList
	pub access_list: Option<Vec<AccessListItem>>,
}

/// EstimateResources response
#[derive(Debug, Eq, PartialEq, Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EstimateResourcesResponse {
	/// Used gas
	pub gas: U256,
	/// Used storage
	pub storage: i32,
	/// Adjusted weight fee
	pub weight_fee: U256,
}
