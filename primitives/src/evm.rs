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

use crate::Balance;
use codec::{Decode, Encode};
use evm::ExitReason;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::{H160, U256};
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

pub use evm::backend::{Basic as Account, Log};
pub use evm::Config;

/// Evm Address.
pub type EvmAddress = sp_core::H160;

#[derive(Clone, Eq, PartialEq, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
/// External input from the transaction.
pub struct Vicinity {
	/// Current transaction gas price.
	pub gas_price: U256,
	/// Origin of the transaction.
	pub origin: EvmAddress,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CreateInfo {
	pub exit_reason: ExitReason,
	pub address: EvmAddress,
	pub output: Vec<u8>,
	pub used_gas: U256,
	pub used_storage: i32,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CallInfo {
	pub exit_reason: ExitReason,
	pub output: Vec<u8>,
	pub used_gas: U256,
	pub used_storage: i32,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Erc20Info {
	pub address: EvmAddress,
	pub name: Vec<u8>,
	pub symbol: Vec<u8>,
	pub decimals: u8,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct EstimateResourcesRequest {
	/// From
	pub from: Option<H160>,
	/// To
	pub to: Option<H160>,
	/// Gas Limit
	pub gas_limit: Option<u64>,
	/// Storage Limit
	pub storage_limit: Option<u32>,
	/// Value
	pub value: Option<Balance>,
	/// Data
	pub data: Option<Vec<u8>>,
}
