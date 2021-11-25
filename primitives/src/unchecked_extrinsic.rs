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

use crate::{evm::EthereumTransactionMessage, signature::AcalaMultiSignature, Address};
use codec::{Decode, Encode};
use frame_support::{
	traits::ExtrinsicCall,
	weights::{DispatchInfo, GetDispatchInfo},
};
use module_evm_utiltity::ethereum::{LegacyTransactionMessage, TransactionAction};
use module_evm_utiltity_macro::keccak256;
use scale_info::TypeInfo;
use sp_core::{H160, H256, U256};
use sp_io::{crypto::secp256k1_ecdsa_recover, hashing::keccak_256};
use sp_runtime::{
	generic::{CheckedExtrinsic, UncheckedExtrinsic},
	traits::{self, Checkable, Convert, Extrinsic, ExtrinsicMetadata, Member, SignedExtension},
	transaction_validity::{InvalidTransaction, TransactionValidityError},
	AccountId32, RuntimeDebug,
};
use sp_std::{marker::PhantomData, prelude::*};

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(ConvertTx))]
pub struct AcalaUncheckedExtrinsic<Call, Extra: SignedExtension, ConvertTx>(
	pub UncheckedExtrinsic<Address, Call, AcalaMultiSignature, Extra>,
	PhantomData<ConvertTx>,
);

#[cfg(feature = "std")]
impl<Call, Extra, ConvertTx> parity_util_mem::MallocSizeOf for AcalaUncheckedExtrinsic<Call, Extra, ConvertTx>
where
	Extra: SignedExtension,
{
	fn size_of(&self, _ops: &mut parity_util_mem::MallocSizeOfOps) -> usize {
		// Instantiated only in runtime.
		0
	}
}

