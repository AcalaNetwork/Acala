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

use frame_support::parameter_types;
pub use module_support::{ExchangeRate, PrecompileCallerFilter, Price, Rate, Ratio};
use sp_runtime::{
	traits::{One, Saturating, Zero},
	FixedPointNumber, FixedPointOperand,
};

parameter_types! {
	pub FeeRateMatrix: [[Rate; 11]; 11] = [
		// when used_buffer_percent is 0%
		[
			Rate::zero(),
			Rate::saturating_from_rational(231487, 100000000), // when demand_in_available_percent is 10%
			Rate::saturating_from_rational(526013, 100000000), // 20%
			Rate::saturating_from_rational(106148, 10000000),  // 30%
			Rate::saturating_from_rational(243221, 10000000),  // 40%
			Rate::saturating_from_rational(597041, 10000000),  // 50%
			Rate::saturating_from_rational(126422, 1000000),   // 60%
			Rate::saturating_from_rational(214815, 1000000),   // 70%
			Rate::saturating_from_rational(311560, 1000000),   // 80%
			Rate::saturating_from_rational(410715, 1000000),   // 90%
			Rate::saturating_from_rational(510500, 1000000),   // 100%
		],
		// when used_buffer_percent is 10%
		[
			Rate::zero(),
			Rate::saturating_from_rational(260999, 100000000), // when demand_in_available_percent is 10%
			Rate::saturating_from_rational(584962, 100000000), // 20%
			Rate::saturating_from_rational(114942, 10000000),  // 30%
			Rate::saturating_from_rational(254703, 10000000),  // 40%
			Rate::saturating_from_rational(610531, 10000000),  // 50%
			Rate::saturating_from_rational(127866, 1000000),   // 60%
			Rate::saturating_from_rational(216285, 1000000),   // 70%
			Rate::saturating_from_rational(313035, 1000000),   // 80%
			Rate::saturating_from_rational(412191, 1000000),   // 90%
			Rate::saturating_from_rational(511976, 1000000),   // 100%
		],
		// when used_buffer_percent is 20%
		[
			Rate::zero(),
			Rate::saturating_from_rational(376267, 100000000), // when demand_in_available_percent is 10%
			Rate::saturating_from_rational(815202, 100000000), // 20%
			Rate::saturating_from_rational(149288, 10000000),  // 30%
			Rate::saturating_from_rational(299546, 10000000),  // 40%
			Rate::saturating_from_rational(663214, 10000000),  // 50%
			Rate::saturating_from_rational(133503, 1000000),   // 60%
			Rate::saturating_from_rational(222025, 1000000),   // 70%
			Rate::saturating_from_rational(318797, 1000000),   // 80%
			Rate::saturating_from_rational(417955, 1000000),   // 90%
			Rate::saturating_from_rational(517741, 1000000),   // 100%
		],
		// when used_buffer_percent is 30%
		[
			Rate::zero(),
			Rate::saturating_from_rational(807626, 100000000), // when demand_in_available_percent is 10%
			Rate::saturating_from_rational(167679, 10000000),  // 20%
			Rate::saturating_from_rational(277809, 10000000),  // 30%
			Rate::saturating_from_rational(467319, 10000000),  // 40%
			Rate::saturating_from_rational(860304, 10000000),  // 50%
			Rate::saturating_from_rational(154595, 1000000),   // 60%
			Rate::saturating_from_rational(243507, 1000000),   // 70%
			Rate::saturating_from_rational(340357, 1000000),   // 80%
			Rate::saturating_from_rational(439528, 1000000),   // 90%
			Rate::saturating_from_rational(539315, 1000000),   // 100%
		],
		// when used_buffer_percent is 40%
		[
			Rate::zero(),
			Rate::saturating_from_rational(219503, 10000000), // when demand_in_available_percent is 10%
			Rate::saturating_from_rational(444770, 10000000), // 20%
			Rate::saturating_from_rational(691029, 10000000), // 30%
			Rate::saturating_from_rational(100646, 1000000),  // 40%
			Rate::saturating_from_rational(149348, 1000000),  // 50%
			Rate::saturating_from_rational(222388, 1000000),  // 60%
			Rate::saturating_from_rational(312586, 1000000),  // 70%
			Rate::saturating_from_rational(409701, 1000000),  // 80%
			Rate::saturating_from_rational(508916, 1000000),  // 90%
			Rate::saturating_from_rational(608707, 1000000),  // 100%
		],
		// when used_buffer_percent is 50%
		[
			Rate::zero(),
			Rate::saturating_from_rational(511974, 10000000), // when demand_in_available_percent is 10%
			Rate::saturating_from_rational(102871, 1000000),  // 20%
			Rate::saturating_from_rational(156110, 1000000),  // 30%
			Rate::saturating_from_rational(213989, 1000000),  // 40%
			Rate::saturating_from_rational(282343, 1000000),  // 50%
			Rate::saturating_from_rational(364989, 1000000),  // 60%
			Rate::saturating_from_rational(458110, 1000000),  // 70%
			Rate::saturating_from_rational(555871, 1000000),  // 80%
			Rate::saturating_from_rational(655197, 1000000),  // 90%
			Rate::saturating_from_rational(755000, 1000000),  // 100%
		],
		// when used_buffer_percent is 60%
		[
			Rate::zero(),
			Rate::saturating_from_rational(804354, 10000000), // when demand_in_available_percent is 10%
			Rate::saturating_from_rational(161193, 1000000),  // 20%
			Rate::saturating_from_rational(242816, 1000000),  // 30%
			Rate::saturating_from_rational(326520, 1000000),  // 40%
			Rate::saturating_from_rational(414156, 1000000),  // 50%
			Rate::saturating_from_rational(506779, 1000000),  // 60%
			Rate::saturating_from_rational(603334, 1000000),  // 70%
			Rate::saturating_from_rational(701969, 1000000),  // 80%
			Rate::saturating_from_rational(801470, 1000000),  // 90%
			Rate::saturating_from_rational(901293, 1000000),  // 100%
		],
		// when used_buffer_percent is 70%
		[
			Rate::zero(),
			Rate::saturating_from_rational(942895, 10000000), // when demand_in_available_percent is 10%
			Rate::saturating_from_rational(188758, 1000000),  // 20%
			Rate::saturating_from_rational(283590, 1000000),  // 30%
			Rate::saturating_from_rational(379083, 1000000),  // 40%
			Rate::saturating_from_rational(475573, 1000000),  // 50%
			Rate::saturating_from_rational(573220, 1000000),  // 60%
			Rate::saturating_from_rational(671864, 1000000),  // 70%
			Rate::saturating_from_rational(771169, 1000000),  // 80%
			Rate::saturating_from_rational(870838, 1000000),  // 90%
			Rate::saturating_from_rational(970685, 1000000),  // 100%
		],
		// when used_buffer_percent is 80%
		[
			Rate::zero(),
			Rate::saturating_from_rational(985811, 10000000), // when demand_in_available_percent is 10%
			Rate::saturating_from_rational(197241, 1000000),  // 20%
			Rate::saturating_from_rational(296017, 1000000),  // 30%
			Rate::saturating_from_rational(394949, 1000000),  // 40%
			Rate::saturating_from_rational(494073, 1000000),  // 50%
			Rate::saturating_from_rational(593401, 1000000),  // 60%
			Rate::saturating_from_rational(692920, 1000000),  // 70%
			Rate::saturating_from_rational(792596, 1000000),  // 80%
			Rate::saturating_from_rational(892388, 1000000),  // 90%
			Rate::saturating_from_rational(992259, 1000000),  // 100%
		],
		// when used_buffer_percent is 90%
		[
			Rate::zero(),
			Rate::saturating_from_rational(997132, 10000000), // when demand_in_available_percent is 10%
			Rate::saturating_from_rational(199444, 1000000),  // 20%
			Rate::saturating_from_rational(299194, 1000000),  // 30%
			Rate::saturating_from_rational(398965, 1000000),  // 40%
			Rate::saturating_from_rational(498757, 1000000),  // 50%
			Rate::saturating_from_rational(598570, 1000000),  // 60%
			Rate::saturating_from_rational(698404, 1000000),  // 70%
			Rate::saturating_from_rational(798259, 1000000),  // 80%
			Rate::saturating_from_rational(898132, 1000000),  // 90%
			Rate::saturating_from_rational(998024, 1000000),  // 100%
		],
		// when used_buffer_percent is 100%
		[
			Rate::zero(),
			Rate::one(), // when demand_in_available_percent is 10%
			Rate::one(),  // 20%
			Rate::one(),  // 30%
			Rate::one(),  // 40%
			Rate::one(),  // 50%
			Rate::one(),  // 60%
			Rate::one(),  // 70%
			Rate::one(),  // 80%
			Rate::one(),  // 90%
			Rate::one(),  // 100%
		],
	];
}

