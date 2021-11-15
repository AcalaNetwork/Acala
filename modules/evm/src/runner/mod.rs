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

pub mod stack;
pub mod state;
pub mod storage_meter;

use crate::{BalanceOf, CallInfo, Config, CreateInfo, ExitError};
use frame_support::dispatch::DispatchError;
use module_evm_utiltity::evm::{self, backend::Backend, Transfer};
pub use primitives::evm::{EvmAddress, Vicinity};
use sp_core::{H160, H256};
use sp_std::vec::Vec;
use state::StackSubstateMetadata;

pub trait Runner<T: Config> {
	fn call(
		source: H160,
		origin: H160,
		target: H160,
		input: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CallInfo, DispatchError>;

	fn create(
		source: H160,
		init: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError>;

	fn create2(
		source: H160,
		init: Vec<u8>,
		salt: H256,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError>;

	fn create_at_address(
		source: H160,
		address: H160,
		init: Vec<u8>,
		value: BalanceOf<T>,
		gas_limit: u64,
		storage_limit: u32,
		config: &evm::Config,
	) -> Result<CreateInfo, DispatchError>;
}

pub trait StackState<'config>: Backend {
	fn metadata(&self) -> &StackSubstateMetadata<'config>;
	fn metadata_mut(&mut self) -> &mut StackSubstateMetadata<'config>;

	fn enter(&mut self, gas_limit: u64, is_static: bool);
	fn exit_commit(&mut self) -> Result<(), ExitError>;
	fn exit_revert(&mut self) -> Result<(), ExitError>;
	fn exit_discard(&mut self) -> Result<(), ExitError>;

	fn is_empty(&self, address: H160) -> bool;
	fn deleted(&self, address: H160) -> bool;
	fn is_cold(&self, address: H160) -> bool;
	fn is_storage_cold(&self, address: H160, key: H256) -> bool;

	fn inc_nonce(&mut self, address: H160);
	fn set_storage(&mut self, address: H160, key: H256, value: H256);
	fn reset_storage(&mut self, address: H160);
	fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>);
	fn set_deleted(&mut self, address: H160);
	fn set_code(&mut self, address: H160, code: Vec<u8>);
	fn transfer(&mut self, transfer: Transfer) -> Result<(), ExitError>;
	fn reset_balance(&mut self, address: H160);
	fn touch(&mut self, address: H160);
}
