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

use crate::Address;
use codec::{Compact, Decode, Encode, EncodeLike, Error, Input};
use ethereum::{EIP1559TransactionMessage, EIP2930TransactionMessage, LegacyTransactionMessage, TransactionV2};
use frame_support::{
	traits::ExtrinsicCall,
	weights::{DispatchInfo, GetDispatchInfo},
};
use sha3::{Digest, Keccak256};
use sp_core::{H160, H256};
use sp_runtime::{
	generic::{CheckedExtrinsic, UncheckedExtrinsic},
	traits::{
		self, Checkable, Convert, Extrinsic, ExtrinsicMetadata, IdentifyAccount, MaybeDisplay, Member, SignedExtension,
	},
	transaction_validity::{InvalidTransaction, TransactionValidityError},
	RuntimeDebug,
};
use sp_std::prelude::*;

/// Unchecked extrinsic that support boths Substrate format and Ethereum format
/// NOTE: a SCALE codec style length prefix is added to the Ethereum format in additional to the RLP
/// length prefix
#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub enum AcalaUncheckedExtrinsic<Call, Signature, Extra: SignedExtension, ConvertTx> {
	Substrate(UncheckedExtrinsic<Address, Call, Signature, Extra>),
	Ethereum(TransactionV2),
	_Phantom(sp_std::marker::PhantomData<ConvertTx>),
}

#[cfg(feature = "std")]
impl<Call, Signature, Extra, ConvertTx> parity_util_mem::MallocSizeOf
	for AcalaUncheckedExtrinsic<Call, Signature, Extra, ConvertTx>
where
	Extra: SignedExtension,
{
	fn size_of(&self, _ops: &mut parity_util_mem::MallocSizeOfOps) -> usize {
		// Instantiated only in runtime.
		0
	}
}

impl<Call, Signature, Extra: SignedExtension, ConvertTx> Extrinsic
	for AcalaUncheckedExtrinsic<Call, Signature, Extra, ConvertTx>
{
	type Call = Call;

	type SignaturePayload = (Address, Signature, Extra);

	fn is_signed(&self) -> Option<bool> {
		match self {
			Self::Substrate(tx) => tx.is_signed(),
			Self::Ethereum(_) => Some(true),
			Self::_Phantom(_) => unreachable!(),
		}
	}

	fn new(function: Call, signed_data: Option<Self::SignaturePayload>) -> Option<Self> {
		Some(if let Some((address, signature, extra)) = signed_data {
			Self::Substrate(UncheckedExtrinsic::new_signed(function, address, signature, extra))
		} else {
			Self::Substrate(UncheckedExtrinsic::new_unsigned(function))
		})
	}
}

impl<Call, Signature, Extra: SignedExtension, ConvertTx> ExtrinsicMetadata
	for AcalaUncheckedExtrinsic<Call, Signature, Extra, ConvertTx>
{
	const VERSION: u8 = UncheckedExtrinsic::<Address, Call, Signature, Extra>::VERSION;
	type SignedExtensions = Extra;
}

impl<Call, Signature, Extra: SignedExtension, ConvertTx> ExtrinsicCall
	for AcalaUncheckedExtrinsic<Call, Signature, Extra, ConvertTx>
{
	fn call(&self) -> &Self::Call {
		match self {
			Self::Substrate(tx) => tx.call(),
			Self::Ethereum(_) => todo!(),
			Self::_Phantom(_) => unreachable!(),
		}
	}
}

impl<AccountId, Call, Signature, Extra, ConvertTx, Lookup> Checkable<Lookup>
	for AcalaUncheckedExtrinsic<Call, Signature, Extra, ConvertTx>
