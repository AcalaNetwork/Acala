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

#![allow(clippy::erasing_op)]
#![cfg(test)]
use super::*;
use crate::precompile::mock::PrecompilesValue;
use module_evm::{Context, ExitRevert};
use primitives::evm::{PRECOMPILE_ADDRESS_START, PREDEPLOY_ADDRESS_START};

#[test]
fn precompile_filter_works_on_acala_precompiles() {
	let precompile = PRECOMPILE_ADDRESS_START;

	let mut non_system = [0u8; 20];
	non_system[0] = 1;

	let non_system_caller_context = Context {
		address: precompile,
		caller: non_system.into(),
		apparent_value: 0.into(),
	};
	assert_eq!(
		PrecompilesValue::get().execute(precompile, &[0u8; 1], Some(10), &non_system_caller_context, false),
		Some(Err(PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: "NoPermission".into(),
			cost: 10,
		})),
	);
}

#[test]
fn precompile_filter_does_not_work_on_system_contracts() {
	let system = PREDEPLOY_ADDRESS_START;

	let mut non_system = [0u8; 20];
	non_system[0] = 1;

	let non_system_caller_context = Context {
		address: system,
		caller: non_system.into(),
		apparent_value: 0.into(),
	};
	assert!(PrecompilesValue::get()
		.execute(non_system.into(), &[0u8; 1], None, &non_system_caller_context, false)
		.is_none());
}

#[test]
fn precompile_filter_does_not_work_on_non_system_contracts() {
	let mut non_system = [0u8; 20];
	non_system[0] = 1;
	let mut another_non_system = [0u8; 20];
	another_non_system[0] = 2;

	let non_system_caller_context = Context {
		address: non_system.into(),
		caller: another_non_system.into(),
		apparent_value: 0.into(),
	};
	assert!(PrecompilesValue::get()
		.execute(non_system.into(), &[0u8; 1], None, &non_system_caller_context, false)
		.is_none());
}
