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

use crate::{
	AcalaOracle, AccountId, Balance, Currencies, CurrencyId, MinimumCount, OperatorMembershipAcala, Price, Runtime,
};

use frame_benchmarking::account;
use frame_support::{assert_ok, traits::Contains};
use frame_system::RawOrigin;
use module_support::Erc20InfoMapping;
use orml_traits::MultiCurrencyExtended;
use sp_runtime::{
	traits::{SaturatedConversion, StaticLookup},
	DispatchResult,
};
use sp_std::prelude::*;

pub fn lookup_of_account(who: AccountId) -> <<Runtime as frame_system::Config>::Lookup as StaticLookup>::Source {
	<Runtime as frame_system::Config>::Lookup::unlookup(who)
}

pub fn set_balance(currency_id: CurrencyId, who: &AccountId, balance: Balance) {
	assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id,
		who,
		balance.saturated_into()
	));
}

pub fn feed_price(prices: Vec<(CurrencyId, Price)>) -> DispatchResult {
	for i in 0..MinimumCount::get() {
		let oracle: AccountId = account("oracle", 0, i);
		if !OperatorMembershipAcala::contains(&oracle) {
			OperatorMembershipAcala::add_member(RawOrigin::Root.into(), oracle.clone())?;
		}
		AcalaOracle::feed_values(RawOrigin::Signed(oracle).into(), prices.to_vec())
			.map_or_else(|e| Err(e.error), |_| Ok(()))?;
	}

	Ok(())
}

pub fn dollar(currency_id: CurrencyId) -> Balance {
	if let Some(decimals) = module_asset_registry::EvmErc20InfoMapping::<Runtime>::decimals(currency_id) {
		10u128.saturating_pow(decimals.into())
	} else {
		panic!("{:?} not support decimals", currency_id);
	}
}

#[cfg(test)]
pub mod tests {
	pub fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::default()
			.build_storage::<crate::Runtime>()
			.unwrap()
			.into()
	}
}