pub struct CurveFeeModel;
impl<Balance: FixedPointOperand> module_staking_pool::FeeModel<Balance> for CurveFeeModel {
	/// The parameter `base_rate` does not work in this fee model, base fee is
	/// fixed at 2%
	fn get_fee(
		remain_available_percent: Ratio,
		available_amount: Balance,
		request_amount: Balance,
		_base_rate: Rate,
	) -> Option<Balance> {
		if remain_available_percent.is_zero()
			|| remain_available_percent > Ratio::one()
			|| request_amount > available_amount
			|| request_amount.is_zero()
		{
			return None;
		}

		let ten = Ratio::saturating_from_rational(10, 1);

		// x , [0, 100%)
		let used_buffer_percent = Ratio::one().saturating_sub(remain_available_percent);
		// y  [0, 100%]
		let demand_in_available_percent = Ratio::saturating_from_rational(request_amount, available_amount);

		// x0 [0, 9]
		let x = used_buffer_percent.saturating_mul(ten);
		let x0 = x
			.into_inner()
			.checked_div(Ratio::accuracy())
			.expect("panics only if accuracy is zero, accuracy is not zero; qed") as usize;
		let prefix_x: Ratio = x.frac();

		// y0 [0, 10]
		let y = demand_in_available_percent.saturating_mul(ten);
		let mut y0 = y
			.into_inner()
			.checked_div(Ratio::accuracy())
			.expect("panics only if accuracy is zero, accuracy is not zero; qed") as usize;
		let mut prefix_y: Ratio = y.frac();

		let multiplier = if prefix_x.is_zero() && prefix_y.is_zero() {
			FeeRateMatrix::get()[x0][y0]
		} else {
			if y0 == 10 {
				y0 -= 1;
				prefix_y = prefix_y.saturating_add(Ratio::saturating_from_rational(10, 100));
			}

			let x0_y0_rate = FeeRateMatrix::get()[x0][y0];
			let x0_y1_rate = FeeRateMatrix::get()[x0][y0 + 1];
			let x1_y0_rate = FeeRateMatrix::get()[x0 + 1][y0];
			let x1_y1_rate = FeeRateMatrix::get()[x0 + 1][y0 + 1];
			let y0_x = prefix_x
				.saturating_mul(x1_y0_rate.saturating_sub(x0_y0_rate))
				.saturating_add(x0_y0_rate);
			let y1_x = prefix_x
				.saturating_mul(x1_y1_rate.saturating_sub(x0_y1_rate))
				.saturating_add(x0_y1_rate);

			y1_x.saturating_sub(y0_x).saturating_mul(prefix_y).saturating_add(y0_x)
		};

		multiplier.checked_mul_int(available_amount)
	}
}
