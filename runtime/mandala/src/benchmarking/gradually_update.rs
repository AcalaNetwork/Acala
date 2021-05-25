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

use crate::{GraduallyUpdate, Origin, Runtime, System, UpdateFrequency};

use frame_support::traits::OnFinalize;
use orml_benchmarking::runtime_benchmarks;
use sp_std::{convert::TryInto, prelude::*};

const MAX_TARGET_VALUE: u32 = 100;

runtime_benchmarks! {
	{ Runtime, orml_gradually_update }

	// gradually update numeric parameter
	gradually_update {
		System::set_block_number(1);
		let update = orml_gradually_update::GraduallyUpdate {
			key: vec![1].try_into().unwrap(),
			target_value: vec![10].try_into().unwrap(),
			per_block: vec![1].try_into().unwrap(),
		};
	}: _(Origin::root(), update)

	// cancel gradually update
	cancel_gradually_update {
		let update = orml_gradually_update::GraduallyUpdate {
			key: vec![1].try_into().unwrap(),
			target_value: vec![10].try_into().unwrap(),
			per_block: vec![1].try_into().unwrap(),
		};
		GraduallyUpdate::gradually_update(Origin::root(), update.clone())?;
	}: _(Origin::root(), update.key)

	// execute gradually update
	on_finalize {
		let u in 2 .. MAX_TARGET_VALUE;

		System::set_block_number(1);
		for i in 1..u {
			let update = orml_gradually_update::GraduallyUpdate {
				key: vec![1].try_into().unwrap(),
				target_value: vec![200].try_into().unwrap(),
				per_block: vec![i as u8].try_into().unwrap(),
			};
			GraduallyUpdate::gradually_update(Origin::root(), update)?;
		}

		System::set_block_number(1 + UpdateFrequency::get());
	}: {
		GraduallyUpdate::on_finalize(System::block_number());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
