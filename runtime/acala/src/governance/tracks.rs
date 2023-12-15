// This file is part of Acala.

// Copyright (C) 2020-2023 Acala Foundation.
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

//! Track configurations for governance.

use super::*;
use pallet_referenda::Curve;
use sp_std::str::FromStr;

const fn percent(x: i32) -> sp_runtime::FixedI64 {
	sp_runtime::FixedI64::from_rational(x as u128, 100)
}
const fn permill(x: i32) -> sp_runtime::FixedI64 {
	sp_runtime::FixedI64::from_rational(x as u128, 1000)
}

lazy_static::lazy_static! {
static ref TRACKS_DATA: [(u16, pallet_referenda::TrackInfo<Balance, BlockNumber>); 5] = [
	(
		0,
		pallet_referenda::TrackInfo {
			// Name of this track.
			name: "root",
			// A limit for the number of referenda on this track that can be being decided at once.
			// For Root origin this should generally be just one.
			max_deciding: 1,
			// Amount that must be placed on deposit before a decision can be made.
			decision_deposit: 20_000 * dollar(ACA),
			// Amount of time this must be submitted for before a decision can be made.
			prepare_period: 2 * HOURS,
			// Amount of time that a decision may take to be approved prior to cancellation.
			decision_period: 14 * DAYS,
			// Amount of time that the approval criteria must hold before it can be approved.
			confirm_period: DAYS,
			// Minimum amount of time that an approved proposal must be in the dispatch queue.
			min_enactment_period: DAYS,
			// Minimum aye votes as percentage of overall conviction-weighted votes needed for
			// approval as a function of time into decision period.
			min_approval: Curve::make_reciprocal(4, 14, percent(80), percent(50), percent(100)),
			// Minimum pre-conviction aye-votes ("support") as percentage of overall population that
			// is needed for approval as a function of time into decision period.
			min_support: Curve::make_linear(14, 14, permill(1), percent(40)),
		},
	),
	(
		1,
		pallet_referenda::TrackInfo {
			name: "whitelisted_caller",
			max_deciding: 5,
			decision_deposit: 5_000 * dollar(ACA),
			prepare_period: 10 * MINUTES,
			decision_period: 10 * DAYS,
			confirm_period: 10 * MINUTES,
			min_enactment_period: 30 * MINUTES,
			min_approval: Curve::make_reciprocal(1, 10, percent(90), percent(50), percent(100)),
			min_support: Curve::make_reciprocal(1, 10, percent(1), percent(0), percent(5)),
		},
	),
	(
		2,
		pallet_referenda::TrackInfo {
			name: "general_admin",
			max_deciding: 10,
			decision_deposit: 1000 * dollar(ACA),
			prepare_period: HOURS,
			decision_period: 10 * DAYS,
			confirm_period: DAYS,
			min_enactment_period: DAYS,
			min_approval: Curve::make_reciprocal(4, 14, percent(80), percent(50), percent(100)),
			min_support: Curve::make_reciprocal(7, 14, percent(10), percent(0), percent(50)),
		},
	),
	(
		3,
		pallet_referenda::TrackInfo {
			name: "referendum_canceller",
			max_deciding: 20,
			decision_deposit: 2 * 1000 * dollar(ACA),
			prepare_period: HOURS,
			decision_period: 10 * DAYS,
			confirm_period: 3 * HOURS,
			min_enactment_period: 10 * MINUTES,
			min_approval: Curve::make_reciprocal(1, 10, percent(96), percent(50), percent(100)),
			min_support: Curve::make_reciprocal(1, 10, percent(1), percent(0), percent(10)),
		},
	),
	(
		4,
		pallet_referenda::TrackInfo {
			name: "referendum_killer",
			max_deciding: 20,
			decision_deposit: 4 * 1000 * dollar(ACA),
			prepare_period: HOURS,
			decision_period: 10 * DAYS,
			confirm_period: 3 * HOURS,
			min_enactment_period: 10 * MINUTES,
			min_approval: Curve::make_reciprocal(1, 10, percent(96), percent(50), percent(100)),
			min_support: Curve::make_reciprocal(1, 10, percent(1), percent(0), percent(10)),
		},
	),
];
}

pub struct TracksInfo;
impl pallet_referenda::TracksInfo<Balance, BlockNumber> for TracksInfo {
	type Id = u16;
	type RuntimeOrigin = <RuntimeOrigin as frame_support::traits::OriginTrait>::PalletsOrigin;
	fn tracks() -> &'static [(Self::Id, pallet_referenda::TrackInfo<Balance, BlockNumber>)] {
		&TRACKS_DATA[..]
	}
	fn track_for(id: &Self::RuntimeOrigin) -> Result<Self::Id, ()> {
		if let Ok(system_origin) = frame_system::RawOrigin::try_from(id.clone()) {
			match system_origin {
				frame_system::RawOrigin::Root => {
					if let Some((track_id, _)) = Self::tracks().iter().find(|(_, track)| track.name == "root") {
						Ok(*track_id)
					} else {
						Err(())
					}
				}
				_ => Err(()),
			}
		} else if let Ok(custom_origin) = custom_origins::Origin::try_from(id.clone()) {
			if let Some((track_id, _)) = Self::tracks().iter().find(|(_, track)| {
				if let Ok(track_custom_origin) = custom_origins::Origin::from_str(track.name) {
					track_custom_origin == custom_origin
				} else {
					false
				}
			}) {
				Ok(*track_id)
			} else {
				Err(())
			}
		} else {
			Err(())
		}
	}
}

#[test]
/// To ensure voters are always locked into their vote
fn vote_locking_always_longer_than_enactment_period() {
	for (_, track) in (&TRACKS_DATA).iter() {
		assert!(
			<Runtime as pallet_conviction_voting::Config>::VoteLockingPeriod::get() >= track.min_enactment_period,
			"Track {} has enactment period {} < vote locking period {}",
			track.name,
			track.min_enactment_period,
			<Runtime as pallet_conviction_voting::Config>::VoteLockingPeriod::get(),
		);
	}
}

#[test]
fn all_tracks_have_origins() {
	for (_, track) in (&TRACKS_DATA).iter() {
		// check name.into() is successful either converts into "root" or custom origin
		let track_is_root = track.name == "root";
		let track_has_custom_origin = custom_origins::Origin::from_str(track.name).is_ok();
		assert!(track_is_root || track_has_custom_origin);
	}
}
