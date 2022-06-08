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

use crate::setup::*;

#[test]
fn currency_id_encode_decode() {
	let erc20 = CurrencyId::Erc20(H160::from_low_u64_be(1));
	let encode_key = erc20.encode();
	let key = &encode_key[..];
	let currency_id1 = CurrencyId::decode(&mut &*encode_key).ok().unwrap();
	let currency_id2 = CurrencyId::decode(&mut &*key).ok().unwrap();
	assert_eq!(currency_id1, currency_id2);
	assert_eq!(currency_id1, erc20);
}

#[test]
fn currency_id_convert() {
	ExtBuilder::default().build().execute_with(|| {
		let id: u32 = ParachainInfo::get().into();

		assert_eq!(
			CurrencyIdConvert::convert(RELAY_CHAIN_CURRENCY),
			Some(MultiLocation::parent())
		);

		assert_eq!(
			CurrencyIdConvert::convert(NATIVE_CURRENCY),
			Some(MultiLocation::sibling_parachain_general_key(
				id,
				NATIVE_CURRENCY.encode()
			))
		);
		assert_eq!(
			CurrencyIdConvert::convert(USD_CURRENCY),
			Some(MultiLocation::sibling_parachain_general_key(id, USD_CURRENCY.encode()))
		);
		assert_eq!(
			CurrencyIdConvert::convert(LIQUID_CURRENCY),
			Some(MultiLocation::sibling_parachain_general_key(
				id,
				LIQUID_CURRENCY.encode()
			))
		);
		assert_eq!(
			CurrencyIdConvert::convert(MultiLocation::parent()),
			Some(RELAY_CHAIN_CURRENCY)
		);
		assert_eq!(
			CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(
				id,
				NATIVE_CURRENCY.encode()
			)),
			Some(NATIVE_CURRENCY)
		);
		assert_eq!(
			CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(id, USD_CURRENCY.encode())),
			Some(USD_CURRENCY)
		);
		assert_eq!(
			CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(
				id,
				LIQUID_CURRENCY.encode()
			)),
			Some(LIQUID_CURRENCY)
		);

		#[cfg(feature = "with-mandala-runtime")]
		{
			assert_eq!(CurrencyIdConvert::convert(KAR), None);
			assert_eq!(CurrencyIdConvert::convert(KUSD), None);
			assert_eq!(CurrencyIdConvert::convert(KSM), None);
			assert_eq!(CurrencyIdConvert::convert(LKSM), None);

			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(id, RENBTC.encode())),
				Some(RENBTC)
			);
			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(id, KAR.encode())),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(id, KUSD.encode())),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(id, KSM.encode())),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(id, KSM.encode())),
				None
			);

			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(id + 1, RENBTC.encode())),
				None
			);

			let native_currency: MultiAsset = (
				MultiLocation::sibling_parachain_general_key(id, NATIVE_CURRENCY.encode()),
				1,
			)
				.into();
			assert_eq!(CurrencyIdConvert::convert(native_currency), Some(NATIVE_CURRENCY));
		}

		#[cfg(feature = "with-karura-runtime")]
		{
			assert_eq!(CurrencyIdConvert::convert(ACA), None);
			assert_eq!(CurrencyIdConvert::convert(AUSD), None);
			assert_eq!(CurrencyIdConvert::convert(DOT), None);
			assert_eq!(CurrencyIdConvert::convert(LDOT), None);

			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(id, ACA.encode())),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(id, AUSD.encode())),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(id, DOT.encode())),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(id, LDOT.encode())),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(
					parachains::bifrost::ID,
					parachains::bifrost::BNC_KEY.to_vec()
				)),
				Some(BNC)
			);
			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(
					parachains::bifrost::ID,
					parachains::bifrost::VSKSM_KEY.to_vec()
				)),
				Some(VSKSM)
			);

			assert_eq!(
				CurrencyIdConvert::convert(BNC),
				Some(MultiLocation::sibling_parachain_general_key(
					parachains::bifrost::ID,
					parachains::bifrost::BNC_KEY.to_vec()
				))
			);
			assert_eq!(
				CurrencyIdConvert::convert(VSKSM),
				Some(MultiLocation::sibling_parachain_general_key(
					parachains::bifrost::ID,
					parachains::bifrost::VSKSM_KEY.to_vec()
				))
			);

			let native_currency: MultiAsset = (
				MultiLocation::sibling_parachain_general_key(id, NATIVE_CURRENCY.encode()),
				1,
			)
				.into();
			assert_eq!(CurrencyIdConvert::convert(native_currency), Some(NATIVE_CURRENCY));
		}

		#[cfg(feature = "with-acala-runtime")]
		{
			assert_eq!(CurrencyIdConvert::convert(KAR), None);
			assert_eq!(CurrencyIdConvert::convert(KUSD), None);
			assert_eq!(CurrencyIdConvert::convert(KSM), None);
			assert_eq!(CurrencyIdConvert::convert(LKSM), None);

			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(id, RENBTC.encode())),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(id, KAR.encode())),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(id, KUSD.encode())),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(id, KSM.encode())),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(MultiLocation::sibling_parachain_general_key(id, LKSM.encode())),
				None
			);

			let native_currency: MultiAsset = (
				MultiLocation::sibling_parachain_general_key(id, NATIVE_CURRENCY.encode()),
				1,
			)
				.into();
			assert_eq!(CurrencyIdConvert::convert(native_currency), Some(NATIVE_CURRENCY));
		}
	});
}

