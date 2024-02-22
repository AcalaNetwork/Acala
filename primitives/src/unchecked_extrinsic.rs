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

use crate::{evm::EthereumTransactionMessage, signature::AcalaMultiSignature, to_bytes, Address, Balance};
use frame_support::{
	dispatch::{DispatchInfo, GetDispatchInfo},
	traits::{ExtrinsicCall, Get},
};
use module_evm_utility::ethereum::{
	EIP1559TransactionMessage, EIP2930TransactionMessage, LegacyTransactionMessage, TransactionAction,
};
use module_evm_utility_macro::keccak256;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_core::{H160, H256};
use sp_io::{crypto::secp256k1_ecdsa_recover, hashing::keccak_256};
use sp_runtime::{
	generic::{CheckedExtrinsic, UncheckedExtrinsic},
	traits::{self, Checkable, Convert, Extrinsic, ExtrinsicMetadata, Member, SignedExtension, Zero},
	transaction_validity::{InvalidTransaction, TransactionValidityError},
	AccountId32, RuntimeDebug,
};
#[cfg(not(feature = "std"))]
use sp_std::alloc::format;
use sp_std::{marker::PhantomData, prelude::*};

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(ConvertEthTx))]
pub struct AcalaUncheckedExtrinsic<Call, Extra: SignedExtension, ConvertEthTx, StorageDepositPerByte, TxFeePerGas>(
	pub UncheckedExtrinsic<Address, Call, AcalaMultiSignature, Extra>,
	PhantomData<(ConvertEthTx, StorageDepositPerByte, TxFeePerGas)>,
);

impl<Call: TypeInfo, Extra: SignedExtension, ConvertEthTx, StorageDepositPerByte, TxFeePerGas> Extrinsic
	for AcalaUncheckedExtrinsic<Call, Extra, ConvertEthTx, StorageDepositPerByte, TxFeePerGas>
{
	type Call = Call;

	type SignaturePayload = (Address, AcalaMultiSignature, Extra);

	fn is_signed(&self) -> Option<bool> {
		self.0.is_signed()
	}

	fn new(function: Call, signed_data: Option<Self::SignaturePayload>) -> Option<Self> {
		Some(if let Some((address, signature, extra)) = signed_data {
			Self(
				UncheckedExtrinsic::new_signed(function, address, signature, extra),
				PhantomData,
			)
		} else {
			Self(UncheckedExtrinsic::new_unsigned(function), PhantomData)
		})
	}
}

impl<Call, Extra: SignedExtension, ConvertEthTx, StorageDepositPerByte, TxFeePerGas> ExtrinsicMetadata
	for AcalaUncheckedExtrinsic<Call, Extra, ConvertEthTx, StorageDepositPerByte, TxFeePerGas>
{
	const VERSION: u8 = UncheckedExtrinsic::<Address, Call, AcalaMultiSignature, Extra>::VERSION;
	type SignedExtensions = Extra;
}

impl<Call: TypeInfo, Extra: SignedExtension, ConvertEthTx, StorageDepositPerByte, TxFeePerGas> ExtrinsicCall
	for AcalaUncheckedExtrinsic<Call, Extra, ConvertEthTx, StorageDepositPerByte, TxFeePerGas>
{
	fn call(&self) -> &Self::Call {
		self.0.call()
	}
}

impl<Call, Extra, ConvertEthTx, StorageDepositPerByte, TxFeePerGas, Lookup> Checkable<Lookup>
	for AcalaUncheckedExtrinsic<Call, Extra, ConvertEthTx, StorageDepositPerByte, TxFeePerGas>