impl<Call, Extra: SignedExtension, ConvertTx> Extrinsic for AcalaUncheckedExtrinsic<Call, Extra, ConvertTx> {
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

impl<Call, Extra: SignedExtension, ConvertTx> ExtrinsicMetadata for AcalaUncheckedExtrinsic<Call, Extra, ConvertTx> {
	const VERSION: u8 = UncheckedExtrinsic::<Address, Call, AcalaMultiSignature, Extra>::VERSION;
	type SignedExtensions = Extra;
}

impl<Call, Extra: SignedExtension, ConvertTx> ExtrinsicCall for AcalaUncheckedExtrinsic<Call, Extra, ConvertTx> {
	fn call(&self) -> &Self::Call {
		self.0.call()
	}
}

fn to_bytes<T: Into<U256>>(value: T) -> [u8; 32] {
	Into::<[u8; 32]>::into(value.into())
}

impl<Call, Extra, ConvertTx, Lookup> Checkable<Lookup> for AcalaUncheckedExtrinsic<Call, Extra, ConvertTx>
where
	Call: Encode + Member,
	Extra: SignedExtension<AccountId = AccountId32>,
	ConvertTx: Convert<(Call, Extra), Result<EthereumTransactionMessage, InvalidTransaction>>,
	Lookup: traits::Lookup<Source = Address, Target = AccountId32>,
{
	type Checked = CheckedExtrinsic<AccountId32, Call, Extra>;

	fn check(self, lookup: &Lookup) -> Result<Self::Checked, TransactionValidityError> {
		match self.0.signature {
			Some((addr, AcalaMultiSignature::Ethereum(sig), extra)) => {
				let function = self.0.function;
				let eth_msg = ConvertTx::convert((function.clone(), extra.clone()))?;

				if eth_msg.tip != 0 {
					// Not yet supported, require zero tip
					return Err(InvalidTransaction::BadProof.into());
				}

				// we merge storage_limit and valid_until into gas_price
				let gas_price = (eth_msg.storage_limit as u64) << 32 | eth_msg.valid_until as u64;

				let msg = LegacyTransactionMessage {
					nonce: eth_msg.nonce.into(),
					gas_price: gas_price.into(),
					gas_limit: eth_msg.gas_limit.into(),
					action: eth_msg.action,
					value: eth_msg.value.into(),
					input: eth_msg.input,
					chain_id: Some(eth_msg.chain_id),
				};

				let msg_hash = msg.hash(); // TODO: consider rewirte this to use `keccak_256` for hashing because it could be faster

				let signer = recover_signer(&sig, msg_hash.as_fixed_bytes()).ok_or(InvalidTransaction::BadProof)?;

				let acc = lookup.lookup(Address::Address20(signer.into()))?;
				let expected = lookup.lookup(addr)?;

				if acc != expected {
					return Err(InvalidTransaction::BadProof.into());
				}

				Ok(CheckedExtrinsic {
					signed: Some((acc, extra)),
					function,
				})
			}
			Some((addr, AcalaMultiSignature::AcalaEip712(sig), extra)) => {
				let function = self.0.function;
				let eth_msg = ConvertTx::convert((function.clone(), extra.clone()))?;

				let signer = verify_eip712_signature(eth_msg, sig).ok_or(InvalidTransaction::BadProof)?;

				let acc = lookup.lookup(Address::Address20(signer.into()))?;
				let expected = lookup.lookup(addr)?;

				if acc != expected {
					return Err(InvalidTransaction::BadProof.into());
				}

				Ok(CheckedExtrinsic {
					signed: Some((acc, extra)),
					function,
				})
			}
			_ => self.0.check(lookup),
		}
	}
}

impl<Call, Extra, ConvertTx> GetDispatchInfo for AcalaUncheckedExtrinsic<Call, Extra, ConvertTx>
where
	Call: GetDispatchInfo,
	Extra: SignedExtension,
{
	fn get_dispatch_info(&self) -> DispatchInfo {
		self.0.get_dispatch_info()
	}
}

#[cfg(feature = "std")]
impl<Call: Encode, Extra: SignedExtension, ConvertTx> serde::Serialize
	for AcalaUncheckedExtrinsic<Call, Extra, ConvertTx>
{
	fn serialize<S>(&self, seq: S) -> Result<S::Ok, S::Error>
	where
		S: ::serde::Serializer,
	{
		self.0.serialize(seq)
	}
}

#[cfg(feature = "std")]
impl<'a, Call: Decode, Extra: SignedExtension, ConvertTx> serde::Deserialize<'a>
	for AcalaUncheckedExtrinsic<Call, Extra, ConvertTx>
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
	let tx_type_hash = keccak256!("Transaction(string action,address to,uint256 nonce,uint256 tip,bytes data,uint256 value,uint256 gasLimit,uint256 storageLimit,uint256 validUntil)");

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
	tx_msg.extend_from_slice(&to_bytes(eth_msg.valid_until));

	let mut msg = b"\x19\x01".to_vec();
	msg.extend_from_slice(&domain_separator);
	msg.extend_from_slice(&keccak_256(tx_msg.as_slice()));

	let msg_hash = keccak_256(msg.as_slice());

	recover_signer(&sig, &msg_hash)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::{ops::Add, str::FromStr};

	#[test]
	fn verify_eip712_should_works() {
		let msg = EthereumTransactionMessage {
			nonce: 1,
			tip: 2,
			gas_limit: 222,
			storage_limit: 333,
			action: TransactionAction::Call(H160::from_str("0x1111111111222222222233333333334444444444").unwrap()),
			value: 111,
			input: vec![],
			chain_id: 595,
			genesis: H256::from_str("0xc3751fc073ec83e6aa13e2be395d21b05dce0692618a129324261c80ede07d4c").unwrap(),
			valid_until: 444,
		};
		let sign = hex_literal::hex!("acb56f12b407bd0bc8f7abefe2e2585affe28009abcb6980aa33aecb815c56b324ab60a41eff339a88631c4b0e5183427be1fcfde3c05fb9b6c71a691e977c4a1b");
		let sender = Some(H160::from_str("0x14791697260E4c9A71f18484C9f997B308e59325").unwrap());

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
		new_msg.action = TransactionAction::Create;
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

		let sign = hex_literal::hex!("f84345a6459785986a1b2df711fe02597d70c1393757a243f8f924ea541d2ecb51476de1aa437cd820d59e1d9836e37e643fec711fe419464e637cab592918751c");
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
}
