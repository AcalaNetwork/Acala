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
use frame_support::{
	assert_ok, parameter_types,
	traits::{Everything, IsInVec},
	weights::Weight,
};
use xcm_builder::{AllowSubscriptionsFrom, AllowTopLevelPaidExecutionFrom, TakeWeightCredit};
use xcm_executor::{traits::*, Config, XcmExecutor};

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

#[test]
fn weigher_weight_and_take_weight_credit_barrier_works() {
	let mut message = Xcm(vec![
		ReserveAssetDeposited((Parent, 100).into()),
		BuyExecution {
			fees: (Parent, 1).into(),
			weight_limit: Limited(10),
		},
		DepositAsset {
			assets: All.into(),
			max_assets: 1,
			beneficiary: Here.into(),
		},
	]);

	#[cfg(feature = "with-karura-runtime")]
	{
		let expect_weight: Weight = 600_000_000;
		let mut weight_credit = 1_000_000_000;
		assert_eq!(<XcmConfig as Config>::Weigher::weight(&mut message), Ok(expect_weight));
		let r = TakeWeightCredit::should_execute(&Parent.into(), &mut message, expect_weight, &mut weight_credit);
		assert_ok!(r);
		assert_eq!(weight_credit, 400_000_000);

		let r = TakeWeightCredit::should_execute(&Parent.into(), &mut message, expect_weight, &mut weight_credit);
		assert_eq!(r, Err(()));
		assert_eq!(weight_credit, 400_000_000);

		let r = XcmExecutor::<XcmConfig>::execute_xcm(Parent, message.clone(), 10);
		assert_eq!(r, Outcome::Error(XcmError::WeightLimitReached(expect_weight)));
	}

	#[cfg(feature = "with-mandala-runtime")]
	{
		let expect_weight: Weight = 3_000_000;
		let mut weight_credit = 4_000_000;
		assert_eq!(<XcmConfig as Config>::Weigher::weight(&mut message), Ok(expect_weight));
		let r = TakeWeightCredit::should_execute(&Parent.into(), &mut message, expect_weight, &mut weight_credit);
		assert_ok!(r);
		assert_eq!(weight_credit, 1_000_000);

		let r = TakeWeightCredit::should_execute(&Parent.into(), &mut message, expect_weight, &mut weight_credit);
		assert_eq!(r, Err(()));
		assert_eq!(weight_credit, 1_000_000);

		let r = XcmExecutor::<XcmConfig>::execute_xcm(Parent, message.clone(), 10);
		assert_eq!(r, Outcome::Error(XcmError::WeightLimitReached(expect_weight)));
	}

	#[cfg(feature = "with-acala-runtime")]
	{
		assert_eq!(<XcmConfig as Config>::Weigher::weight(&mut message), Ok(600_000_000));
	}
}

#[cfg(feature = "with-karura-runtime")]
#[test]
fn top_level_paid_execution_barrier_works() {
	let mut message = Xcm::<karura_runtime::Call>(vec![
		ReserveAssetDeposited((Parent, 100).into()),
		BuyExecution {
			fees: (Parent, 1).into(),
			weight_limit: Limited(10),
		},
		DepositAsset {
			assets: All.into(),
			max_assets: 1,
			beneficiary: Here.into(),
		},
	]);

	// BuyExecution weight_limit set to 10
	let r = AllowTopLevelPaidExecutionFrom::<Everything>::should_execute(&Parent.into(), &mut message, 10, &mut 0);
	assert_ok!(r);

	// BuyExecution weight_limit less than max_weight, error
	let r = AllowTopLevelPaidExecutionFrom::<Everything>::should_execute(&Parent.into(), &mut message, 20, &mut 0);
	assert_eq!(r, Err(()));
}

