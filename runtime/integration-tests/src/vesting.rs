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
use orml_vesting::VestingSchedule;

#[test]
fn test_vesting_use_relaychain_block_number() {
	ExtBuilder::default().build().execute_with(|| {
		#[cfg(feature = "with-mandala-runtime")]
		let signer: AccountId = TreasuryPalletId::get().into_account_truncating();
		#[cfg(feature = "with-karura-runtime")]
		let signer: AccountId = KaruraFoundationAccounts::get()[0].clone();
		#[cfg(feature = "with-acala-runtime")]
		let signer: AccountId = AcalaFoundationAccounts::get()[0].clone();

		assert_ok!(Balances::set_balance(
			Origin::root(),
			signer.clone().into(),
			1_000 * dollar(ACA),
			0
		));

		assert_ok!(Vesting::vested_transfer(
			Origin::signed(signer),
			alice().into(),
			VestingSchedule {
				start: 10,
				period: 2,
				period_count: 5,
				per_period: 3 * dollar(NATIVE_CURRENCY),
			}
		));

		assert_eq!(Balances::free_balance(&alice()), 15 * dollar(NATIVE_CURRENCY));
		assert_eq!(Balances::usable_balance(&alice()), 0);

		set_relaychain_block_number(10);

		assert_ok!(Vesting::claim(Origin::signed(alice())));
		assert_eq!(Balances::usable_balance(&alice()), 0);

		set_relaychain_block_number(12);

		assert_ok!(Vesting::claim(Origin::signed(alice())));
		assert_eq!(Balances::usable_balance(&alice()), 3 * dollar(NATIVE_CURRENCY));

		set_relaychain_block_number(15);

		assert_ok!(Vesting::claim(Origin::signed(alice())));
		assert_eq!(Balances::usable_balance(&alice()), 6 * dollar(NATIVE_CURRENCY));

		set_relaychain_block_number(20);

		assert_ok!(Vesting::claim(Origin::signed(alice())));
		assert_eq!(Balances::usable_balance(&alice()), 15 * dollar(NATIVE_CURRENCY));

		set_relaychain_block_number(22);

		assert_ok!(Vesting::claim(Origin::signed(alice())));
		assert_eq!(Balances::usable_balance(&alice()), 15 * dollar(NATIVE_CURRENCY));
	});
}
