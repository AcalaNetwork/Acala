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

use codec::{Compact, Decode, Encode, EncodeLike, Error, Input};
use ethereum::TransactionV2;
use sp_runtime::{generic::UncheckedExtrinsic, traits::SignedExtension, RuntimeDebug};
use sp_std::prelude::*;

const MAX_TX_LENGTH: usize = 5 * 1024 * 1024; // max block length is 5MB

#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub enum AcalaUncheckedExtrinsic<Address, Call, Signature, Extra: SignedExtension> {
	Substrate(UncheckedExtrinsic<Address, Call, Signature, Extra>),
	Ethereum(TransactionV2),
}

impl<Address, Call, Signature, Extra> Decode for AcalaUncheckedExtrinsic<Address, Call, Signature, Extra>
where
	Address: Decode,
	Signature: Decode,
	Call: Decode,
	Extra: SignedExtension,
{
	fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
		// Min size for Substrate tx is 4 bytes: (length_prefix, tx_version, call_module_index,
		// call_method_index) Min size for Ethereum tx is about 75 bytes
		let mut first_4_bytes = [0u8; 4];
		input.read(&mut first_4_bytes)?;

		let slice = &mut &first_4_bytes[..];
		let sub_len = Compact::<u32>::decode(slice).unwrap_or_else(|_| 0.into()).0 as usize;
		let sub_len = sub_len + 4 - slice.len(); // add length for prefix

		let rlp_len = rlp::PayloadInfo::from(&first_4_bytes)
			.map_err(|_| Error::from("Invalid RLP length"))?
			.total();

		let sub_len = if sub_len >= MAX_TX_LENGTH || sub_len < 4 {
			None
		} else {
			Some(sub_len)
		};
		let rlp_len = if rlp_len >= MAX_TX_LENGTH || rlp_len < 4 {
			None
		} else {
			Some(rlp_len)
		};

		let max_len = sub_len.unwrap_or_default().max(rlp_len.unwrap_or_default());

		if max_len < 4 {
			return Err(Error::from("Invalid data length"));
		}

		let mut payload = vec![0u8; max_len];
		payload[0..4].copy_from_slice(&first_4_bytes);
		let min_len = sub_len.unwrap_or(MAX_TX_LENGTH).min(rlp_len.unwrap_or(MAX_TX_LENGTH));
		input.read(&mut payload[4..min_len])?;

		// try the smaller one first and than the larger one
		if rlp_len < sub_len {
			if let Some(rlp_len) = rlp_len {
				let utx = rlp::decode::<TransactionV2>(&payload[..rlp_len]);
				if let Ok(utx) = utx {
					return Ok(AcalaUncheckedExtrinsic::Ethereum(utx));
				}
			}
			input.read(&mut payload[min_len..])?;
			let utx = UncheckedExtrinsic::decode(&mut &payload[..])?;
			return Ok(AcalaUncheckedExtrinsic::Substrate(utx));
		} else {
			if let Some(sub_len) = sub_len {
				let utx = UncheckedExtrinsic::decode(&mut &payload[..sub_len]);
				if let Ok(utx) = utx {
					return Ok(AcalaUncheckedExtrinsic::Substrate(utx));
				}
			}
			input.read(&mut payload[min_len..])?;
			let utx = rlp::decode::<TransactionV2>(&payload).map_err(|_| Error::from("Invalid RLP length"))?;
			return Ok(AcalaUncheckedExtrinsic::Ethereum(utx));
		}
	}
}

impl<Address, Call, Signature, Extra> Encode for AcalaUncheckedExtrinsic<Address, Call, Signature, Extra>
where
	Address: Encode,
	Signature: Encode,
	Call: Encode,
	Extra: SignedExtension,
{
	fn encode(&self) -> Vec<u8> {
		match self {
			AcalaUncheckedExtrinsic::Substrate(tx) => tx.encode(),
			AcalaUncheckedExtrinsic::Ethereum(tx) => rlp::encode(tx).to_vec(),
		}
	}
}

impl<Address, Call, Signature, Extra> EncodeLike for AcalaUncheckedExtrinsic<Address, Call, Signature, Extra>
where
	Address: Encode,
	Signature: Encode,
	Call: Encode,
	Extra: SignedExtension,
{
}