#[test]
fn barrier_contains_works() {
	parameter_types! {
		pub static AllowUnpaidFrom: Vec<MultiLocation> = vec![];
		pub static AllowPaidFrom: Vec<MultiLocation> = vec![];
		pub static AllowSubsFrom: Vec<MultiLocation> = vec![Parent.into()];
	}
	let mut message1 = Xcm::<()>(vec![
		ReserveAssetDeposited((Parent, 100).into()),
		BuyExecution {
			fees: (Parent, 1).into(),
			weight_limit: Limited(20),
		},
		DepositAsset {
			assets: All.into(),
			max_assets: 1,
			beneficiary: Here.into(),
		},
	]);
	let mut message2 = Xcm::<()>(vec![SubscribeVersion {
		query_id: 42,
		max_response_weight: 5000,
	}]);

	// T::Contains set to Parent
	AllowSubsFrom::set(vec![Parent.into()]);
	let r = AllowTopLevelPaidExecutionFrom::<IsInVec<AllowSubsFrom>>::should_execute(
		&Parent.into(),
		&mut message1,
		10,
		&mut 0,
	);
	assert_ok!(r);
	let r = AllowSubscriptionsFrom::<IsInVec<AllowSubsFrom>>::should_execute(&Parent.into(), &mut message2, 20, &mut 0);
	assert_ok!(r);

	// T::Contains set to Parachain(1000)
	AllowSubsFrom::set(vec![Parachain(1000).into()]);
	let r = AllowTopLevelPaidExecutionFrom::<IsInVec<AllowSubsFrom>>::should_execute(
		&Parent.into(),
		&mut message1,
		10,
		&mut 0,
	);
	assert_eq!(r, Err(()));
	let r = AllowSubscriptionsFrom::<IsInVec<AllowSubsFrom>>::should_execute(&Parent.into(), &mut message2, 20, &mut 0);
	assert_eq!(r, Err(()));

	// T::Contains set to empty
	AllowSubsFrom::set(vec![]);
	let r = AllowTopLevelPaidExecutionFrom::<IsInVec<AllowSubsFrom>>::should_execute(
		&Parent.into(),
		&mut message1,
		10,
		&mut 0,
	);
	assert_eq!(r, Err(()));
	let r = AllowSubscriptionsFrom::<IsInVec<AllowSubsFrom>>::should_execute(&Parent.into(), &mut message2, 20, &mut 0);
	assert_eq!(r, Err(()));
	let r = AllowTopLevelPaidExecutionFrom::<Everything>::should_execute(&Parent.into(), &mut message1, 10, &mut 0);
	assert_ok!(r);
	let r = AllowSubscriptionsFrom::<Everything>::should_execute(&Parent.into(), &mut message2, 20, &mut 0);
	assert_ok!(r);
}

#[test]
fn xcm_executor_execute_xcm() {
	#[cfg(feature = "with-karura-runtime")]
	{
		ExtBuilder::default().build().execute_with(|| {
			// weight limited set to Unlimited, it's ok
			let message = Xcm::<karura_runtime::Call>(vec![
				ReserveAssetDeposited((Parent, 600_000_000).into()),
				BuyExecution {
					fees: (Parent, 600_000_000).into(),
					weight_limit: Unlimited,
				},
				DepositAsset {
					assets: All.into(),
					max_assets: 1,
					beneficiary: Here.into(),
				},
			]);

			let r = XcmExecutor::<XcmConfig>::execute_xcm(Parent, message, 600_000_000);
			assert_eq!(r, Outcome::Complete(600_000_000));
		});
	}

	#[cfg(feature = "with-acala-runtime")]
	{
		ExtBuilder::default().build().execute_with(|| {
			// weight limited large than xcm_weight, it's ok
			let message = Xcm::<acala_runtime::Call>(vec![
				ReserveAssetDeposited((Parent, 600_000_000).into()),
				BuyExecution {
					fees: (Parent, 600_000_000).into(),
					weight_limit: Limited(6_000_000_000),
				},
				DepositAsset {
					assets: All.into(),
					max_assets: 1,
					beneficiary: Here.into(),
				},
			]);

			let r = XcmExecutor::<acala_runtime::XcmConfig>::execute_xcm(Parent, message, 600_000_000);
			assert_eq!(r, Outcome::Complete(600_000_000));
		});
	}

	#[cfg(feature = "with-mandala-runtime")]
	{
		ExtBuilder::default().build().execute_with(|| {
			// weight limited less than xcm_weight, it's error
			let message = Xcm::<mandala_runtime::Call>(vec![
				ReserveAssetDeposited((Parent, 3_000_000).into()),
				BuyExecution {
					fees: (Parent, 3_000_000).into(),
					weight_limit: Limited(300_000),
				},
				DepositAsset {
					assets: All.into(),
					max_assets: 1,
					beneficiary: Here.into(),
				},
			]);

			let r = XcmExecutor::<mandala_runtime::XcmConfig>::execute_xcm(Parent, message, 3_000_000);
			assert_eq!(r, Outcome::Error(XcmError::Barrier));
		});
	}
}

#[cfg(feature = "with-karura-runtime")]
#[test]
fn subscribe_version_barrier_works() {
	ExtBuilder::default().build().execute_with(|| {
		// BadOrigin if original origin is not equal to origin
		let origin = Parachain(1000).into();
		let message = Xcm(vec![
			DescendOrigin(X1(AccountIndex64 { index: 1, network: Any })),
			SubscribeVersion {
				query_id: 42,
				max_response_weight: 5000,
			},
		]);
		let weight_limit = 2_000_000_000;
		let r = XcmExecutor::<kusama_runtime::XcmConfig>::execute_xcm_in_credit(
			origin.clone(),
			message.clone(),
			weight_limit,
			weight_limit,
		);
		assert_eq!(r, Outcome::Incomplete(weight_limit, XcmError::BadOrigin));

		// relay chain force subscribe version notify of karura para chain
		let message = Xcm(vec![SubscribeVersion {
			query_id: 42,
			max_response_weight: 5000,
		}]);
		let weight_limit = 1_000_000_000;
		let r = XcmExecutor::<karura_runtime::XcmConfig>::execute_xcm_in_credit(
			Parent,
			message.clone(),
			weight_limit,
			weight_limit,
		);
		assert_eq!(r, Outcome::Complete(200_000_000));
	});
}