where
	Call: Encode + Member,
	Extra: SignedExtension<AccountId = AccountId32>,
	ConvertEthTx: Convert<(Call, Extra), Result<(EthereumTransactionMessage, Extra), InvalidTransaction>>,
	StorageDepositPerByte: Get<Balance>,
	TxFeePerGas: Get<Balance>,
	Lookup: traits::Lookup<Source = Address, Target = AccountId32>,
{
	type Checked = CheckedExtrinsic<AccountId32, Call, Extra>;

	fn check(self, lookup: &Lookup) -> Result<Self::Checked, TransactionValidityError> {
		let function = self.0.function.clone();

		match self.0.signature {
			Some((addr, AcalaMultiSignature::Ethereum(sig), extra)) => {
				let (eth_msg, eth_extra) = ConvertEthTx::convert((function.clone(), extra))?;
				log::trace!(
					target: "evm", "Ethereum eth_msg: {:?}", eth_msg
				);

				if !eth_msg.access_list.len().is_zero() {
					// Not yet supported, require empty
					return Err(InvalidTransaction::BadProof.into());
				}

				let (tx_gas_price, tx_gas_limit) = if eth_msg.gas_price.is_zero() {
					recover_sign_data(&eth_msg, TxFeePerGas::get(), StorageDepositPerByte::get())
						.ok_or(InvalidTransaction::BadProof)?
				} else {
					// eth_call_v2, the gas_price and gas_limit are encoded.
					(eth_msg.gas_price as u128, eth_msg.gas_limit as u128)
				};

				let msg = LegacyTransactionMessage {
					nonce: eth_msg.nonce.into(),
					gas_price: tx_gas_price.into(),
					gas_limit: tx_gas_limit.into(),
					action: eth_msg.action,
					value: eth_msg.value.into(),
					input: eth_msg.input,
					chain_id: Some(eth_msg.chain_id),
				};
				log::trace!(
					target: "evm", "tx msg: {:?}", msg
				);

				let msg_hash = msg.hash(); // TODO: consider rewirte this to use `keccak_256` for hashing because it could be faster

				let signer = recover_signer(&sig, msg_hash.as_fixed_bytes()).ok_or(InvalidTransaction::BadProof)?;

				let account_id = lookup.lookup(Address::Address20(signer.into()))?;
				let expected_account_id = lookup.lookup(addr)?;

				if account_id != expected_account_id {
					return Err(InvalidTransaction::BadProof.into());
				}

				Ok(CheckedExtrinsic {
					signed: Some((account_id, eth_extra)),
					function,
				})
			}
			Some((addr, AcalaMultiSignature::Eip2930(sig), extra)) => {
				let (eth_msg, eth_extra) = ConvertEthTx::convert((function.clone(), extra))?;
				log::trace!(
					target: "evm", "Eip2930 eth_msg: {:?}", eth_msg
				);

				let (tx_gas_price, tx_gas_limit) = if eth_msg.gas_price.is_zero() {
					recover_sign_data(&eth_msg, TxFeePerGas::get(), StorageDepositPerByte::get())
						.ok_or(InvalidTransaction::BadProof)?
				} else {
					// eth_call_v2, the gas_price and gas_limit are encoded.
					(eth_msg.gas_price as u128, eth_msg.gas_limit as u128)
				};

				let msg = EIP2930TransactionMessage {
					chain_id: eth_msg.chain_id,
					nonce: eth_msg.nonce.into(),
					gas_price: tx_gas_price.into(),
					gas_limit: tx_gas_limit.into(),
					action: eth_msg.action,
					value: eth_msg.value.into(),
					input: eth_msg.input,
					access_list: eth_msg.access_list,
				};
				log::trace!(
					target: "evm", "tx msg: {:?}", msg
				);

				let msg_hash = msg.hash(); // TODO: consider rewirte this to use `keccak_256` for hashing because it could be faster

				let signer = recover_signer(&sig, msg_hash.as_fixed_bytes()).ok_or(InvalidTransaction::BadProof)?;

				let account_id = lookup.lookup(Address::Address20(signer.into()))?;
				let expected_account_id = lookup.lookup(addr)?;

				if account_id != expected_account_id {
					return Err(InvalidTransaction::BadProof.into());
				}

				Ok(CheckedExtrinsic {
					signed: Some((account_id, eth_extra)),
					function,
				})
			}
			Some((addr, AcalaMultiSignature::Eip1559(sig), extra)) => {
				let (eth_msg, eth_extra) = ConvertEthTx::convert((function.clone(), extra))?;
				log::trace!(
					target: "evm", "Eip1559 eth_msg: {:?}", eth_msg
				);

				let (tx_gas_price, tx_gas_limit) = if eth_msg.gas_price.is_zero() {
					recover_sign_data(&eth_msg, TxFeePerGas::get(), StorageDepositPerByte::get())
						.ok_or(InvalidTransaction::BadProof)?
				} else {
					// eth_call_v2, the gas_price and gas_limit are encoded.
					(eth_msg.gas_price as u128, eth_msg.gas_limit as u128)
				};

				// tip = priority_fee * gas_limit
				let priority_fee = eth_msg.tip.checked_div(eth_msg.gas_limit.into()).unwrap_or_default();

				let msg = EIP1559TransactionMessage {
					chain_id: eth_msg.chain_id,
					nonce: eth_msg.nonce.into(),
					max_priority_fee_per_gas: priority_fee.into(),
					max_fee_per_gas: tx_gas_price.into(),
					gas_limit: tx_gas_limit.into(),
					action: eth_msg.action,
					value: eth_msg.value.into(),
					input: eth_msg.input,
					access_list: eth_msg.access_list,
				};
				log::trace!(
					target: "evm", "tx msg: {:?}", msg
				);

				let msg_hash = msg.hash(); // TODO: consider rewirte this to use `keccak_256` for hashing because it could be faster

				let signer = recover_signer(&sig, msg_hash.as_fixed_bytes()).ok_or(InvalidTransaction::BadProof)?;

				let account_id = lookup.lookup(Address::Address20(signer.into()))?;
				let expected_account_id = lookup.lookup(addr)?;

				if account_id != expected_account_id {
					return Err(InvalidTransaction::BadProof.into());
				}

				Ok(CheckedExtrinsic {
					signed: Some((account_id, eth_extra)),
					function,
				})
			}
			Some((addr, AcalaMultiSignature::AcalaEip712(sig), extra)) => {
				let (eth_msg, eth_extra) = ConvertEthTx::convert((function.clone(), extra))?;
				log::trace!(
					target: "evm", "AcalaEip712 eth_msg: {:?}", eth_msg
				);

				let signer = verify_eip712_signature(eth_msg, sig).ok_or(InvalidTransaction::BadProof)?;

				let account_id = lookup.lookup(Address::Address20(signer.into()))?;
				let expected_account_id = lookup.lookup(addr)?;

				if account_id != expected_account_id {
					return Err(InvalidTransaction::BadProof.into());
				}

				Ok(CheckedExtrinsic {
					signed: Some((account_id, eth_extra)),
					function,
				})
			}
			_ => self.0.check(lookup),
		}
	}

	#[cfg(feature = "try-runtime")]
	fn unchecked_into_checked_i_know_what_i_am_doing(
		self,
		_lookup: &Lookup,
	) -> Result<Self::Checked, TransactionValidityError> {
		unreachable!();
	}
}

