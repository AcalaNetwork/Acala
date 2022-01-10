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

use crate::{Origin, Runtime, TransactionPause};

use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;

runtime_benchmarks! {
	{ Runtime, module_transaction_pause }

	pause_transaction {
	}: _(RawOrigin::Root, b"Balances".to_vec(), b"transfer".to_vec())

	unpause_transaction {
		TransactionPause::pause_transaction(Origin::root(), b"Balances".to_vec(), b"transfer".to_vec())?;
	}: _(RawOrigin::Root, b"Balances".to_vec(), b"transfer".to_vec())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
