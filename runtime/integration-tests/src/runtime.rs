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
			Some(Location::parent())
		);

		assert_eq!(
			CurrencyIdConvert::convert(NATIVE_CURRENCY),
			Some(Location::sibling_parachain_general_key(
				id,
				NATIVE_CURRENCY.encode().try_into().unwrap()
			))
		);
		assert_eq!(
			CurrencyIdConvert::convert(USD_CURRENCY),
			Some(Location::sibling_parachain_general_key(
				id,
				USD_CURRENCY.encode().try_into().unwrap()
			))
		);
		assert_eq!(
			CurrencyIdConvert::convert(LIQUID_CURRENCY),
			Some(Location::sibling_parachain_general_key(
				id,
				LIQUID_CURRENCY.encode().try_into().unwrap()
			))
		);
		assert_eq!(
			CurrencyIdConvert::convert(Location::parent()),
			Some(RELAY_CHAIN_CURRENCY)
		);
		assert_eq!(
			CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
				id,
				NATIVE_CURRENCY.encode().try_into().unwrap()
			)),
			Some(NATIVE_CURRENCY)
		);
		assert_eq!(
			CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
				id,
				USD_CURRENCY.encode().try_into().unwrap()
			)),
			Some(USD_CURRENCY)
		);
		assert_eq!(
			CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
				id,
				LIQUID_CURRENCY.encode().try_into().unwrap()
			)),
			Some(LIQUID_CURRENCY)
		);

		#[cfg(feature = "with-mandala-runtime")]
		{
			assert_eq!(CurrencyIdConvert::convert(KAR), None);
			assert_eq!(CurrencyIdConvert::convert(KUSD), None);
			assert_eq!(CurrencyIdConvert::convert(KSM), None);
			assert_eq!(CurrencyIdConvert::convert(LKSM), None);
			assert_eq!(CurrencyIdConvert::convert(TAP), None);

			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					KAR.encode().try_into().unwrap()
				)),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					KUSD.encode().try_into().unwrap()
				)),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					KSM.encode().try_into().unwrap()
				)),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					LKSM.encode().try_into().unwrap()
				)),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					TAP.encode().try_into().unwrap()
				)),
				None
			);

			let native_currency: Asset = (
				Location::sibling_parachain_general_key(id, NATIVE_CURRENCY.encode().try_into().unwrap()),
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
			assert_eq!(CurrencyIdConvert::convert(TAP), None);

			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					ACA.encode().try_into().unwrap()
				)),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					AUSD.encode().try_into().unwrap()
				)),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					DOT.encode().try_into().unwrap()
				)),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					LDOT.encode().try_into().unwrap()
				)),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					TAP.encode().try_into().unwrap()
				)),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					TAI.encode().try_into().unwrap()
				)),
				Some(TAI)
			);
			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					parachains::bifrost::ID,
					parachains::bifrost::BNC_KEY.to_vec().try_into().unwrap()
				)),
				Some(BNC)
			);
			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					parachains::bifrost::ID,
					parachains::bifrost::VSKSM_KEY.to_vec().try_into().unwrap()
				)),
				Some(VSKSM)
			);

			assert_eq!(
				CurrencyIdConvert::convert(BNC),
				Some(Location::sibling_parachain_general_key(
					parachains::bifrost::ID,
					parachains::bifrost::BNC_KEY.to_vec().try_into().unwrap()
				))
			);
			assert_eq!(
				CurrencyIdConvert::convert(VSKSM),
				Some(Location::sibling_parachain_general_key(
					parachains::bifrost::ID,
					parachains::bifrost::VSKSM_KEY.to_vec().try_into().unwrap()
				))
			);

			let native_currency: Asset = (
				Location::sibling_parachain_general_key(id, NATIVE_CURRENCY.encode().try_into().unwrap()),
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
			assert_eq!(CurrencyIdConvert::convert(TAI), None);

			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					KAR.encode().try_into().unwrap()
				)),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					KUSD.encode().try_into().unwrap()
				)),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					KSM.encode().try_into().unwrap()
				)),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					LKSM.encode().try_into().unwrap()
				)),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					TAI.encode().try_into().unwrap()
				)),
				None
			);
			assert_eq!(
				CurrencyIdConvert::convert(Location::sibling_parachain_general_key(
					id,
					TAP.encode().try_into().unwrap()
				)),
				Some(TAP)
			);

			let native_currency: Asset = (
				Location::sibling_parachain_general_key(id, NATIVE_CURRENCY.encode().try_into().unwrap()),
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
			create_x2_parachain_location(0),
			Location::new(
				1,
				[Junction::AccountId32 {
					network: None,
					id: hex_literal::hex!["d7b8926b326dd349355a9a7cca6606c1e0eb6fd2b506066b518c7155ff0d8297"].into(),
				}]
			),
		);
		assert_eq!(
			create_x2_parachain_location(1),
			Location::new(
				1,
				[Junction::AccountId32 {
					network: None,
					id: hex_literal::hex!["74d37d762e06c6841a5dad64463a9afe0684f7e45245f6a7296ca613cca74669"].into(),
				}]
			),
		);
	});
}

