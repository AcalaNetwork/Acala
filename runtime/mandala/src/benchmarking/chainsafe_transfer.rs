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

use crate::{
	dollar, AccountId, ChainBridge, ChainSafeTransfer, CurrencyId, GetNativeCurrencyId, LocalChainId, Runtime,
	RuntimeBlockLength, ACA, AUSD,
};

use super::utils::set_balance;
use frame_benchmarking::account;
use frame_support::{traits::EnsureOrigin, weights::DispatchClass};
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use sp_std::vec;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, ecosystem_chainsafe }

	register_resource_id {
		let currency_id: CurrencyId = ACA;
		let resource_id: chainbridge::ResourceId = chainbridge::derive_resource_id(LocalChainId::get(), b"aca");
	}: _(RawOrigin::Root, resource_id, currency_id)

	remove_resource_id {
		let currency_id: CurrencyId = ACA;
		let resource_id: chainbridge::ResourceId = chainbridge::derive_resource_id(LocalChainId::get(), b"aca");

		ChainSafeTransfer::register_resource_id(RawOrigin::Root.into(), resource_id, currency_id)?;
	}: _(RawOrigin::Root, resource_id)

	transfer_origin_chain_token_to_bridge {
		let b in 0 .. *RuntimeBlockLength::get().max.get(DispatchClass::Normal) as u32;
		let dest = vec![1; b as usize];
		let sender: AccountId = account("sender", 0, SEED);
		let currency_id: CurrencyId = AUSD;
		let resource_id: chainbridge::ResourceId = chainbridge::derive_resource_id(LocalChainId::get(), b"ausd");
		let dest_chain_id: chainbridge::ChainId = 0;

		ChainBridge::whitelist_chain(RawOrigin::Root.into(), dest_chain_id)?;
		ChainSafeTransfer::register_resource_id(RawOrigin::Root.into(), resource_id, currency_id)?;
		set_balance(currency_id, &sender, dollar(currency_id) * 100);
	}: transfer_to_bridge(RawOrigin::Signed(sender), currency_id, dest_chain_id, dest, dollar(currency_id) * 10)

	transfer_other_chain_token_to_bridge {
		let b in 0 .. *RuntimeBlockLength::get().max.get(DispatchClass::Normal) as u32;
		let dest = vec![1; b as usize];
		let sender: AccountId = account("sender", 0, SEED);
		let resource_id: chainbridge::ResourceId = chainbridge::derive_resource_id(0, b"weth");
		let currency_id: CurrencyId = CurrencyId::ChainSafe(resource_id);
		let dest_chain_id: chainbridge::ChainId = 0;

		ChainBridge::whitelist_chain(RawOrigin::Root.into(), dest_chain_id)?;
		ChainSafeTransfer::register_resource_id(RawOrigin::Root.into(), resource_id, currency_id)?;
		set_balance(currency_id, &sender, 10_000_000_000_000_000_000u128);
	}: transfer_to_bridge(RawOrigin::Signed(sender), currency_id, dest_chain_id, dest, 1_000_000_000_000_000_000u128)

	transfer_native_to_bridge {
		let b in 0 .. *RuntimeBlockLength::get().max.get(DispatchClass::Normal) as u32;
		let dest = vec![1; b as usize];
		let sender: AccountId = account("sender", 0, SEED);
		let currency_id: CurrencyId = GetNativeCurrencyId::get();
		let resource_id: chainbridge::ResourceId = chainbridge::derive_resource_id(LocalChainId::get(), b"native");
		let dest_chain_id: chainbridge::ChainId = 0;

		ChainBridge::whitelist_chain(RawOrigin::Root.into(), dest_chain_id)?;
		ChainSafeTransfer::register_resource_id(RawOrigin::Root.into(), resource_id, currency_id)?;
		set_balance(currency_id, &sender, dollar(currency_id) * 100);
	}: _(RawOrigin::Signed(sender), dest_chain_id, dest, dollar(currency_id) * 10)

	transfer_origin_chain_token_from_bridge {
		let sender: AccountId = account("sender", 0, SEED);
		let receiver: AccountId = account("receiver", 0, SEED);
		let currency_id: CurrencyId = AUSD;
		let resource_id: chainbridge::ResourceId = chainbridge::derive_resource_id(LocalChainId::get(), b"ausd");
		let dest_chain_id: chainbridge::ChainId = 0;

		ChainBridge::whitelist_chain(RawOrigin::Root.into(), dest_chain_id)?;
		ChainSafeTransfer::register_resource_id(RawOrigin::Root.into(), resource_id, currency_id)?;
		set_balance(currency_id, &sender, dollar(currency_id) * 100);
		ChainSafeTransfer::transfer_to_bridge(RawOrigin::Signed(sender).into(), currency_id, dest_chain_id, vec![0], dollar(currency_id) * 100)?;
	}: transfer_from_bridge(chainbridge::EnsureBridge::<Runtime>::successful_origin(), receiver, dollar(currency_id) * 10, resource_id)

	transfer_other_chain_token_from_bridge {
		let sender: AccountId = account("sender", 0, SEED);
		let receiver: AccountId = account("receiver", 0, SEED);
		let resource_id: chainbridge::ResourceId = chainbridge::derive_resource_id(0, b"weth");
		let currency_id: CurrencyId = CurrencyId::ChainSafe(resource_id);
		let dest_chain_id: chainbridge::ChainId = 0;

		ChainBridge::whitelist_chain(RawOrigin::Root.into(), dest_chain_id)?;
		ChainSafeTransfer::register_resource_id(RawOrigin::Root.into(), resource_id, currency_id)?;
		set_balance(currency_id, &sender, 10_000_000_000_000_000_000u128);
		ChainSafeTransfer::transfer_to_bridge(RawOrigin::Signed(sender).into(), currency_id, dest_chain_id, vec![0], 10_000_000_000_000_000_000u128)?;
	}: transfer_from_bridge(chainbridge::EnsureBridge::<Runtime>::successful_origin(), receiver, 1_000_000_000_000_000_000u128, resource_id)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
