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

use super::xcm_config::*;
use super::{
	constants::{fee::*, parachains},
	FixedRateOfForeignAsset, Runtime, TransactionFeePoolTrader,
};
use xcm::latest::prelude::*;

parameter_types! {
	pub BncPerSecondOfCanonicalLocation: (AssetId, u128) = (
		MultiLocation::new(
			0,
			X1(GeneralKey(parachains::bifrost::BNC_KEY.to_vec())),
		).into(),
		ksm_per_second() * 80
	);
}

pub type Trader = (
	TransactionFeePoolTrader<Runtime, CurrencyIdConvert, KarPerSecondAsBased, ToTreasury>,
	FixedRateOfFungible<KsmPerSecond, ToTreasury>,
	FixedRateOfFungible<KusdPerSecond, ToTreasury>,
	FixedRateOfFungible<KarPerSecond, ToTreasury>,
	FixedRateOfFungible<LksmPerSecond, ToTreasury>,
	FixedRateOfFungible<BncPerSecond, ToTreasury>,
	FixedRateOfFungible<BncPerSecondOfCanonicalLocation, ToTreasury>,
	FixedRateOfFungible<VsksmPerSecond, ToTreasury>,
	FixedRateOfFungible<PHAPerSecond, ToTreasury>,
	FixedRateOfFungible<KbtcPerSecond, ToTreasury>,
	FixedRateOfFungible<KintPerSecond, ToTreasury>,
	FixedRateOfForeignAsset<Runtime, ForeignAssetUnitsPerSecond, ToTreasury>,
);
