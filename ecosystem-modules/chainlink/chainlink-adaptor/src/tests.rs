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

//! Unit tests for the chainlink adaptor module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{Event, *};
use sp_runtime::traits::BadOrigin;

#[test]
fn mapping_feed_id_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_eq!(ChainlinkAdaptor::feed_id_mapping(DOT), None);
		assert_eq!(ChainlinkFeed::feed_config(0), None);

		assert_noop!(
			ChainlinkAdaptor::mapping_feed_id(Origin::signed(ALICE), 0, DOT),
			BadOrigin,
		);

		assert_noop!(
			ChainlinkAdaptor::mapping_feed_id(Origin::signed(RegistorOrigin::get()), 0, DOT),
			Error::<Runtime>::InvalidFeedId,
		);

		assert_ok!(ChainlinkFeed::create_feed(
			Origin::signed(ALICE),
			20,
			10,
			(10, 1000),
			1,
			0,
			b"dotusd".to_vec(),
			0,
			vec![(ALICE, ALICE)],
			None,
			None,
		));
		assert_eq!(ChainlinkFeed::feed_config(0).is_some(), true);

		assert_ok!(ChainlinkAdaptor::mapping_feed_id(
			Origin::signed(RegistorOrigin::get()),
			0,
			DOT
		));
		System::assert_last_event(Event::ChainlinkAdaptor(crate::Event::MappingFeedId(0, DOT)));
		assert_eq!(ChainlinkAdaptor::feed_id_mapping(DOT), Some(0));

		assert_noop!(
			ChainlinkAdaptor::mapping_feed_id(Origin::signed(RegistorOrigin::get()), 1, DOT),
			Error::<Runtime>::CurrencyIdAlreadyMapping,
		);
	});
}

#[test]
fn unmapping_feed_id_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_ok!(ChainlinkFeed::create_feed(
			Origin::signed(ALICE),
			20,
			10,
			(10, 1000),
			1,
			0,
			b"dotusd".to_vec(),
			0,
			vec![(ALICE, ALICE)],
			None,
			None,
		));
		assert_ok!(ChainlinkAdaptor::mapping_feed_id(
			Origin::signed(RegistorOrigin::get()),
			0,
			DOT
		));
		assert_eq!(ChainlinkAdaptor::feed_id_mapping(DOT), Some(0));

		assert_noop!(
			ChainlinkAdaptor::unmapping_feed_id(Origin::signed(ALICE), DOT),
			BadOrigin,
		);

		assert_ok!(ChainlinkAdaptor::unmapping_feed_id(
			Origin::signed(RegistorOrigin::get()),
			DOT
		));
		System::assert_last_event(Event::ChainlinkAdaptor(crate::Event::UnmappingFeedId(0, DOT)));
		assert_eq!(ChainlinkAdaptor::feed_id_mapping(DOT), None);
	});
}

#[test]
fn data_provider_get_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(ChainlinkFeed::create_feed(
			Origin::signed(ALICE),
			20,
			10,
			(0, u128::MAX),
			1,
			0,
			b"dotusd".to_vec(),
			0,
			vec![(ALICE, ALICE), (BOB, BOB)],
			None,
			None,
		));
		assert_ok!(ChainlinkAdaptor::mapping_feed_id(
			Origin::signed(RegistorOrigin::get()),
			0,
			DOT
		));

		Timestamp::set_timestamp(10000);
		assert_ok!(ChainlinkFeed::submit(
			Origin::signed(ALICE),
			0,
			1,
			40_000_000_000_000_000_000u128,
		));
		assert_eq!(ChainlinkAdaptor::last_updated_timestamp(0), 10000);
		assert_eq!(
			ChainlinkAdaptor::get(&DOT),
			Some(Price::from_inner(40_000_000_000_000_000_000u128))
		);

		Timestamp::set_timestamp(20000);
		assert_ok!(ChainlinkFeed::submit(
			Origin::signed(BOB),
			0,
			1,
			50_000_000_000_000_000_000u128,
		));
		assert_eq!(ChainlinkAdaptor::last_updated_timestamp(0), 20000);
		assert_eq!(
			ChainlinkAdaptor::get(&DOT),
			Some(Price::from_inner(45_000_000_000_000_000_000u128))
		);
	});
}

