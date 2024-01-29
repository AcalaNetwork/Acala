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

#![allow(clippy::erasing_op)]
#![cfg(test)]
use super::*;
use crate::precompile::mock::{new_test_ext, PrecompilesValue};
use module_evm::precompiles::tests::MockPrecompileHandle;
use module_evm::{Context, ExitRevert};
use primitives::evm::{PRECOMPILE_ADDRESS_START, PREDEPLOY_ADDRESS_START};

#[test]
fn precompile_filter_works_on_acala_precompiles() {
	new_test_ext().execute_with(|| {
		let precompile = PRECOMPILE_ADDRESS_START;

		let mut non_system = [0u8; 20];
		non_system[0] = 1;

		let non_system_caller_context = Context {
			address: precompile,
			caller: non_system.into(),
			apparent_value: 0.into(),
		};
		let mut handle = MockPrecompileHandle {
			input: &[0u8; 1],
			code_address: precompile,
			gas_limit: Some(10),
			gas_used: 0,
			context: &non_system_caller_context,
			is_static: false,
		};
		assert_eq!(
			PrecompilesValue::get().execute(&mut handle),
			Some(Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "NoPermission".into(),
			})),
		);
	});
}

#[test]
fn precompile_filter_does_not_work_on_system_contracts() {
	new_test_ext().execute_with(|| {
		let system = PREDEPLOY_ADDRESS_START;

		let mut non_system = [0u8; 20];
		non_system[0] = 1;

		let non_system_caller_context = Context {
			address: system,
			caller: non_system.into(),
			apparent_value: 0.into(),
		};
		let mut handle = MockPrecompileHandle {
			input: &[0u8; 1],
			code_address: non_system.into(),
			gas_limit: None,
			gas_used: 0,
			context: &non_system_caller_context,
			is_static: false,
		};
		assert!(PrecompilesValue::get().execute(&mut handle).is_none());
	});
}

#[test]
fn precompile_filter_does_not_work_on_non_system_contracts() {
	new_test_ext().execute_with(|| {
		let mut non_system = [0u8; 20];
		non_system[0] = 1;
		let mut another_non_system = [0u8; 20];
		another_non_system[0] = 2;

		let non_system_caller_context = Context {
			address: non_system.into(),
			caller: another_non_system.into(),
			apparent_value: 0.into(),
		};
		let mut handle = MockPrecompileHandle {
			input: &[0u8; 1],
			code_address: non_system.into(),
			gas_limit: None,
			gas_used: 0,
			context: &non_system_caller_context,
			is_static: false,
		};
		assert!(PrecompilesValue::get().execute(&mut handle).is_none());
	});
}