#[test]
fn parachain_subaccounts_are_unique() {
	ExtBuilder::default().build().execute_with(|| {
		let parachain: AccountId = ParachainInfo::parachain_id().into_account_truncating();
		assert_eq!(
			parachain,
			hex_literal::hex!["70617261d0070000000000000000000000000000000000000000000000000000"].into()
		);

		assert_eq!(
			create_x2_parachain_multilocation(0),
			MultiLocation::new(
				1,
				X1(Junction::AccountId32 {
					network: NetworkId::Any,
					id: hex_literal::hex!["d7b8926b326dd349355a9a7cca6606c1e0eb6fd2b506066b518c7155ff0d8297"].into(),
				})
			),
		);
		assert_eq!(
			create_x2_parachain_multilocation(1),
			MultiLocation::new(
				1,
				X1(Junction::AccountId32 {
					network: NetworkId::Any,
					id: hex_literal::hex!["74d37d762e06c6841a5dad64463a9afe0684f7e45245f6a7296ca613cca74669"].into(),
				})
			),
		);
	});
}

#[cfg(feature = "with-mandala-runtime")]
mod mandala_only_tests {
	use super::*;
	use ecosystem_renvm_bridge::EcdsaSignature;
	use frame_support::dispatch::GetDispatchInfo;
	use hex_literal::hex;
	use mandala_runtime::RenVmBridge;
	use module_transaction_payment::ChargeTransactionPayment;
	use pallet_transaction_payment::InclusionFee;
	use sp_runtime::{
		traits::{Extrinsic, SignedExtension, ValidateUnsigned},
		transaction_validity::{TransactionSource, ValidTransaction},
	};

	#[test]
	fn check_transaction_fee_for_empty_remark() {
		ExtBuilder::default().build().execute_with(|| {
			let call = Call::System(frame_system::Call::remark { remark: vec![] });
			let ext = UncheckedExtrinsic::new(call.into(), None).expect("This should not fail");
			let bytes = ext.encode();

			// Get information on the fee for the call.
			let fee = TransactionPayment::query_fee_details(ext, bytes.len() as u32);

			let InclusionFee {
				base_fee,
				len_fee,
				adjusted_weight_fee,
			} = fee.inclusion_fee.unwrap();

			assert_eq!(base_fee, 1_000_000_000);
			assert_eq!(len_fee, 500_000_000);
			assert_eq!(adjusted_weight_fee, 0);

			let total_fee = base_fee.saturating_add(len_fee).saturating_add(adjusted_weight_fee);
			assert_eq!(total_fee, 1_500_000_000);
		});
	}

