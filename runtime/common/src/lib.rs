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

//! Common runtime code for Acala, Karura and Mandala.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	parameter_types,
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, WEIGHT_PER_SECOND},
		DispatchClass, Weight,
	},
};
use frame_system::limits;
pub use module_support::{ExchangeRate, PrecompileCallerFilter, Price, Rate, Ratio};
use primitives::{Balance, CurrencyId, PRECOMPILE_ADDRESS_START, PREDEPLOY_ADDRESS_START};
use sp_core::H160;
use sp_runtime::{
	traits::{Convert, Saturating},
	transaction_validity::TransactionPriority,
	FixedPointNumber, FixedPointOperand, Perbill,
};
use static_assertions::const_assert;

pub mod precompile;
pub use precompile::{
	AllPrecompiles, DexPrecompile, MultiCurrencyPrecompile, NFTPrecompile, OraclePrecompile, ScheduleCallPrecompile,
	StateRentPrecompile,
};
pub use primitives::currency::{
	GetDecimals, ACA, AUSD, DOT, KAR, KILT, KSM, KUSD, LDOT, LKSM, PHA, PLM, POLKABTC, RENBTC, SDN, XBTC,
};

pub type TimeStampedPrice = orml_oracle::TimestampedValue<Price, primitives::Moment>;

// Priority of unsigned transactions
parameter_types! {
	pub const StakingUnsignedPriority: TransactionPriority = TransactionPriority::max_value() / 2;
	pub const RenvmBridgeUnsignedPriority: TransactionPriority = TransactionPriority::max_value() / 2;
	pub const CdpEngineUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
	pub const AuctionManagerUnsignedPriority: TransactionPriority = TransactionPriority::max_value() - 1;
}

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

pub const SYSTEM_CONTRACT_LEADING_ZERO_BYTES: usize = 12;

/// Check if the given `address` is a system contract.
///
/// It's system contract if the address starts with 12 zero bytes.
pub fn is_system_contract(address: H160) -> bool {
	address[..SYSTEM_CONTRACT_LEADING_ZERO_BYTES] == [0u8; SYSTEM_CONTRACT_LEADING_ZERO_BYTES]
}

pub fn is_acala_precompile(address: H160) -> bool {
	address >= H160::from_low_u64_be(PRECOMPILE_ADDRESS_START)
		&& address < H160::from_low_u64_be(PREDEPLOY_ADDRESS_START)
}

/// The call is allowed only if caller is a system contract.
pub struct SystemContractsFilter;
impl PrecompileCallerFilter for SystemContractsFilter {
	fn is_allowed(caller: H160) -> bool {
		is_system_contract(caller)
	}
}

/// Convert gas to weight
pub struct GasToWeight;
impl Convert<u64, Weight> for GasToWeight {
	fn convert(a: u64) -> u64 {
		a as Weight
	}
}

pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_perthousand(25);
/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be
/// used by  Operational  extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 2 seconds of compute with a 6 second average block time.
pub const MAXIMUM_BLOCK_WEIGHT: Weight = 2 * WEIGHT_PER_SECOND;

const_assert!(NORMAL_DISPATCH_RATIO.deconstruct() >= AVERAGE_ON_INITIALIZE_RATIO.deconstruct());

parameter_types! {
	/// Maximum length of block. Up to 5MB.
	pub RuntimeBlockLength: limits::BlockLength =
		limits::BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	/// Block weights base values and limits.
	pub RuntimeBlockWeights: limits::BlockWeights = limits::BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have an extra reserved space, so that they
			// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
			weights.reserved = Some(
				MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT,
			);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();
}

parameter_types! {
	/// A limit for off-chain phragmen unsigned solution submission.
	///
	/// We want to keep it as high as possible, but can't risk having it reject,
	/// so we always subtract the base block execution weight.
	pub OffchainSolutionWeightLimit: Weight = RuntimeBlockWeights::get()
		.get(DispatchClass::Normal)
		.max_extrinsic
		.expect("Normal extrinsics have weight limit configured by default; qed")
		.saturating_sub(BlockExecutionWeight::get());
}

pub struct RelaychainValidatorFilter;
impl<AccountId> orml_traits::Contains<AccountId> for RelaychainValidatorFilter {
	fn contains(_: &AccountId) -> bool {
		true
	}
}

pub fn dollar(currency_id: CurrencyId) -> Balance {
	10u128.saturating_pow(currency_id.decimals())
}

pub fn cent(currency_id: CurrencyId) -> Balance {
	dollar(currency_id) / 100
}

pub fn millicent(currency_id: CurrencyId) -> Balance {
	cent(currency_id) / 1000
}

pub fn microcent(currency_id: CurrencyId) -> Balance {
	millicent(currency_id) / 1000
}

pub fn deposit(items: u32, bytes: u32, currency_id: CurrencyId) -> Balance {
	items as Balance * 15 * cent(currency_id) + (bytes as Balance) * 6 * cent(currency_id)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn system_contracts_filter_works() {
		assert!(SystemContractsFilter::is_allowed(H160::from_low_u64_be(1)));

		let mut max_allowed_addr = [0u8; 20];
		max_allowed_addr[SYSTEM_CONTRACT_LEADING_ZERO_BYTES] = 127u8;
		assert!(SystemContractsFilter::is_allowed(max_allowed_addr.into()));

		let mut min_blocked_addr = [0u8; 20];
		min_blocked_addr[SYSTEM_CONTRACT_LEADING_ZERO_BYTES - 1] = 1u8;
		assert!(!SystemContractsFilter::is_allowed(min_blocked_addr.into()));
	}

	#[test]
	fn is_system_contract_works() {
		assert!(is_system_contract(H160::from_low_u64_be(0)));
		assert!(is_system_contract(H160::from_low_u64_be(u64::max_value())));

		let mut bytes = [0u8; 20];
		bytes[SYSTEM_CONTRACT_LEADING_ZERO_BYTES - 1] = 1u8;

		assert!(!is_system_contract(bytes.into()));

		bytes = [0u8; 20];
		bytes[0] = 1u8;

		assert!(!is_system_contract(bytes.into()));
	}

	#[test]
	fn is_acala_precompile_works() {
		assert!(!is_acala_precompile(H160::from_low_u64_be(0)));
		assert!(!is_acala_precompile(H160::from_low_u64_be(
			PRECOMPILE_ADDRESS_START - 1
		)));
		assert!(is_acala_precompile(H160::from_low_u64_be(PRECOMPILE_ADDRESS_START)));
		assert!(is_acala_precompile(H160::from_low_u64_be(PREDEPLOY_ADDRESS_START - 1)));
		assert!(!is_acala_precompile(H160::from_low_u64_be(PREDEPLOY_ADDRESS_START)));
		assert!(!is_acala_precompile([1u8; 20].into()));
	}
}
