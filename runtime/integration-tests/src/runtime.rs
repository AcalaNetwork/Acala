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

use crate::setup::*;

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
		let parachain: AccountId = ParachainInfo::parachain_id().into_account();
		assert_eq!(
			parachain,
			hex_literal::hex!["70617261d0070000000000000000000000000000000000000000000000000000"].into()
		);

		assert_eq!(
			RelayChainSovereignSubAccount::get(),
			create_x2_parachain_multilocation(0)
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