	#[test]
	fn check_tx_priority() {
		ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY, 20_000 * dollar(NATIVE_CURRENCY)),
		])
		.build().execute_with(|| {
			// Ensure tx priority order:
			// Inherent -> Operational tx -> Unsigned tx -> Signed normal tx
			let call = Call::System(frame_system::Call::remark { remark: vec![] });
			let bytes = UncheckedExtrinsic::new(call.clone().into(), None).expect("This should not fail").encode();

			// tips = 0
			assert_eq!(
				ChargeTransactionPayment::<Runtime>::from(0).validate(
					&alice(),
					&call.clone(),
					&call.get_dispatch_info(),
					bytes.len()
				),
				Ok(ValidTransaction {
					priority: 0,
					requires: vec![],
					provides: vec![],
					longevity: 18_446_744_073_709_551_615,
					propagate: true,
				})
			);

			// tips = TipPerWeightStep
			assert_eq!(
				ChargeTransactionPayment::<Runtime>::from(TipPerWeightStep::get()).validate(
					&alice(),
					&call.clone(),
					&call.get_dispatch_info(),
					bytes.len()
				),
				Ok(ValidTransaction {
					priority: 734_003,
					requires: vec![],
					provides: vec![],
					longevity: 18_446_744_073_709_551_615,
					propagate: true,
				})
			);

			// tips = TipPerWeightStep + 1
			assert_eq!(
				ChargeTransactionPayment::<Runtime>::from(TipPerWeightStep::get() + 1).validate(
					&alice(),
					&call.clone(),
					&call.get_dispatch_info(),
					bytes.len()
				),
				Ok(ValidTransaction {
					priority: 734_003,
					requires: vec![],
					provides: vec![],
					longevity: 18_446_744_073_709_551_615,
					propagate: true,
				})
			);

			// tips = MaxTipsOfPriority + 1
			assert_eq!(
				ChargeTransactionPayment::<Runtime>::from(MaxTipsOfPriority::get() + 1).validate(
					&alice(),
					&call.clone(),
					&call.get_dispatch_info(),
					bytes.len()
				),
				Ok(ValidTransaction {
					priority: 734_003_000_000,
					requires: vec![],
					provides: vec![],
					longevity: 18_446_744_073_709_551_615,
					propagate: true,
				})
			);

			// tips = 0
			// unsigned extrinsic
			let sig = EcdsaSignature::from_slice(&hex!["defda6eef01da2e2a90ce30ba73e90d32204ae84cae782b485f01d16b69061e0381a69cafed3deb6112af044c42ed0f7c73ee0eec7b533334d31a06db50fc40e1b"]).unwrap();
			let call = ecosystem_renvm_bridge::Call::mint {
				who: hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into(),
				p_hash: hex!["67028f26328144de6ef80b8cd3b05e0cefb488762c340d1574c0542f752996cb"],
				amount: 93963,
				n_hash: hex!["f6a75cc370a2dda6dfc8d016529766bb6099d7fa0d787d9fe5d3a7e60c9ac2a0"],
				sig: sig.clone(),
			};

			assert_eq!(
				RenVmBridge::validate_unsigned(
					TransactionSource::Local,
					&call,
				),
				Ok(ValidTransaction {
					priority: 14_999_999_997_000,
					requires: vec![],
					provides: vec![("renvm-bridge", sig).encode()],
					longevity: 64,
					propagate: true,
				})
			);

			// tips = 0
			// operational extrinsic
			let call = Call::Sudo(pallet_sudo::Call::sudo { call: Box::new(module_emergency_shutdown::Call::open_collateral_refund { }.into()) });
			let bytes = UncheckedExtrinsic::new(call.clone().into(), None).expect("This should not fail").encode();

			assert_eq!(
				ChargeTransactionPayment::<Runtime>::from(0).validate(
					&alice(),
					&call.clone(),
					&call.get_dispatch_info(),
					bytes.len()
				),
				Ok(ValidTransaction {
					priority: 81_156_562_730_100_000,
					requires: vec![],
					provides: vec![],
					longevity: 18_446_744_073_709_551_615,
					propagate: true,
				})
			);

		});
	}
}
