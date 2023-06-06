// This file is part of Acala.

// Copyright (C) 2020-2023 Acala Foundation.
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

use crate::{Config, PausedEvmPrecompiles, Weight, H160};
use frame_support::{log, traits::OnRuntimeUpgrade};
use hex_literal::hex;
use sp_core::Get;
use sp_std::{marker::PhantomData, vec};

pub struct MigrateEvmPrecompile<T>(PhantomData<T>);
impl<T: Config> OnRuntimeUpgrade for MigrateEvmPrecompile<T> {
	fn on_runtime_upgrade() -> Weight {
		let mut weight: Weight = Weight::zero();

		let address_list = vec![
			H160(hex!("0000000000000000000000000000000000000406")), // STABLE_ASSET
			H160(hex!("0000000000000000000000000000000000000407")), // HOMA
			H160(hex!("0000000000000000000000000000000000000409")), // HONZON
			H160(hex!("000000000000000000000000000000000000040a")), // INCENTIVES
			H160(hex!("000000000000000000000000000000000000040b")), // XTOKENS
		];

		log::info!(
			target: "transaction-pause",
			"MigrateEvmPrecompile::on_runtime_upgrade execute, will pause the address {:?}", address_list
		);

		for addr in address_list.iter() {
			PausedEvmPrecompiles::<T>::mutate_exists(addr, |maybe_paused| {
				if maybe_paused.is_none() {
					*maybe_paused = Some(());
				}
			});
		}

		weight.saturating_accrue(T::DbWeight::get().writes(address_list.len().try_into().unwrap()));
		weight
	}
}