#[cfg(feature = "with-mandala-runtime")]
mod mandala_only_tests {
	use super::*;
	use frame_support::dispatch::GetDispatchInfo;
	use module_transaction_payment::ChargeTransactionPayment;
	use pallet_transaction_payment::InclusionFee;
	use sp_runtime::{
		traits::{Extrinsic, SignedExtension, ValidateUnsigned},
		transaction_validity::{TransactionSource, ValidTransaction},
	};

	#[test]
	fn check_transaction_fee_for_empty_remark() {
		ExtBuilder::default().build().execute_with(|| {
			let call = RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
			let ext = UncheckedExtrinsic::new(call.into(), None).expect("This should not fail");
			let bytes = ext.encode();

			// Get information on the fee for the call.
			let fee = TransactionPayment::query_fee_details(ext, bytes.len() as u32);

			let InclusionFee {
				base_fee,
				len_fee,
				adjusted_weight_fee,
			} = fee.inclusion_fee.unwrap();

			assert_debug_snapshot!(base_fee, @"1000000000");
			assert_debug_snapshot!(len_fee, @"50000000");
			assert_debug_snapshot!(adjusted_weight_fee, @"10625773");

			let total_fee = base_fee.saturating_add(len_fee).saturating_add(adjusted_weight_fee);
			assert_debug_snapshot!(total_fee, @"1060625773");
		});
	}

	#[test]
	fn check_tx_priority() {
		ExtBuilder::default()
			.balances(vec![(alice(), NATIVE_CURRENCY, 20_000 * dollar(NATIVE_CURRENCY))])
			.build()
			.execute_with(|| {
				// Ensure tx priority order:
				// Inherent -> Operational tx -> Unsigned tx -> Signed normal tx
				let call = RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
				let bytes = UncheckedExtrinsic::new(call.clone().into(), None)
					.expect("This should not fail")
					.encode();

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
				assert_debug_snapshot!(
					ChargeTransactionPayment::<Runtime>::from(TipPerWeightStep::get()).validate(
						&alice(),
						&call.clone(),
						&call.get_dispatch_info(),
						bytes.len()
					),
					@r###"
    Ok(
        ValidTransaction {
            priority: 439466,
            requires: [],
            provides: [],
            longevity: 18446744073709551615,
            propagate: true,
        },
    )
    "###
				);

				// tips = TipPerWeightStep + 1
				assert_debug_snapshot!(
					ChargeTransactionPayment::<Runtime>::from(TipPerWeightStep::get() + 1).validate(
						&alice(),
						&call.clone(),
						&call.get_dispatch_info(),
						bytes.len()
					),
					@r###"
    Ok(
        ValidTransaction {
            priority: 439466,
            requires: [],
            provides: [],
            longevity: 18446744073709551615,
            propagate: true,
        },
    )
    "###
				);

				// tips = MaxTipsOfPriority + 1
				assert_debug_snapshot!(
					ChargeTransactionPayment::<Runtime>::from(MaxTipsOfPriority::get() + 1).validate(
						&alice(),
						&call.clone(),
						&call.get_dispatch_info(),
						bytes.len()
					),
					@r###"
    Ok(
        ValidTransaction {
            priority: 439466000000,
            requires: [],
            provides: [],
            longevity: 18446744073709551615,
            propagate: true,
        },
    )
    "###
				);

				// setup a unsafe cdp
				set_oracle_price(vec![(NATIVE_CURRENCY, Price::saturating_from_rational(10, 1))]);
				assert_ok!(CdpEngine::set_collateral_params(
					RuntimeOrigin::root(),
					NATIVE_CURRENCY,
					Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
					Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
					Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
					Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
					Change::NewValue(1000 * dollar(AUSD)),
				));
				assert_ok!(CdpEngine::adjust_position(
					&alice(),
					NATIVE_CURRENCY,
					100 * dollar(NATIVE_CURRENCY) as i128,
					500 * dollar(AUSD) as i128
				));
				set_oracle_price(vec![(NATIVE_CURRENCY, Price::saturating_from_rational(1, 10))]);

				// tips = 0
				// unsigned extrinsic
				let call = module_cdp_engine::Call::liquidate {
					currency_id: NATIVE_CURRENCY,
					who: MultiAddress::Id(alice()),
				};

				assert_eq!(
					CdpEngine::validate_unsigned(TransactionSource::Local, &call,),
					Ok(ValidTransaction {
						priority: 14_999_999_999_000,
						requires: vec![],
						provides: vec![("CDPEngineOffchainWorker", 1u8, 0u32, NATIVE_CURRENCY, alice()).encode()],
						longevity: 64,
						propagate: true,
					})
				);

				// tips = 0
				// operational extrinsic
				let call = RuntimeCall::Sudo(pallet_sudo::Call::sudo {
					call: Box::new(module_emergency_shutdown::Call::open_collateral_refund {}.into()),
				});
				let bytes = UncheckedExtrinsic::new(call.clone().into(), None)
					.expect("This should not fail")
					.encode();

				assert_debug_snapshot!(
					ChargeTransactionPayment::<Runtime>::from(0).validate(
						&alice(),
						&call.clone(),
						&call.get_dispatch_info(),
						bytes.len()
					),
					@r###"
    Ok(
        ValidTransaction {
            priority: 61218482942130000,
            requires: [],
            provides: [],
            longevity: 18446744073709551615,
            propagate: true,
        },
    )
    "###
				);
			});
	}
}