where
	Call: Encode + Member,
	Signature: Member + traits::Verify,
	<Signature as traits::Verify>::Signer: IdentifyAccount<AccountId = AccountId>,
	Extra: SignedExtension<AccountId = AccountId>,
	ConvertTx: Convert<TransactionV2, Result<(Call, Extra), InvalidTransaction>>,
	AccountId: Member + MaybeDisplay,
	Lookup: traits::Lookup<Source = Address, Target = AccountId>,
{
	type Checked = CheckedExtrinsic<AccountId, Call, Extra>;

	fn check(self, lookup: &Lookup) -> Result<Self::Checked, TransactionValidityError> {
		match self {
			Self::Substrate(tx) => tx.check(lookup),
			Self::Ethereum(tx) => {
				let mut sig = [0u8; 65];

				let msg = match tx.clone() {
					TransactionV2::Legacy(tx) => {
						sig[0..32].copy_from_slice(&tx.signature.r()[..]);
						sig[32..64].copy_from_slice(&tx.signature.s()[..]);
						sig[64] = tx.signature.standard_v();

						LegacyTransactionMessage::from(tx).hash()
					}
					TransactionV2::EIP2930(tx) => {
						sig[0..32].copy_from_slice(&tx.r[..]);
						sig[32..64].copy_from_slice(&tx.s[..]);
						sig[64] = if tx.odd_y_parity { 1 } else { 0 };

						EIP2930TransactionMessage::from(tx).hash()
					}
					TransactionV2::EIP1559(tx) => {
						sig[0..32].copy_from_slice(&tx.r[..]);
						sig[32..64].copy_from_slice(&tx.s[..]);
						sig[64] = if tx.odd_y_parity { 1 } else { 0 };

						EIP1559TransactionMessage::from(tx).hash()
					}
				};

				let pubkey = sp_io::crypto::secp256k1_ecdsa_recover(&sig, msg.as_fixed_bytes())
					.map_err(|_| InvalidTransaction::BadProof)?;
				let signer = H160::from(H256::from_slice(Keccak256::digest(&pubkey).as_slice()));

				let acc = lookup.lookup(Address::Address20(signer.into()))?;

				let (function, extra) = ConvertTx::convert(tx)?;

				Ok(CheckedExtrinsic {
					signed: Some((acc, extra)),
					function,
				})
			}
			Self::_Phantom(_) => unreachable!(),
		}
	}
}

impl<Call, Signature, Extra, ConvertTx> GetDispatchInfo for AcalaUncheckedExtrinsic<Call, Signature, Extra, ConvertTx>
where
	Call: GetDispatchInfo,
	Extra: SignedExtension,
{
	fn get_dispatch_info(&self) -> DispatchInfo {
		match self {
			Self::Substrate(tx) => tx.get_dispatch_info(),
			Self::Ethereum(_) => todo!(),
			Self::_Phantom(_) => unreachable!(),
		}
	}
}

struct InputReplayer<'a, I: Input> {
	pub input: &'a mut I,
	pub buffer: Vec<u8>,
}

impl<'a, I: Input> Input for InputReplayer<'a, I> {
	fn remaining_len(&mut self) -> Result<Option<usize>, Error> {
		self.input.remaining_len()
	}

	fn read(&mut self, into: &mut [u8]) -> Result<(), Error> {
		self.input.read(into)?;
		self.buffer.extend_from_slice(into);
		Ok(())
	}

	fn read_byte(&mut self) -> Result<u8, Error> {
		let byte = self.input.read_byte()?;
		self.buffer.push(byte);
		Ok(byte)
	}

	fn descend_ref(&mut self) -> Result<(), Error> {
		self.input.descend_ref()
	}

	fn ascend_ref(&mut self) {
		self.input.ascend_ref()
	}
}