#[test]
fn data_provider_get_no_op_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(ChainlinkFeed::create_feed(
			Origin::signed(ALICE),
			20,
			10,
			(0, u128::MAX),
			1,
			0,
			b"dotusd".to_vec(),
			0,
			vec![(ALICE, ALICE), (BOB, BOB)],
			None,
			None,
		));
		assert_ok!(ChainlinkAdaptor::mapping_feed_id(
			Origin::signed(RegistorOrigin::get()),
			0,
			DOT
		));

		Timestamp::set_timestamp(10000);
		assert_ok!(ChainlinkFeed::submit(
			Origin::signed(ALICE),
			0,
			1,
			40_000_000_000_000_000_000u128,
		));
		assert_eq!(
			ChainlinkAdaptor::get_no_op(&DOT),
			Some(TimestampedValue {
				value: Price::from_inner(40_000_000_000_000_000_000u128),
				timestamp: 10000,
			})
		);

		Timestamp::set_timestamp(20000);
		assert_ok!(ChainlinkFeed::submit(
			Origin::signed(BOB),
			0,
			1,
			50_000_000_000_000_000_000u128,
		));
		assert_eq!(
			ChainlinkAdaptor::get_no_op(&DOT),
			Some(TimestampedValue {
				value: Price::from_inner(45_000_000_000_000_000_000u128),
				timestamp: 20000,
			})
		);

		Timestamp::set_timestamp(30000);
		assert_ok!(ChainlinkFeed::submit(
			Origin::signed(ALICE),
			0,
			2,
			10_000_000_000_000_000_000u128,
		));
		assert_eq!(
			ChainlinkAdaptor::get_no_op(&DOT),
			Some(TimestampedValue {
				value: Price::from_inner(10_000_000_000_000_000_000u128),
				timestamp: 30000,
			})
		);
	});
}

#[test]
fn data_provider_get_all_values_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(ChainlinkFeed::create_feed(
			Origin::signed(ALICE),
			20,
			10,
			(0, u128::MAX),
			1,
			0,
			b"dotusd".to_vec(),
			0,
			vec![(ALICE, ALICE), (BOB, BOB)],
			None,
			None,
		));
		assert_ok!(ChainlinkFeed::create_feed(
			Origin::signed(ALICE),
			20,
			10,
			(0, u128::MAX),
			1,
			0,
			b"ksmusd".to_vec(),
			0,
			vec![(ALICE, ALICE), (BOB, BOB)],
			None,
			None,
		));
		assert_ok!(ChainlinkAdaptor::mapping_feed_id(
			Origin::signed(RegistorOrigin::get()),
			0,
			DOT
		));
		assert_ok!(ChainlinkAdaptor::mapping_feed_id(
			Origin::signed(RegistorOrigin::get()),
			1,
			KSM
		));

		Timestamp::set_timestamp(10000);
		assert_ok!(ChainlinkFeed::submit(
			Origin::signed(ALICE),
			0,
			1,
			40_000_000_000_000_000_000u128,
		));
		assert_eq!(
			ChainlinkAdaptor::get_all_values(),
			vec![
				(
					KSM,
					Some(TimestampedValue {
						value: Default::default(),
						timestamp: Default::default(),
					})
				),
				(
					DOT,
					Some(TimestampedValue {
						value: Price::from_inner(40_000_000_000_000_000_000u128),
						timestamp: 10000,
					})
				),
			]
		);

		Timestamp::set_timestamp(20000);
		assert_ok!(ChainlinkFeed::submit(
			Origin::signed(BOB),
			1,
			1,
			400_000_000_000_000_000_000u128,
		));
		assert_eq!(
			ChainlinkAdaptor::get_all_values(),
			vec![
				(
					KSM,
					Some(TimestampedValue {
						value: Price::from_inner(400_000_000_000_000_000_000u128),
						timestamp: 20000,
					})
				),
				(
					DOT,
					Some(TimestampedValue {
						value: Price::from_inner(40_000_000_000_000_000_000u128),
						timestamp: 10000,
					})
				),
			]
		);

		Timestamp::set_timestamp(30000);
		assert_ok!(ChainlinkFeed::submit(
			Origin::signed(BOB),
			0,
			1,
			50_000_000_000_000_000_000u128,
		));
		assert_eq!(
			ChainlinkAdaptor::get_all_values(),
			vec![
				(
					KSM,
					Some(TimestampedValue {
						value: Price::from_inner(400_000_000_000_000_000_000u128),
						timestamp: 20000,
					})
				),
				(
					DOT,
					Some(TimestampedValue {
						value: Price::from_inner(45_000_000_000_000_000_000u128),
						timestamp: 30000,
					})
				),
			]
		);
	});
}
