//! Common runtime code for Acala and Karura.

#![cfg_attr(not(feature = "std"), no_std)]

pub use module_support::{ExchangeRate, Price, Rate, Ratio};
use sp_runtime::{traits::Saturating, FixedPointNumber};

pub type TimeStampedPrice = orml_oracle::TimestampedValue<Price, primitives::Moment>;

pub struct CurveFeeModel;
impl module_staking_pool::FeeModel for CurveFeeModel {
	fn get_fee_rate(remain_available_percent: Ratio, demand_in_available_percent: Ratio, base_rate: Rate) -> Rate {
		let fee_rate_matrix: [[Rate; 10]; 9] = [
			// when used_buffer_percent is 10%
			[
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
		];

		let used_buffer_percent = Ratio::one().saturating_sub(remain_available_percent);

		// demand_in_available_percent

		Rate::one()
			.saturating_sub(base_rate)
			.saturating_mul(Rate::one().saturating_sub(remain_available_percent))
			.saturating_mul(demand_in_available_percent)
			.saturating_add(base_rate)
	}
}