impl<Call, Signature, Extra, ConvertTx> Decode for AcalaUncheckedExtrinsic<Call, Signature, Extra, ConvertTx>
where
	Address: Decode,
	Signature: Decode,
	Call: Decode,
	Extra: SignedExtension,
{
	fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
		let capacity = input.remaining_len().unwrap_or_default().unwrap_or(1024);

		let mut replayer = InputReplayer {
			input,
			buffer: Vec::with_capacity(capacity),
		};

		let utx = UncheckedExtrinsic::decode(&mut replayer);
		if let Ok(utx) = utx {
			return Ok(AcalaUncheckedExtrinsic::Substrate(utx));
		}

		let mut buffer = replayer.buffer;
		let input = replayer.input;

		// read the length prefix
		let len: Compact<u32> = Decode::decode(&mut &buffer[..])?;
		let len_len = len.encode().len();

		let old_len = buffer.len();
		buffer.resize(len.0 as usize + len_len, 0);
		input.read(&mut buffer[old_len..])?;

		let utx = rlp::decode::<TransactionV2>(&buffer[len_len..]).map_err(|_| Error::from("Invalid extrinsic"))?;
		Ok(AcalaUncheckedExtrinsic::Ethereum(utx))
	}
}

impl<Call, Signature, Extra, ConvertTx> Encode for AcalaUncheckedExtrinsic<Call, Signature, Extra, ConvertTx>
where
	Signature: Encode,
	Call: Encode,
	Extra: SignedExtension,
{
	fn encode(&self) -> Vec<u8> {
		match self {
			AcalaUncheckedExtrinsic::Substrate(tx) => tx.encode(),
			AcalaUncheckedExtrinsic::Ethereum(tx) => rlp::encode(tx).encode(),
			Self::_Phantom(_) => unreachable!(),
		}
	}
}

impl<Call, Signature, Extra, ConvertTx> EncodeLike for AcalaUncheckedExtrinsic<Call, Signature, Extra, ConvertTx>
where
	Signature: Encode,
	Call: Encode,
	Extra: SignedExtension,
{
}

#[cfg(feature = "std")]
impl<Signature: Encode, Call: Encode, Extra: SignedExtension, ConvertTx> serde::Serialize
	for AcalaUncheckedExtrinsic<Call, Signature, Extra, ConvertTx>
{
	fn serialize<S>(&self, seq: S) -> Result<S::Ok, S::Error>
	where
		S: ::serde::Serializer,
	{
		self.using_encoded(|bytes| seq.serialize_bytes(bytes))
	}
}

