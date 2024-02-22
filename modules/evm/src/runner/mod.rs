// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

pub mod stack;
pub mod state;
pub mod storage_meter;
pub mod tagged_runtime;

#[cfg(feature = "tracing")]
pub mod tracing;

use crate::{BalanceOf, CallInfo, Config, CreateInfo};
use module_evm_utility::evm;
pub use primitives::evm::{EvmAddress, Vicinity};
use sp_core::{H160, H256};
use sp_runtime::DispatchError;
use sp_std::vec::Vec;

pub trait Runner<T: Config> {
	fn call(
		source: H160,
		origin: H160,
		target: H160,
		input: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		access_list: Vec<(H160, Vec<H256>)>,
		config: &evm::Config,
	) -> Result<CallInfo, DispatchError>;

	fn create(
		source: H160,
		init: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		access_list: Vec<(H160, Vec<H256>)>,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError>;

	fn create2(
		source: H160,
		init: Vec<u8>,
		salt: H256,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		access_list: Vec<(H160, Vec<H256>)>,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError>;

	fn create_at_address(
		source: H160,
		address: H160,
		init: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		access_list: Vec<(H160, Vec<H256>)>,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError>;
}

pub trait RunnerExtended<T: Config>: Runner<T> {
	fn rpc_call(
		source: H160,
		origin: H160,
		target: H160,
		input: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		access_list: Vec<(H160, Vec<H256>)>,
		config: &evm::Config,
	) -> Result<CallInfo, DispatchError>;

	fn rpc_create(
		source: H160,
		init: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		access_list: Vec<(H160, Vec<H256>)>,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError>;
}