impl<Call, Extra, ConvertEthTx, StorageDepositPerByte, TxFeePerGas> GetDispatchInfo
	for AcalaUncheckedExtrinsic<Call, Extra, ConvertEthTx, StorageDepositPerByte, TxFeePerGas>
where
	Call: GetDispatchInfo,
	Extra: SignedExtension,
{
	fn get_dispatch_info(&self) -> DispatchInfo {
		self.0.get_dispatch_info()
	}
}

impl<Call: Encode, Extra: SignedExtension, ConvertEthTx, StorageDepositPerByte, TxFeePerGas> serde::Serialize
	for AcalaUncheckedExtrinsic<Call, Extra, ConvertEthTx, StorageDepositPerByte, TxFeePerGas>
{
	fn serialize<S>(&self, seq: S) -> Result<S::Ok, S::Error>
	where
		S: ::serde::Serializer,
	{
		self.0.serialize(seq)
	}
}

impl<'a, Call: Decode, Extra: SignedExtension, ConvertEthTx, StorageDepositPerByte, TxFeePerGas> serde::Deserialize<'a>
	for AcalaUncheckedExtrinsic<Call, Extra, ConvertEthTx, StorageDepositPerByte, TxFeePerGas>
{
	fn deserialize<D>(de: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'a>,
	{
		let r = sp_core::bytes::deserialize(de)?;
		Decode::decode(&mut &r[..]).map_err(|e| serde::de::Error::custom(format!("Decode error: {}", e)))
	}
}

fn recover_signer(sig: &[u8; 65], msg_hash: &[u8; 32]) -> Option<H160> {
	secp256k1_ecdsa_recover(sig, msg_hash)
		.map(|pubkey| H160::from(H256::from_slice(&keccak_256(&pubkey))))
		.ok()
}

fn verify_eip712_signature(eth_msg: EthereumTransactionMessage, sig: [u8; 65]) -> Option<H160> {
	let domain_hash = keccak256!("EIP712Domain(string name,string version,uint256 chainId,bytes32 salt)");
	let access_list_type_hash = keccak256!("AccessList(address address,uint256[] storageKeys)");
	let tx_type_hash = keccak256!("Transaction(string action,address to,uint256 nonce,uint256 tip,bytes data,uint256 value,uint256 gasLimit,uint256 storageLimit,AccessList[] accessList,uint256 validUntil)AccessList(address address,uint256[] storageKeys)");

	let mut domain_seperator_msg = domain_hash.to_vec();
	domain_seperator_msg.extend_from_slice(keccak256!("Acala EVM")); // name
	domain_seperator_msg.extend_from_slice(keccak256!("1")); // version
	domain_seperator_msg.extend_from_slice(&to_bytes(eth_msg.chain_id)); // chain id
	domain_seperator_msg.extend_from_slice(eth_msg.genesis.as_bytes()); // salt
	let domain_separator = keccak_256(domain_seperator_msg.as_slice());

	let mut tx_msg = tx_type_hash.to_vec();
	match eth_msg.action {
		TransactionAction::Call(to) => {
			tx_msg.extend_from_slice(keccak256!("Call"));
			tx_msg.extend_from_slice(H256::from(to).as_bytes());
		}
		TransactionAction::Create => {
			tx_msg.extend_from_slice(keccak256!("Create"));
			tx_msg.extend_from_slice(H256::default().as_bytes());
		}
	}
	tx_msg.extend_from_slice(&to_bytes(eth_msg.nonce));
	tx_msg.extend_from_slice(&to_bytes(eth_msg.tip));
	tx_msg.extend_from_slice(&keccak_256(eth_msg.input.as_slice()));
	tx_msg.extend_from_slice(&to_bytes(eth_msg.value));
	tx_msg.extend_from_slice(&to_bytes(eth_msg.gas_limit));
	tx_msg.extend_from_slice(&to_bytes(eth_msg.storage_limit));

	let mut access_list: Vec<[u8; 32]> = Vec::new();
	eth_msg.access_list.iter().for_each(|v| {
		let mut access_list_msg = access_list_type_hash.to_vec();
		access_list_msg.extend_from_slice(&to_bytes(v.address.as_bytes()));
		access_list_msg.extend_from_slice(&keccak_256(
			&v.storage_keys.iter().map(|v| v.as_bytes()).collect::<Vec<_>>().concat(),
		));
		access_list.push(keccak_256(access_list_msg.as_slice()));
	});
	tx_msg.extend_from_slice(&keccak_256(&access_list.concat()));
	tx_msg.extend_from_slice(&to_bytes(eth_msg.valid_until));

	let mut msg = b"\x19\x01".to_vec();
	msg.extend_from_slice(&domain_separator);
	msg.extend_from_slice(&keccak_256(tx_msg.as_slice()));

	let msg_hash = keccak_256(msg.as_slice());

	recover_signer(&sig, &msg_hash)
}

fn recover_sign_data(
	eth_msg: &EthereumTransactionMessage,
	ts_fee_per_gas: u128,
	storage_deposit_per_byte: u128,
) -> Option<(u128, u128)> {
	// tx_gas_price = tx_fee_per_gas + block_period << 16 + storage_entry_limit
	// tx_gas_limit = gas_limit + storage_entry_deposit / tx_fee_per_gas * storage_entry_limit
	let block_period = eth_msg.valid_until.saturating_div(30);
	// u16: max value 0xffff * 64 = 4194240 bytes = 4MB
	let storage_entry_limit: u16 = eth_msg.storage_limit.saturating_div(64).try_into().ok()?;
	let storage_entry_deposit = storage_deposit_per_byte.saturating_mul(64);
	let tx_gas_price = ts_fee_per_gas
		.checked_add(Into::<u128>::into(block_period).checked_shl(16)?)?
		.checked_add(storage_entry_limit.into())?;
	// There is a loss of precision here, so the order of calculation must be guaranteed
	// must ensure storage_deposit / tx_fee_per_gas * storage_limit
	let tx_gas_limit = storage_entry_deposit
		.checked_div(ts_fee_per_gas)
		.expect("divisor is non-zero; qed")
		.checked_mul(storage_entry_limit.into())?
		.checked_add(eth_msg.gas_limit.into())?;

	Some((tx_gas_price, tx_gas_limit))
}

#[cfg(test)]
mod tests {
	use super::*;
	use hex_literal::hex;
	use module_evm_utility::ethereum::AccessListItem;
	use sp_core::U256;
	use std::{ops::Add, str::FromStr};

	#[test]
	fn verify_eip712_should_works() {
		let sender = Some(H160::from_str("0x14791697260E4c9A71f18484C9f997B308e59325").unwrap());
		// access_list = vec![]
		let msg = EthereumTransactionMessage {
			chain_id: 595,
			genesis: H256::from_str("0xafb55f3937d1377c23b8f351315b2792f5d2753bb95420c191d2dc70ad7196e8").unwrap(),
			nonce: 0,
			tip: 2,
			gas_price: 0,
			gas_limit: 2100000,
			storage_limit: 20000,
			action: TransactionAction::Create,
			value: 0,
			input: vec![0x01],
			valid_until: 105,
			access_list: vec![],
		};
		let sign = hex!("c30a85ee9218af4e2892c82d65a8a7fbeee75c010973d42cee2e52309449d687056c09cf486a16d58d23b0ebfed63a0276d5fb1a464f645dc7607147a37f7a211c");
		assert_eq!(verify_eip712_signature(msg, sign), sender);

		// access_list.storage_keys = vec![]
		let msg = EthereumTransactionMessage {
			chain_id: 595,
			genesis: H256::from_str("0xafb55f3937d1377c23b8f351315b2792f5d2753bb95420c191d2dc70ad7196e8").unwrap(),
			nonce: 0,
			tip: 2,
			gas_price: 0,
			gas_limit: 2100000,
			storage_limit: 20000,
			action: TransactionAction::Create,
			value: 0,
			input: vec![0x01],
			valid_until: 105,
			access_list: vec![AccessListItem {
				address: hex!("0000000000000000000000000000000000000000").into(),
				storage_keys: vec![],
			}],
		};
		let sign = hex!("a94da7159e29f2a0c9aec08eb62cbb6eefd6ee277960a3c96b183b53201687ce19f1fd9c2cfdace8730fd5249ea11e57701cd0cc20386bbd9d3df5092fe218851c");
		assert_eq!(verify_eip712_signature(msg, sign), sender);

		let msg = EthereumTransactionMessage {
			chain_id: 595,
			genesis: H256::from_str("0xafb55f3937d1377c23b8f351315b2792f5d2753bb95420c191d2dc70ad7196e8").unwrap(),
			nonce: 0,
			tip: 2,
			gas_price: 0,
			gas_limit: 2100000,
			storage_limit: 20000,
			action: TransactionAction::Create,
			value: 0,
			input: vec![0x01],
			valid_until: 105,
			access_list: vec![AccessListItem {
				address: hex!("0000000000000000000000000000000000000000").into(),
				storage_keys: vec![
					H256::from_str("0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef").unwrap(),
					H256::from_str("0x0000000000111111111122222222223333333333444444444455555555556666").unwrap(),
					H256::from_str("0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef").unwrap(),
				],
			}],
		};
		let sign = hex!("dca9701b77bac69e5a88c7f040a6fa0a051f97305619e66e9182bf3416ca2d0e7b730cb732e2f747754f6b9307d78ce611aabb3692ea48314670a6a8c447dc9b1c");
		assert_eq!(verify_eip712_signature(msg.clone(), sign), sender);

		let mut new_msg = msg.clone();
		new_msg.nonce += 1;
		assert_ne!(verify_eip712_signature(new_msg, sign), sender);

		let mut new_msg = msg.clone();
		new_msg.tip += 1;
		assert_ne!(verify_eip712_signature(new_msg, sign), sender);

		let mut new_msg = msg.clone();
		new_msg.gas_limit += 1;
		assert_ne!(verify_eip712_signature(new_msg, sign), sender);

		let mut new_msg = msg.clone();
		new_msg.storage_limit += 1;
		assert_ne!(verify_eip712_signature(new_msg, sign), sender);

		let mut new_msg = msg.clone();
		new_msg.action = TransactionAction::Call(H160::from_str("0x1111111111222222222233333333334444444444").unwrap());
		assert_ne!(verify_eip712_signature(new_msg, sign), sender);

		let mut new_msg = msg.clone();
		new_msg.value += 1;
		assert_ne!(verify_eip712_signature(new_msg, sign), sender);

		let mut new_msg = msg.clone();
		new_msg.input = vec![0x00];
		assert_ne!(verify_eip712_signature(new_msg, sign), sender);

		let mut new_msg = msg.clone();
		new_msg.chain_id += 1;
		assert_ne!(verify_eip712_signature(new_msg, sign), sender);

		let mut new_msg = msg.clone();
		new_msg.genesis = Default::default();
		assert_ne!(verify_eip712_signature(new_msg, sign), sender);

		let mut new_msg = msg.clone();
		new_msg.access_list = vec![AccessListItem {
			address: hex!("bb9bc244d798123fde783fcc1c72d3bb8c189413").into(),
			storage_keys: vec![],
		}];
		assert_ne!(verify_eip712_signature(new_msg, sign), sender);

		let mut new_msg = msg;
		new_msg.valid_until += 1;
		assert_ne!(verify_eip712_signature(new_msg, sign), sender);
	}

	#[test]
	fn verify_eth_should_works() {
		let msg = LegacyTransactionMessage {
			nonce: U256::from(1),
			gas_price: U256::from("0x640000006a"),
			gas_limit: U256::from(21000),
			action: TransactionAction::Call(H160::from_str("0x1111111111222222222233333333334444444444").unwrap()),
			value: U256::from(123123),
			input: vec![],
			chain_id: Some(595),
		};

		let sign = hex!("f84345a6459785986a1b2df711fe02597d70c1393757a243f8f924ea541d2ecb51476de1aa437cd820d59e1d9836e37e643fec711fe419464e637cab592918751c");
		let sender = Some(H160::from_str("0x14791697260E4c9A71f18484C9f997B308e59325").unwrap());

		assert_eq!(recover_signer(&sign, msg.hash().as_fixed_bytes()), sender);

		let mut new_msg = msg.clone();
		new_msg.nonce = new_msg.nonce.add(U256::one());
		assert_ne!(recover_signer(&sign, new_msg.hash().as_fixed_bytes()), sender);

		let mut new_msg = msg.clone();
		new_msg.gas_price = new_msg.gas_price.add(U256::one());
		assert_ne!(recover_signer(&sign, new_msg.hash().as_fixed_bytes()), sender);

		let mut new_msg = msg.clone();
		new_msg.gas_limit = new_msg.gas_limit.add(U256::one());
		assert_ne!(recover_signer(&sign, new_msg.hash().as_fixed_bytes()), sender);

		let mut new_msg = msg.clone();
		new_msg.action = TransactionAction::Create;
		assert_ne!(recover_signer(&sign, new_msg.hash().as_fixed_bytes()), sender);

		let mut new_msg = msg.clone();
		new_msg.value = new_msg.value.add(U256::one());
		assert_ne!(recover_signer(&sign, new_msg.hash().as_fixed_bytes()), sender);

		let mut new_msg = msg.clone();
		new_msg.input = vec![0x00];
		assert_ne!(recover_signer(&sign, new_msg.hash().as_fixed_bytes()), sender);

		let mut new_msg = msg;
		new_msg.chain_id = None;
		assert_ne!(recover_signer(&sign, new_msg.hash().as_fixed_bytes()), sender);
	}

	#[test]
	fn verify_eth_1559_should_works() {
		let msg = EIP1559TransactionMessage {
			chain_id: 595,
			nonce: U256::from(1),
			max_priority_fee_per_gas: U256::from(1),
			max_fee_per_gas: U256::from("0x640000006a"),
			gas_limit: U256::from(21000),
			action: TransactionAction::Call(H160::from_str("0x1111111111222222222233333333334444444444").unwrap()),
			value: U256::from(123123),
			input: vec![],
			access_list: vec![],
		};

		let sign = hex!("e88df53d4d66cb7a4f54ea44a44942b9b7f4fb4951525d416d3f7d24755a1f817734270872b103ac04c59d74f4dacdb8a6eff09a6638bd95dad1fa3eda921d891b");
		let sender = Some(H160::from_str("0x14791697260E4c9A71f18484C9f997B308e59325").unwrap());

		assert_eq!(recover_signer(&sign, msg.hash().as_fixed_bytes()), sender);

		let mut new_msg = msg.clone();
		new_msg.chain_id = new_msg.chain_id.add(1u64);
		assert_ne!(recover_signer(&sign, new_msg.hash().as_fixed_bytes()), sender);

		let mut new_msg = msg.clone();
		new_msg.nonce = new_msg.nonce.add(U256::one());
		assert_ne!(recover_signer(&sign, new_msg.hash().as_fixed_bytes()), sender);

		let mut new_msg = msg.clone();
		new_msg.max_priority_fee_per_gas = new_msg.max_priority_fee_per_gas.add(U256::one());
		assert_ne!(recover_signer(&sign, new_msg.hash().as_fixed_bytes()), sender);

		let mut new_msg = msg.clone();
		new_msg.max_fee_per_gas = new_msg.max_fee_per_gas.add(U256::one());
		assert_ne!(recover_signer(&sign, new_msg.hash().as_fixed_bytes()), sender);

		let mut new_msg = msg.clone();
		new_msg.gas_limit = new_msg.gas_limit.add(U256::one());
		assert_ne!(recover_signer(&sign, new_msg.hash().as_fixed_bytes()), sender);

		let mut new_msg = msg.clone();
		new_msg.action = TransactionAction::Create;
		assert_ne!(recover_signer(&sign, new_msg.hash().as_fixed_bytes()), sender);

		let mut new_msg = msg.clone();
		new_msg.value = new_msg.value.add(U256::one());
		assert_ne!(recover_signer(&sign, new_msg.hash().as_fixed_bytes()), sender);

		let mut new_msg = msg.clone();
		new_msg.input = vec![0x00];
		assert_ne!(recover_signer(&sign, new_msg.hash().as_fixed_bytes()), sender);

		let mut new_msg = msg;
		new_msg.access_list = vec![AccessListItem {
			address: hex!("bb9bc244d798123fde783fcc1c72d3bb8c189413").into(),
			storage_keys: vec![],
		}];
		assert_ne!(recover_signer(&sign, new_msg.hash().as_fixed_bytes()), sender);
	}

	#[test]
	fn recover_sign_data_should_works() {
		let mut msg = EthereumTransactionMessage {
			chain_id: 595,
			genesis: Default::default(),
			nonce: 1,
			tip: 0,
			gas_price: 0,
			gas_limit: 2100000,
			storage_limit: 64000,
			action: TransactionAction::Call(H160::from_str("0x1111111111222222222233333333334444444444").unwrap()),
			value: 0,
			input: vec![],
			access_list: vec![],
			valid_until: 30,
		};

		let ts_fee_per_gas = 200u128.saturating_mul(10u128.saturating_pow(9)) & !0xffff;
		let storage_deposit_per_byte = 100_000_000_000_000u128;

		assert_eq!(
			recover_sign_data(&msg, ts_fee_per_gas, storage_deposit_per_byte),
			Some((200000013288, 34100000))
		);
		msg.valid_until = 3600030;
		assert_eq!(
			recover_sign_data(&msg, ts_fee_per_gas, storage_deposit_per_byte),
			Some((207864333288, 34100000))
		);
		msg.valid_until = u32::MAX;
		assert_eq!(
			recover_sign_data(&msg, ts_fee_per_gas, storage_deposit_per_byte),
			Some((9582499136488, 34100000))
		);

		// check storage_limit max is 0xffff * 64 + 63
		msg.storage_limit = 0xffff * 64 + 64;
		assert_eq!(recover_sign_data(&msg, ts_fee_per_gas, storage_deposit_per_byte), None);

		msg.storage_limit = 0xffff * 64 + 63;
		assert_eq!(
			recover_sign_data(&msg, ts_fee_per_gas, storage_deposit_per_byte),
			Some((9582499201023, 2099220000))
		);

		assert_eq!(
			recover_sign_data(&msg, ts_fee_per_gas, u128::MAX),
			Some((9582499201023, 111502054267125439094838181151820))
		);

		assert_eq!(recover_sign_data(&msg, u128::MAX, storage_deposit_per_byte), None);

		assert_eq!(recover_sign_data(&msg, u128::MAX, u128::MAX), None);
	}
}
