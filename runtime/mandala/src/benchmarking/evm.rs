use crate::{AccountId, Balance, Event, EvmAccounts, Origin, Runtime, System, DOLLARS, EVM};

use super::utils::set_aca_balance;
use frame_support::dispatch::DispatchError;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use sp_core::H160;
use sp_io::hashing::keccak_256;
use sp_std::{prelude::*, vec};

fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	DOLLARS.saturating_mul(d)
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

	if let Event::module_evm(module_evm::Event::Created(address)) = System::events().iter().last().unwrap().event {
		Ok(address)
	} else {
		Err("deploy_contract failed".into())
	}
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

	_ {}

	transfer_maintainer {
		set_aca_balance(&alice_account_id(), dollar(1000));
		set_aca_balance(&bob_account_id(), dollar(1000));
		let contract = deploy_contract(alice_account_id())?;
		let bob_address = EvmAccounts::eth_address(&bob());
	}: _(RawOrigin::Signed(alice_account_id()), contract, bob_address)

	deploy {
		set_aca_balance(&alice_account_id(), dollar(1000));
		set_aca_balance(&bob_account_id(), dollar(1000));
		let contract = deploy_contract(alice_account_id())?;
	}: _(RawOrigin::Signed(alice_account_id()), contract)

	deploy_free {
		set_aca_balance(&alice_account_id(), dollar(1000));
		set_aca_balance(&bob_account_id(), dollar(1000));
		let contract = deploy_contract(alice_account_id())?;
	}: _(RawOrigin::Root, contract)

	enable_contract_development {
		set_aca_balance(&alice_account_id(), dollar(1000));
	}: _(RawOrigin::Signed(alice_account_id()))

	disable_contract_development {
		set_aca_balance(&alice_account_id(), dollar(1000));
		EVM::enable_contract_development(Origin::signed(alice_account_id()))?;
	}: _(RawOrigin::Signed(alice_account_id()))

	set_code {
		set_aca_balance(&alice_account_id(), dollar(1000));
		let contract = deploy_contract(alice_account_id())?;

		let new_contract = hex_literal::hex!("608060405234801561001057600080fd5b5061016f806100206000396000f3fe608060405260043610610041576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063412a5a6d14610046575b600080fd5b61004e610050565b005b600061005a6100e2565b604051809103906000f080158015610076573d6000803e3d6000fd5b50905060008190806001815401808255809150509060018203906000526020600020016000909192909190916101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505050565b6040516052806100f28339019056fe6080604052348015600f57600080fd5b50603580601d6000396000f3fe6080604052600080fdfea165627a7a7230582092dc1966a8880ddf11e067f9dd56a632c11a78a4afd4a9f05924d427367958cc0029a165627a7a723058202b2cc7384e11c452cdbf39b68dada2d5e10a632cc0174a354b8b8c83237e28a400291234").to_vec();

	}: _(RawOrigin::Signed(alice_account_id()), contract, new_contract)

	selfdestruct {
		set_aca_balance(&alice_account_id(), dollar(1000));
		let contract = deploy_contract(alice_account_id())?;
	}: _(RawOrigin::Signed(alice_account_id()), contract)
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::assert_ok;

	fn new_test_ext() -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}

	#[test]
	fn test_transfer_maintainer() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_transfer_maintainer());
		});
	}

	#[test]
	fn test_deploy() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_deploy());
		});
	}

	#[test]
	fn test_deploy_free() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_deploy_free());
		});
	}

	#[test]
	fn test_enable_contract_development() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_enable_contract_development());
		});
	}

	#[test]
	fn test_disable_contract_development() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_disable_contract_development());
		});
	}

	#[test]
	fn test_set_code() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_set_code());
		});
	}

	#[test]
	fn test_selfdestruct() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_selfdestruct());
		});
	}
}