#[cfg(feature = "std")]
impl<'a, Signature: Decode, Call: Decode, Extra: SignedExtension, ConvertTx> serde::Deserialize<'a>
	for AcalaUncheckedExtrinsic<Call, Signature, Extra, ConvertTx>
{
	fn deserialize<D>(de: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'a>,
	{
		let r = sp_core::bytes::deserialize(de)?;
		Decode::decode(&mut &r[..]).map_err(|e| serde::de::Error::custom(format!("Decode error: {}", e)))
	}
}

impl<Call, Signature, Extra: SignedExtension, ConvertTx> Into<UncheckedExtrinsic<Address, Call, Signature, Extra>>
	for AcalaUncheckedExtrinsic<Call, Signature, Extra, ConvertTx>
{
	fn into(self) -> UncheckedExtrinsic<Address, Call, Signature, Extra> {
		match self {
			AcalaUncheckedExtrinsic::Substrate(tx) => tx,
			AcalaUncheckedExtrinsic::Ethereum(_tx) => todo!(),
			Self::_Phantom(_) => unreachable!(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use ethereum::{
		AccessListItem, EIP1559Transaction, EIP2930Transaction, LegacyTransaction, TransactionAction,
		TransactionSignature,
	};
	use hex_literal::hex;
	use sp_core::U256;

	#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
	pub struct DummyConvert;

	impl<A, B> Convert<A, B> for DummyConvert {
		fn convert(_: A) -> B {
			unimplemented!()
		}
	}

	#[test]
	fn test_decode_substrate_tx() {
		let data = UncheckedExtrinsic::<Address, Vec<u8>, u64, ()>::new_signed(vec![], Address::Index(456), 789, ());
		let encoded = data.encode();
		let decoded = AcalaUncheckedExtrinsic::<Vec<u8>, u64, (), DummyConvert>::decode(&mut &encoded[..]).unwrap();
		assert_eq!(decoded, AcalaUncheckedExtrinsic::Substrate(data));

		let encoded2 = decoded.encode();
		assert_eq!(encoded, encoded2);
	}

	#[test]
	fn test_decode_substrate_tx_big() {
		let data = UncheckedExtrinsic::<Address, Vec<u8>, u64, ()>::new_signed(
			vec![123; 1024 * 1024 * 4],
			Address::Index(456),
			789,
			(),
		);
		let encoded = data.encode();
		let decoded = AcalaUncheckedExtrinsic::<Vec<u8>, u64, (), DummyConvert>::decode(&mut &encoded[..]).unwrap();
		assert_eq!(decoded, AcalaUncheckedExtrinsic::Substrate(data));

		let encoded2 = decoded.encode();
		assert_eq!(encoded, encoded2);
	}

	#[test]
	fn test_decode_unsigned_substrate_tx() {
		let data = UncheckedExtrinsic::<Address, Vec<u8>, u64, ()>::new_unsigned(vec![1, 2]);
		let encoded = data.encode();
		let decoded = AcalaUncheckedExtrinsic::<Vec<u8>, u64, (), DummyConvert>::decode(&mut &encoded[..]).unwrap();
		assert_eq!(decoded, AcalaUncheckedExtrinsic::Substrate(data));

		let encoded2 = decoded.encode();
		assert_eq!(encoded, encoded2);
	}

	#[test]
	fn test_decode_unsigned_substrate_tx_big() {
		let data = UncheckedExtrinsic::<Address, Vec<u8>, u64, ()>::new_unsigned(vec![123; 1024 * 1024 * 4]);
		let encoded = data.encode();
		let decoded = AcalaUncheckedExtrinsic::<Vec<u8>, u64, (), DummyConvert>::decode(&mut &encoded[..]).unwrap();
		assert_eq!(decoded, AcalaUncheckedExtrinsic::Substrate(data));

		let encoded2 = decoded.encode();
		assert_eq!(encoded, encoded2);
	}

	#[test]
	fn test_decode_legacy_ethereum_tx() {
		let data = TransactionV2::Legacy(LegacyTransaction {
			nonce: U256::from(123),
			gas_price: U256::from(456),
			gas_limit: U256::from(789),
			action: TransactionAction::Create,
			value: U256::from(912),
			input: vec![],
			signature: TransactionSignature::new(
				38,
				hex!("be67e0a07db67da8d446f76add590e54b6e92cb6b8f9835aeb67540579a27717").into(),
				hex!("2d690516512020171c1ec870f6ff45398cc8609250326be89915fb538e7bd718").into(),
			)
			.unwrap(),
		});
		let encoded = rlp::encode(&data).encode();
		let decoded = AcalaUncheckedExtrinsic::<Vec<u8>, u64, (), DummyConvert>::decode(&mut &encoded[..]).unwrap();
		assert_eq!(decoded, AcalaUncheckedExtrinsic::Ethereum(data));

		let encoded2 = decoded.encode();
		assert_eq!(encoded, encoded2);
	}

	#[test]
	fn test_decode_legacy_ethereum_tx_big() {
		let data = TransactionV2::Legacy(LegacyTransaction {
			nonce: U256::from(123),
			gas_price: U256::from(456),
			gas_limit: U256::from(789),
			action: TransactionAction::Create,
			value: U256::from(912),
			input: vec![123; 1024 * 1024 * 4],
			signature: TransactionSignature::new(
				38,
				hex!("be67e0a07db67da8d446f76add590e54b6e92cb6b8f9835aeb67540579a27717").into(),
				hex!("2d690516512020171c1ec870f6ff45398cc8609250326be89915fb538e7bd718").into(),
			)
			.unwrap(),
		});
		let encoded = rlp::encode(&data).encode();
		let decoded = AcalaUncheckedExtrinsic::<Vec<u8>, u64, (), DummyConvert>::decode(&mut &encoded[..]).unwrap();
		assert_eq!(decoded, AcalaUncheckedExtrinsic::Ethereum(data));

		let encoded2 = decoded.encode();
		assert_eq!(encoded, encoded2);
	}

	#[test]
	fn test_decode_eip2930_ethereum_tx() {
		let data = TransactionV2::EIP2930(EIP2930Transaction {
			chain_id: 5,
			nonce: 7.into(),
			gas_price: 30_000_000_000_u64.into(),
			gas_limit: 5_748_100_u64.into(),
			action: TransactionAction::Call(hex!("811a752c8cd697e3cb27279c330ed1ada745a8d7").into()),
			value: U256::from(2) * 1_000_000_000 * 1_000_000_000,
			input: hex!("6ebaf477f83e051589c1188bcc6ddccd").into(),
			access_list: vec![
				AccessListItem {
					address: hex!("de0b295669a9fd93d5f28d9ec85e40f4cb697bae").into(),
					slots: vec![
						hex!("0000000000000000000000000000000000000000000000000000000000000003").into(),
						hex!("0000000000000000000000000000000000000000000000000000000000000007").into(),
					],
				},
				AccessListItem {
					address: hex!("bb9bc244d798123fde783fcc1c72d3bb8c189413").into(),
					slots: vec![],
				},
			],
			odd_y_parity: false,
			r: hex!("36b241b061a36a32ab7fe86c7aa9eb592dd59018cd0443adc0903590c16b02b0").into(),
			s: hex!("5edcc541b4741c5cc6dd347c5ed9577ef293a62787b4510465fadbfe39ee4094").into(),
		});
		let encoded = rlp::encode(&data).encode();
		let decoded = AcalaUncheckedExtrinsic::<Vec<u8>, u64, (), DummyConvert>::decode(&mut &encoded[..]).unwrap();
		assert_eq!(decoded, AcalaUncheckedExtrinsic::Ethereum(data));

		let encoded2 = decoded.encode();
		assert_eq!(encoded, encoded2);
	}

	#[test]
	fn test_decode_eip1559_ethereum_tx() {
		let data = TransactionV2::EIP1559(EIP1559Transaction {
			chain_id: 5,
			nonce: 7.into(),
			max_priority_fee_per_gas: 10_000_000_000_u64.into(),
			max_fee_per_gas: 30_000_000_000_u64.into(),
			gas_limit: 5_748_100_u64.into(),
			action: TransactionAction::Call(hex!("811a752c8cd697e3cb27279c330ed1ada745a8d7").into()),
			value: U256::from(2) * 1_000_000_000 * 1_000_000_000,
			input: hex!("6ebaf477f83e051589c1188bcc6ddccd").into(),
			access_list: vec![
				AccessListItem {
					address: hex!("de0b295669a9fd93d5f28d9ec85e40f4cb697bae").into(),
					slots: vec![
						hex!("0000000000000000000000000000000000000000000000000000000000000003").into(),
						hex!("0000000000000000000000000000000000000000000000000000000000000007").into(),
					],
				},
				AccessListItem {
					address: hex!("bb9bc244d798123fde783fcc1c72d3bb8c189413").into(),
					slots: vec![],
				},
			],
			odd_y_parity: false,
			r: hex!("36b241b061a36a32ab7fe86c7aa9eb592dd59018cd0443adc0903590c16b02b0").into(),
			s: hex!("5edcc541b4741c5cc6dd347c5ed9577ef293a62787b4510465fadbfe39ee4094").into(),
		});
		let encoded = rlp::encode(&data).encode();
		let decoded = AcalaUncheckedExtrinsic::<Vec<u8>, u64, (), DummyConvert>::decode(&mut &encoded[..]).unwrap();
		assert_eq!(decoded, AcalaUncheckedExtrinsic::Ethereum(data));

		let encoded2 = decoded.encode();
		assert_eq!(encoded, encoded2);
	}
}
