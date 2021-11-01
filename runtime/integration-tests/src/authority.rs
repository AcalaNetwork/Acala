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
use frame_support::traits::{schedule::DispatchTime, OriginTrait};
use orml_authority::DelayedOrigin;

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
			let ensure_root_call = Call::System(frame_system::Call::fill_block { ratio: Perbill::one() });
			let call = Call::Authority(orml_authority::Call::dispatch_as {
				as_origin: AuthoritysOriginId::Root,
				call: Box::new(ensure_root_call.clone()),
			});

			// dispatch_as
			assert_ok!(Authority::dispatch_as(
				Origin::root(),
				AuthoritysOriginId::Root,
				Box::new(ensure_root_call.clone())
			));

			assert_noop!(
				Authority::dispatch_as(
					Origin::signed(AccountId::from(BOB)),
					AuthoritysOriginId::Root,
					Box::new(ensure_root_call.clone())
				),
				BadOrigin
			);

			assert_noop!(
				Authority::dispatch_as(
					Origin::signed(AccountId::from(BOB)),
					AuthoritysOriginId::Treasury,
					Box::new(ensure_root_call.clone())
				),
				BadOrigin
			);

			// schedule_dispatch
			run_to_block(1);
			// Treasury transfer
			let transfer_call = Call::Currencies(module_currencies::Call::transfer {
				dest: AccountId::from(BOB).into(),
				currency_id: USD_CURRENCY,
				amount: 500 * dollar(USD_CURRENCY),
			});
			let treasury_reserve_call = Call::Authority(orml_authority::Call::dispatch_as {
				as_origin: AuthoritysOriginId::Treasury,
				call: Box::new(transfer_call.clone()),
			});

			let one_day_later = OneDay::get() + 1;

			assert_ok!(Authority::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(one_day_later),
				0,
				true,
				Box::new(treasury_reserve_call.clone())
			));

			assert_ok!(Authority::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(one_day_later),
				0,
				true,
				Box::new(call.clone())
			));
			System::assert_last_event(Event::Authority(orml_authority::Event::Scheduled(
				OriginCaller::Authority(DelayedOrigin {
					delay: one_day_later - 1,
					origin: Box::new(OriginCaller::system(RawOrigin::Root)),
				}),
				1,
			)));

			run_to_block(one_day_later);

			assert_eq!(
				Currencies::free_balance(USD_CURRENCY, &TreasuryPalletId::get().into_account()),
				500 * dollar(USD_CURRENCY)
			);
			assert_eq!(
				Currencies::free_balance(USD_CURRENCY, &AccountId::from(BOB)),
				500 * dollar(USD_CURRENCY)
			);

			// delay < SevenDays
			#[cfg(feature = "with-mandala-runtime")]
			System::assert_last_event(Event::Scheduler(pallet_scheduler::Event::<Runtime>::Dispatched(
				(OneDay::get() + 1, 1),
				Some([AUTHORITY_ORIGIN_ID, 64, 56, 0, 0, 0, 0, 1, 0, 0, 0].to_vec()),
				Err(DispatchError::BadOrigin),
			)));
			#[cfg(feature = "with-karura-runtime")]
			System::assert_last_event(Event::Scheduler(pallet_scheduler::Event::<Runtime>::Dispatched(
				(OneDay::get() + 1, 1),
				Some([AUTHORITY_ORIGIN_ID, 32, 28, 0, 0, 0, 0, 1, 0, 0, 0].to_vec()),
				Err(DispatchError::BadOrigin),
			)));
			#[cfg(feature = "with-acala-runtime")]
			System::assert_last_event(Event::Scheduler(pallet_scheduler::Event::<Runtime>::Dispatched(
				(OneDay::get() + 1, 1),
				Some([AUTHORITY_ORIGIN_ID, 32, 28, 0, 0, 0, 0, 1, 0, 0, 0].to_vec()),
				Err(DispatchError::BadOrigin),
			)));

			let seven_days_later = one_day_later + SevenDays::get() + 1;

			// delay = SevenDays
			assert_ok!(Authority::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(seven_days_later),
				0,
				true,
				Box::new(call.clone())
			));

			run_to_block(seven_days_later);

			#[cfg(feature = "with-mandala-runtime")]
			System::assert_last_event(Event::Scheduler(pallet_scheduler::Event::<Runtime>::Dispatched(
				(seven_days_later, 0),
				Some([AUTHORITY_ORIGIN_ID, 193, 137, 1, 0, 0, 0, 2, 0, 0, 0].to_vec()),
				Ok(()),
			)));

			#[cfg(feature = "with-karura-runtime")]
			System::assert_last_event(Event::Scheduler(pallet_scheduler::Event::<Runtime>::Dispatched(
				(seven_days_later, 0),
				Some([AUTHORITY_ORIGIN_ID, 225, 196, 0, 0, 0, 0, 2, 0, 0, 0].to_vec()),
				Ok(()),
			)));

			#[cfg(feature = "with-acala-runtime")]
			System::assert_last_event(Event::Scheduler(pallet_scheduler::Event::<Runtime>::Dispatched(
				(seven_days_later, 0),
				Some([AUTHORITY_ORIGIN_ID, 225, 196, 0, 0, 0, 0, 2, 0, 0, 0].to_vec()),
				Ok(()),
			)));

			// with_delayed_origin = false
			assert_ok!(Authority::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(seven_days_later + 1),
				0,
				false,
				Box::new(call.clone())
			));
			System::assert_last_event(Event::Authority(orml_authority::Event::Scheduled(
				OriginCaller::system(RawOrigin::Root),
				3,
			)));

			run_to_block(seven_days_later + 1);
			System::assert_last_event(Event::Scheduler(pallet_scheduler::Event::<Runtime>::Dispatched(
				(seven_days_later + 1, 0),
				Some([0, 0, 3, 0, 0, 0].to_vec()),
				Ok(()),
			)));

			assert_ok!(Authority::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(seven_days_later + 2),
				0,
				false,
				Box::new(call.clone())
			));

			// fast_track_scheduled_dispatch
			assert_ok!(Authority::fast_track_scheduled_dispatch(
				Origin::root(),
				Box::new(frame_system::RawOrigin::Root.into()),
				4,
				DispatchTime::At(seven_days_later + 3),
			));

			// delay_scheduled_dispatch
			assert_ok!(Authority::delay_scheduled_dispatch(
				Origin::root(),
				Box::new(frame_system::RawOrigin::Root.into()),
				4,
				4,
			));

			// cancel_scheduled_dispatch
			assert_ok!(Authority::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(seven_days_later + 2),
				0,
				true,
				Box::new(call.clone())
			));
			System::assert_last_event(Event::Authority(orml_authority::Event::Scheduled(
				OriginCaller::Authority(DelayedOrigin {
					delay: 1,
					origin: Box::new(OriginCaller::system(RawOrigin::Root)),
				}),
				5,
			)));

			let schedule_origin = {
				let origin: <Runtime as orml_authority::Config>::Origin = From::from(Origin::root());
				let origin: <Runtime as orml_authority::Config>::Origin = From::from(DelayedOrigin::<
					BlockNumber,
					<Runtime as orml_authority::Config>::PalletsOrigin,
				> {
					delay: 1,
					origin: Box::new(origin.caller().clone()),
				});
				origin
			};

			let pallets_origin = Box::new(schedule_origin.caller().clone());
			assert_ok!(Authority::cancel_scheduled_dispatch(Origin::root(), pallets_origin, 5));
			System::assert_last_event(Event::Authority(orml_authority::Event::Cancelled(
				OriginCaller::Authority(DelayedOrigin {
					delay: 1,
					origin: Box::new(OriginCaller::system(RawOrigin::Root)),
				}),
				5,
			)));

			assert_ok!(Authority::schedule_dispatch(
				Origin::root(),
				DispatchTime::At(seven_days_later + 3),
				0,
				false,
				Box::new(call.clone())
			));
			System::assert_last_event(Event::Authority(orml_authority::Event::Scheduled(
				OriginCaller::system(RawOrigin::Root),
				6,
			)));

			assert_ok!(Authority::cancel_scheduled_dispatch(
				Origin::root(),
				Box::new(frame_system::RawOrigin::Root.into()),
				6
			));
			System::assert_last_event(Event::Authority(orml_authority::Event::Cancelled(
				OriginCaller::system(RawOrigin::Root),
				6,
			)));
		});
}
