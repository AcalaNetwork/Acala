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
use frame_support::traits::{schedule::DispatchTime, Bounded, OriginTrait};
use orml_authority::DelayedOrigin;
use sp_io::hashing::blake2_256;

fn bounded_call(call: RuntimeCall) -> Box<Bounded<RuntimeCall, <Runtime as frame_system::Config>::Hashing>> {
	let encoded_call = call.encode();
	Box::new(Bounded::Inline(encoded_call.try_into().unwrap()))
}

#[test]
fn test_authority_module() {
	#[cfg(feature = "with-mandala-runtime")]
	const AUTHORITY_ORIGIN_ID: u8 = 70u8;

	#[cfg(feature = "with-karura-runtime")]
	const AUTHORITY_ORIGIN_ID: u8 = 60u8;

	#[cfg(feature = "with-acala-runtime")]
	const AUTHORITY_ORIGIN_ID: u8 = 60u8;

	ExtBuilder::default()
		.balances(vec![
			(AccountId::from(ALICE), USD_CURRENCY, 1_000 * dollar(USD_CURRENCY)),
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				1_000 * dollar(RELAY_CHAIN_CURRENCY),
			),
			(TreasuryAccount::get(), USD_CURRENCY, 1_000 * dollar(USD_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			let ensure_root_call = RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
			let call = RuntimeCall::Authority(orml_authority::Call::dispatch_as {
				as_origin: AuthoritysOriginId::Root,
				call: Box::new(ensure_root_call.clone()),
			});

			// dispatch_as
			assert_ok!(Authority::dispatch_as(
				RuntimeOrigin::root(),
				AuthoritysOriginId::Root,
				Box::new(ensure_root_call.clone())
			));

			assert_noop!(
				Authority::dispatch_as(
					RuntimeOrigin::signed(AccountId::from(BOB)),
					AuthoritysOriginId::Root,
					Box::new(ensure_root_call.clone())
				),
				BadOrigin
			);

			assert_noop!(
				Authority::dispatch_as(
					RuntimeOrigin::signed(AccountId::from(BOB)),
					AuthoritysOriginId::Treasury,
					Box::new(ensure_root_call.clone())
				),
				BadOrigin
			);

			// schedule_dispatch
			run_to_block(1);
			// Treasury transfer
			let transfer_call = RuntimeCall::Currencies(module_currencies::Call::transfer {
				dest: AccountId::from(BOB).into(),
				currency_id: USD_CURRENCY,
				amount: 500 * dollar(USD_CURRENCY),
			});
			let treasury_reserve_call = RuntimeCall::Authority(orml_authority::Call::dispatch_as {
				as_origin: AuthoritysOriginId::Treasury,
				call: Box::new(transfer_call.clone()),
			});

			let one_day_later = OneDay::get() + 1;

			assert_ok!(Authority::schedule_dispatch(
				RuntimeOrigin::root(),
				DispatchTime::At(one_day_later),
				0,
				true,
				bounded_call(treasury_reserve_call)
			));

			assert_ok!(Authority::schedule_dispatch(
				RuntimeOrigin::root(),
				DispatchTime::At(one_day_later),
				0,
				true,
				bounded_call(call.clone())
			));
			System::assert_last_event(RuntimeEvent::Authority(orml_authority::Event::Scheduled {
				origin: OriginCaller::Authority(DelayedOrigin::new(
					one_day_later - 1,
					Box::new(OriginCaller::system(RawOrigin::Root)),
				)),
				index: 1,
			}));

			run_to_block(one_day_later);

			assert_eq!(
				Currencies::free_balance(USD_CURRENCY, &TreasuryPalletId::get().into_account_truncating()),
				500 * dollar(USD_CURRENCY)
			);
			assert_eq!(
				Currencies::free_balance(USD_CURRENCY, &AccountId::from(BOB)),
				500 * dollar(USD_CURRENCY)
			);

			// delay < SevenDays
			#[cfg(feature = "with-mandala-runtime")]
			System::assert_last_event(RuntimeEvent::Scheduler(
				pallet_scheduler::Event::<Runtime>::Dispatched {
					task: (OneDay::get() + 1, 1),
					id: Some(blake2_256(
						[AUTHORITY_ORIGIN_ID, 32, 28, 0, 0, 0, 0, 1, 0, 0, 0].as_ref(),
					)),
					result: Err(DispatchError::BadOrigin),
				},
			));
			#[cfg(feature = "with-karura-runtime")]
			System::assert_last_event(RuntimeEvent::Scheduler(
				pallet_scheduler::Event::<Runtime>::Dispatched {
					task: (OneDay::get() + 1, 1),
					id: Some(blake2_256(
						[AUTHORITY_ORIGIN_ID, 32, 28, 0, 0, 0, 0, 1, 0, 0, 0].as_ref(),
					)),
					result: Err(DispatchError::BadOrigin),
				},
			));
			#[cfg(feature = "with-acala-runtime")]
			System::assert_last_event(RuntimeEvent::Scheduler(
				pallet_scheduler::Event::<Runtime>::Dispatched {
					task: (OneDay::get() + 1, 1),
					id: Some(blake2_256(
						[AUTHORITY_ORIGIN_ID, 32, 28, 0, 0, 0, 0, 1, 0, 0, 0].as_ref(),
					)),
					result: Err(DispatchError::BadOrigin),
				},
			));

			let seven_days_later = one_day_later + SevenDays::get() + 1;

			// delay = SevenDays
			assert_ok!(Authority::schedule_dispatch(
				RuntimeOrigin::root(),
				DispatchTime::At(seven_days_later),
				0,
				true,
				bounded_call(call.clone())
			));

			run_to_block(seven_days_later);

			#[cfg(feature = "with-mandala-runtime")]
			System::assert_last_event(RuntimeEvent::Scheduler(
				pallet_scheduler::Event::<Runtime>::Dispatched {
					task: (seven_days_later, 0),
					id: Some(blake2_256(
						[AUTHORITY_ORIGIN_ID, 225, 196, 0, 0, 0, 0, 2, 0, 0, 0].as_ref(),
					)),
					result: Ok(()),
				},
			));

			#[cfg(feature = "with-karura-runtime")]
			System::assert_last_event(RuntimeEvent::Scheduler(
				pallet_scheduler::Event::<Runtime>::Dispatched {
					task: (seven_days_later, 0),
					id: Some(blake2_256(
						[AUTHORITY_ORIGIN_ID, 225, 196, 0, 0, 0, 0, 2, 0, 0, 0].as_ref(),
					)),
					result: Ok(()),
				},
			));

			#[cfg(feature = "with-acala-runtime")]
			System::assert_last_event(RuntimeEvent::Scheduler(
				pallet_scheduler::Event::<Runtime>::Dispatched {
					task: (seven_days_later, 0),
					id: Some(blake2_256(
						[AUTHORITY_ORIGIN_ID, 225, 196, 0, 0, 0, 0, 2, 0, 0, 0].as_ref(),
					)),
					result: Ok(()),
				},
			));

			// with_delayed_origin = false
			assert_ok!(Authority::schedule_dispatch(
				RuntimeOrigin::root(),
				DispatchTime::At(seven_days_later + 1),
				0,
				false,
				bounded_call(call.clone())
			));
			System::assert_last_event(RuntimeEvent::Authority(orml_authority::Event::Scheduled {
				origin: OriginCaller::system(RawOrigin::Root),
				index: 3,
			}));

			run_to_block(seven_days_later + 1);
			System::assert_last_event(RuntimeEvent::Scheduler(
				pallet_scheduler::Event::<Runtime>::Dispatched {
					task: (seven_days_later + 1, 0),
					id: Some(blake2_256([0, 0, 3, 0, 0, 0].as_ref())),
					result: Ok(()),
				},
			));

			assert_ok!(Authority::schedule_dispatch(
				RuntimeOrigin::root(),
				DispatchTime::At(seven_days_later + 2),
				0,
				false,
				bounded_call(call.clone())
			));

			// fast_track_scheduled_dispatch
			assert_ok!(Authority::fast_track_scheduled_dispatch(
				RuntimeOrigin::root(),
				Box::new(frame_system::RawOrigin::Root.into()),
				4,
				DispatchTime::At(seven_days_later + 3),
			));

			// delay_scheduled_dispatch
			assert_ok!(Authority::delay_scheduled_dispatch(
				RuntimeOrigin::root(),
				Box::new(frame_system::RawOrigin::Root.into()),
				4,
				4,
			));

			// cancel_scheduled_dispatch
			assert_ok!(Authority::schedule_dispatch(
				RuntimeOrigin::root(),
				DispatchTime::At(seven_days_later + 2),
				0,
				true,
				bounded_call(call.clone())
			));
			System::assert_last_event(RuntimeEvent::Authority(orml_authority::Event::Scheduled {
				origin: OriginCaller::Authority(DelayedOrigin::new(1, Box::new(OriginCaller::system(RawOrigin::Root)))),
				index: 5,
			}));

			let schedule_origin = {
				let origin: <Runtime as orml_authority::Config>::RuntimeOrigin = From::from(RuntimeOrigin::root());
				let origin: <Runtime as orml_authority::Config>::RuntimeOrigin =
					From::from(DelayedOrigin::<
						BlockNumber,
						<Runtime as orml_authority::Config>::PalletsOrigin,
					>::new(1, Box::new(origin.caller().clone())));
				origin
			};

			let pallets_origin = Box::new(schedule_origin.caller().clone());
			assert_ok!(Authority::cancel_scheduled_dispatch(
				RuntimeOrigin::root(),
				pallets_origin,
				5
			));
			System::assert_last_event(RuntimeEvent::Authority(orml_authority::Event::Cancelled {
				origin: OriginCaller::Authority(DelayedOrigin::new(1, Box::new(OriginCaller::system(RawOrigin::Root)))),
				index: 5,
			}));

			assert_ok!(Authority::schedule_dispatch(
				RuntimeOrigin::root(),
				DispatchTime::At(seven_days_later + 3),
				0,
				false,
				bounded_call(call)
			));
			System::assert_last_event(RuntimeEvent::Authority(orml_authority::Event::Scheduled {
				origin: OriginCaller::system(RawOrigin::Root),
				index: 6,
			}));

			assert_ok!(Authority::cancel_scheduled_dispatch(
				RuntimeOrigin::root(),
				Box::new(frame_system::RawOrigin::Root.into()),
				6
			));
			System::assert_last_event(RuntimeEvent::Authority(orml_authority::Event::Cancelled {
				origin: OriginCaller::system(RawOrigin::Root),
				index: 6,
			}));
		});
}

#[test]
fn cancel_schedule_test() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(FinancialCouncil::set_members(
			RuntimeOrigin::root(),
			vec![AccountId::from(ALICE), AccountId::from(BOB), AccountId::from(CHARLIE)],
			None,
			5,
		));
		let council_call = RuntimeCall::CdpEngine(module_cdp_engine::Call::set_collateral_params {
			currency_id: DOT,
			interest_rate_per_sec: Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			liquidation_ratio: Change::NewValue(Some(Ratio::saturating_from_rational(5, 2))),
			liquidation_penalty: Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			required_collateral_ratio: Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			maximum_total_debit_value: Change::NewValue(10000),
		});

		assert_ok!(Authority::schedule_dispatch(
			OriginCaller::FinancialCouncil(pallet_collective::RawOrigin::Members(2, 3)).into(),
			DispatchTime::At(2),
			0,
			false,
			bounded_call(council_call.clone()),
		));

		// canceling will not work if yes vote is less than the scheduled call
		assert_noop!(
			Authority::cancel_scheduled_dispatch(
				OriginCaller::FinancialCouncil(pallet_collective::RawOrigin::Members(1, 3)).into(),
				Box::new(OriginCaller::FinancialCouncil(pallet_collective::RawOrigin::Members(
					2, 3
				))),
				0,
			),
			BadOrigin
		);
		// canceling works when yes vote is greater than the scheduled call
		assert_ok!(Authority::cancel_scheduled_dispatch(
			OriginCaller::FinancialCouncil(pallet_collective::RawOrigin::Members(3, 3)).into(),
			Box::new(OriginCaller::FinancialCouncil(pallet_collective::RawOrigin::Members(
				2, 3
			))),
			0,
		));

		assert_ok!(Authority::schedule_dispatch(
			OriginCaller::FinancialCouncil(pallet_collective::RawOrigin::Members(2, 3)).into(),
			DispatchTime::At(2),
			0,
			false,
			bounded_call(council_call),
		));
		// canceling works when yes vote is equal to the scheduled call
		assert_ok!(Authority::cancel_scheduled_dispatch(
			OriginCaller::FinancialCouncil(pallet_collective::RawOrigin::Members(2, 3)).into(),
			Box::new(OriginCaller::FinancialCouncil(pallet_collective::RawOrigin::Members(
				2, 3
			))),
			1,
		));
	});
}
