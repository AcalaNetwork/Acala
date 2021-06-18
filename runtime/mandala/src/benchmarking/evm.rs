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

use crate::{dollar, AccountId, Event, EvmAccounts, Origin, Runtime, System, ACA, EVM};

use super::utils::set_aca_balance;
use frame_support::dispatch::DispatchError;
use frame_system::RawOrigin;
use orml_benchmarking::{runtime_benchmarks, whitelist_account};
use sp_core::H160;
use sp_io::hashing::keccak_256;
use sp_std::str::FromStr;

fn contract_addr() -> H160 {
	H160::from_str("0x5e0b4bfa0b55932a3587e648c3552a6515ba56b1").unwrap()
}

fn alice() -> secp256k1::SecretKey {
	secp256k1::SecretKey::parse(&keccak_256(b"Alice")).unwrap()
}

fn bob() -> secp256k1::SecretKey {
	secp256k1::SecretKey::parse(&keccak_256(b"Bob")).unwrap()
}

fn deploy_contract(caller: AccountId) -> Result<H160, DispatchError> {
	// pragma solidity ^0.5.0;
	//
	// contract Factory {
	//     Contract[] newContracts;
	//
	//     function createContract () public payable {
	//         Contract newContract = new Contract();
	//         newContracts.push(newContract);
	//     }
	// }
	//
	// contract Contract {}
	let contract = hex_literal::hex!("608060405234801561001057600080fd5b5061016f806100206000396000f3fe608060405260043610610041576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063412a5a6d14610046575b600080fd5b61004e610050565b005b600061005a6100e2565b604051809103906000f080158015610076573d6000803e3d6000fd5b50905060008190806001815401808255809150509060018203906000526020600020016000909192909190916101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505050565b6040516052806100f28339019056fe6080604052348015600f57600080fd5b50603580601d6000396000f3fe6080604052600080fdfea165627a7a7230582092dc1966a8880ddf11e067f9dd56a632c11a78a4afd4a9f05924d427367958cc0029a165627a7a723058202b2cc7384e11c452cdbf39b68dada2d5e10a632cc0174a354b8b8c83237e28a40029").to_vec();

	System::set_block_number(1);
	EVM::create(Origin::signed(caller), contract, 0, 1000000000, 1000000000)
		.map_or_else(|e| Err(e.error), |_| Ok(()))?;

	System::assert_last_event(Event::EVM(module_evm::Event::Created(contract_addr())));
	Ok(contract_addr())
}

pub fn alice_account_id() -> AccountId {
	let address = EvmAccounts::eth_address(&alice());
	let mut data = [0u8; 32];
	data[0..4].copy_from_slice(b"evm:");
	data[4..24].copy_from_slice(&address[..]);
	AccountId::from(Into::<[u8; 32]>::into(data))
}

pub fn bob_account_id() -> AccountId {
	let address = EvmAccounts::eth_address(&bob());
	let mut data = [0u8; 32];
	data[0..4].copy_from_slice(b"evm:");
	data[4..24].copy_from_slice(&address[..]);
	AccountId::from(Into::<[u8; 32]>::into(data))
}

runtime_benchmarks! {
	{ Runtime, module_evm }

	transfer_maintainer {
		let alice_account = alice_account_id();

		set_aca_balance(&alice_account, 1_000 * dollar(ACA));
		set_aca_balance(&bob_account_id(), 1_000 * dollar(ACA));
		let contract = deploy_contract(alice_account_id())?;
		let bob_address = EvmAccounts::eth_address(&bob());

		whitelist_account!(alice_account);
	}: _(RawOrigin::Signed(alice_account_id()), contract, bob_address)

	deploy {
		let alice_account = alice_account_id();

		set_aca_balance(&alice_account, 1_000 * dollar(ACA));
		set_aca_balance(&bob_account_id(), 1_000 * dollar(ACA));
		let contract = deploy_contract(alice_account_id())?;

		whitelist_account!(alice_account);
	}: _(RawOrigin::Signed(alice_account_id()), contract)

	deploy_free {
		let alice_account = alice_account_id();

		set_aca_balance(&alice_account, 1_000 * dollar(ACA));
		set_aca_balance(&bob_account_id(), 1_000 * dollar(ACA));
		let contract = deploy_contract(alice_account_id())?;
	}: _(RawOrigin::Root, contract)

	enable_contract_development {
		let alice_account = alice_account_id();

		set_aca_balance(&alice_account, 1_000 * dollar(ACA));

		whitelist_account!(alice_account);
	}: _(RawOrigin::Signed(alice_account_id()))

	disable_contract_development {
		let alice_account = alice_account_id();

		set_aca_balance(&alice_account, 1_000 * dollar(ACA));
		EVM::enable_contract_development(Origin::signed(alice_account_id()))?;

		whitelist_account!(alice_account);
	}: _(RawOrigin::Signed(alice_account_id()))

	set_code {
		let alice_account = alice_account_id();

		set_aca_balance(&alice_account, 1_000 * dollar(ACA));
		let contract = deploy_contract(alice_account_id())?;

		let new_contract = hex_literal::hex!("608060405234801561001057600080fd5b5061016f806100206000396000f3fe608060405260043610610041576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063412a5a6d14610046575b600080fd5b61004e610050565b005b600061005a6100e2565b604051809103906000f080158015610076573d6000803e3d6000fd5b50905060008190806001815401808255809150509060018203906000526020600020016000909192909190916101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505050565b6040516052806100f28339019056fe6080604052348015600f57600080fd5b50603580601d6000396000f3fe6080604052600080fdfea165627a7a7230582092dc1966a8880ddf11e067f9dd56a632c11a78a4afd4a9f05924d427367958cc0029a165627a7a723058202b2cc7384e11c452cdbf39b68dada2d5e10a632cc0174a354b8b8c83237e28a400291234").to_vec();

		whitelist_account!(alice_account);
	}: _(RawOrigin::Signed(alice_account_id()), contract, new_contract)

	selfdestruct {
		let alice_account = alice_account_id();

		set_aca_balance(&alice_account, 1_000 * dollar(ACA));
		let contract = deploy_contract(alice_account_id())?;

		whitelist_account!(alice_account);
	}: _(RawOrigin::Signed(alice_account_id()), contract)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