#[cfg(feature = "std")]
impl<Address: Encode, Signature: Encode, Call: Encode, Extra: SignedExtension> serde::Serialize
	for AcalaUncheckedExtrinsic<Address, Call, Signature, Extra>
{
	fn serialize<S>(&self, seq: S) -> Result<S::Ok, S::Error>
	where
		S: ::serde::Serializer,
	{
		self.using_encoded(|bytes| seq.serialize_bytes(bytes))
	}
}

#[cfg(feature = "std")]
impl<'a, Address: Decode, Signature: Decode, Call: Decode, Extra: SignedExtension> serde::Deserialize<'a>
	for AcalaUncheckedExtrinsic<Address, Call, Signature, Extra>
{
	fn deserialize<D>(de: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'a>,
	{
		let r = sp_core::bytes::deserialize(de)?;
		Decode::decode(&mut &r[..]).map_err(|e| serde::de::Error::custom(format!("Decode error: {}", e)))
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

	#[test]
	fn test_decode_substrate_tx() {
		let data = UncheckedExtrinsic::<u64, Vec<u8>, u64, ()>::new_signed(vec![], 456, 789, ());
		let encoded = data.encode();
		let decoded = AcalaUncheckedExtrinsic::<u64, Vec<u8>, u64, ()>::decode(&mut &encoded[..]).unwrap();
		assert_eq!(decoded, AcalaUncheckedExtrinsic::Substrate(data));

		let encoded2 = decoded.encode();
		assert_eq!(encoded, encoded2);
	}

	#[test]
	fn test_decode_substrate_tx_big() {
		let data = UncheckedExtrinsic::<u64, Vec<u8>, u64, ()>::new_signed(vec![123; 1024 * 1024 * 4], 456, 789, ());
		let encoded = data.encode();
		let decoded = AcalaUncheckedExtrinsic::<u64, Vec<u8>, u64, ()>::decode(&mut &encoded[..]).unwrap();
		assert_eq!(decoded, AcalaUncheckedExtrinsic::Substrate(data));

		let encoded2 = decoded.encode();
		assert_eq!(encoded, encoded2);
	}

	#[test]
	fn test_decode_unsigned_substrate_tx() {
		let data = UncheckedExtrinsic::<u64, Vec<u8>, u64, ()>::new_unsigned(vec![1, 2]);
		let encoded = data.encode();
		let decoded = AcalaUncheckedExtrinsic::<u64, Vec<u8>, u64, ()>::decode(&mut &encoded[..]).unwrap();
		assert_eq!(decoded, AcalaUncheckedExtrinsic::Substrate(data));

		let encoded2 = decoded.encode();
		assert_eq!(encoded, encoded2);
	}

	#[test]
	fn test_decode_unsigned_substrate_tx_big() {
		let data = UncheckedExtrinsic::<u64, Vec<u8>, u64, ()>::new_unsigned(vec![123; 1024 * 1024 * 4]);
		let encoded = data.encode();
		let decoded = AcalaUncheckedExtrinsic::<u64, Vec<u8>, u64, ()>::decode(&mut &encoded[..]).unwrap();
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
		let encoded = rlp::encode(&data);
		let decoded = AcalaUncheckedExtrinsic::<u64, Vec<u8>, u64, ()>::decode(&mut &encoded[..]).unwrap();
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
		let encoded = rlp::encode(&data);
		let decoded = AcalaUncheckedExtrinsic::<u64, Vec<u8>, u64, ()>::decode(&mut &encoded[..]).unwrap();
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
		let encoded = rlp::encode(&data);
		let decoded = AcalaUncheckedExtrinsic::<u64, Vec<u8>, u64, ()>::decode(&mut &encoded[..]).unwrap();
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
		let encoded = rlp::encode(&data);
		let decoded = AcalaUncheckedExtrinsic::<u64, Vec<u8>, u64, ()>::decode(&mut &encoded[..]).unwrap();
		assert_eq!(decoded, AcalaUncheckedExtrinsic::Ethereum(data));

		let encoded2 = decoded.encode();
		assert_eq!(encoded, encoded2);
	}
}
