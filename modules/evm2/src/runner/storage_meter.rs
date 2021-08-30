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

use evm::ExitError;
use frame_support::log;
use sp_core::H160;
use sp_runtime::{DispatchError, DispatchResult};

pub struct StorageMeter {
	limit: u32,
	total_used: u32,
	total_refunded: u32,
}

impl StorageMeter {
	/// Create a new storage_meter with given storage limit.
	pub fn new(limit: u32) -> Self {
		Self {
			limit,
			total_used: 0,
			total_refunded: 0,
		}
	}

	pub fn child_meter(&mut self) -> Self {
		let storage = self.available_storage();
		StorageMeter::new(storage)
	}

	pub fn storage_limit(&self) -> u32 {
		self.limit
	}

	pub fn total_used(&self) -> u32 {
		self.total_used
	}

	pub fn total_refunded(&self) -> u32 {
		self.total_refunded
	}

	pub fn available_storage(&self) -> u32 {
		self.limit
			.saturating_add(self.total_refunded)
			.saturating_sub(self.total_used)
	}

	pub fn used_storage(&self) -> i32 {
		if self.total_used > self.total_refunded {
			(self.total_used - self.total_refunded) as i32
		} else {
			-((self.total_refunded - self.total_used) as i32)
		}
	}

	pub fn finish(&self) -> Result<i32, ExitError> {
		log::trace!(
			target: "evm",
			"StorageMeter: finish: used {:?} refunded {:?}",
			self.total_used, self.total_refunded
		);
		if self.limit < self.total_used.saturating_sub(self.total_refunded) {
			Err(ExitError::Other("OutOfStorage".into()))
		} else {
			Ok(self.used_storage())
		}
	}

	pub fn charge(&mut self, storage: u32) {
		log::trace!(
			target: "evm",
			"StorageMeter: charge: storage {:?}",
			storage
		);
		self.total_used = self.total_used.saturating_add(storage);
	}

	pub fn uncharge(&mut self, storage: u32) {
		log::trace!(
			target: "evm",
			"StorageMeter: uncharge: storage {:?}",
			storage
		);
		self.total_used = self.total_used.saturating_sub(storage);
	}

	pub fn refund(&mut self, storage: u32) {
		log::trace!(
			target: "evm",
			"StorageMeter: refund: storage {:?}",
			storage
		);
		self.total_refunded = self.total_refunded.saturating_add(storage);
	}

	pub fn merge(&mut self, other: &Self) -> Result<i32, ExitError> {
		let storage = other.finish()?;
		if storage.is_positive() {
			self.charge(storage as u32);
		} else {
			self.refund(storage.abs() as u32);
		}
		Ok(storage)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::{assert_err, assert_ok};

	const ALICE: H160 = H160::repeat_byte(11);
	const CONTRACT: H160 = H160::repeat_byte(22);
	const CONTRACT_2: H160 = H160::repeat_byte(33);
	const CONTRACT_3: H160 = H160::repeat_byte(44);

	#[test]
	fn test_storage_with_limit_zero() {
		let mut storage_meter = StorageMeter::new(0);
		assert_eq!(storage_meter.available_storage(), 0);
		assert_eq!(storage_meter.storage_limit(), 0);

		// refund
		storage_meter.refund(1);
		assert_eq!(storage_meter.total_used(), 0);
		assert_eq!(storage_meter.total_refunded(), 1);
		assert_eq!(storage_meter.used_storage(), -1);
		assert_eq!(storage_meter.available_storage(), 1);

		// charge
		storage_meter.charge(1);
		assert_eq!(storage_meter.total_used(), 1);
		assert_eq!(storage_meter.total_refunded(), 1);
		assert_eq!(storage_meter.used_storage(), 0);
		assert_eq!(storage_meter.available_storage(), 0);

		// uncharge
		storage_meter.uncharge(1);
		assert_eq!(storage_meter.total_used(), 0);
		assert_eq!(storage_meter.total_refunded(), 1);
		assert_eq!(storage_meter.used_storage(), -1);
		assert_eq!(storage_meter.available_storage(), 1);

		// finish
		assert_eq!(storage_meter.finish(), Ok(-1));
	}

	#[test]
	fn test_out_of_storage() {
		let mut storage_meter = StorageMeter::new(1000);
		assert_eq!(storage_meter.available_storage(), 1000);

		storage_meter.charge(200);
		assert_eq!(storage_meter.finish(), Ok(200));

		storage_meter.charge(2000);
		assert_eq!(storage_meter.finish(), Err(ExitError::Other("OutOfStorage".into())));

		storage_meter.refund(2000);
		assert_eq!(storage_meter.finish(), Ok(200));
	}

	#[test]
	fn test_high_use_and_refund() {
		let mut storage_meter = StorageMeter::new(1000);
		assert_eq!(storage_meter.available_storage(), 1000);

		storage_meter.charge(1000);
		assert_eq!(storage_meter.available_storage(), 0);

		storage_meter.charge(100);
		assert_eq!(storage_meter.available_storage(), 0);
		storage_meter.refund(200);
		assert_eq!(storage_meter.available_storage(), 100);

		let child_meter = storage_meter.child_meter();
		assert_eq!(storage_meter.available_storage(), 100);

		assert_eq!(child_meter.finish(), Ok(0));
		assert_eq!(storage_meter.finish(), Ok(900));
	}

	#[test]
	fn test_child_meter() {
		let mut storage_meter = StorageMeter::new(1000);
		storage_meter.charge(100);

		let mut child_meter = storage_meter.child_meter();
		assert_eq!(child_meter.available_storage(), 900);

		child_meter.charge(100);
		assert_eq!(child_meter.available_storage(), 800);

		child_meter.refund(50);
		assert_eq!(child_meter.available_storage(), 850);

		let mut child_meter_2 = child_meter.child_meter();
		assert_eq!(child_meter_2.available_storage(), 850);

		child_meter_2.charge(20);
		assert_eq!(child_meter_2.available_storage(), 830);

		assert_eq!(child_meter_2.finish(), Ok(20));

		assert_eq!(child_meter.finish(), Ok(50));

		let mut child_meter_3 = storage_meter.child_meter();
		assert_eq!(child_meter_3.available_storage(), 900);

		child_meter_3.charge(30);
		assert_eq!(child_meter_3.available_storage(), 870);
		assert_eq!(child_meter_3.finish(), Ok(30));

		assert_eq!(storage_meter.available_storage(), 900);
		assert_eq!(storage_meter.finish(), Ok(100));
	}

	#[test]
	fn test_merge() {
		let mut storage_meter = StorageMeter::new(1000);
		storage_meter.charge(100);

		let mut child_meter = storage_meter.child_meter();
		assert_eq!(child_meter.available_storage(), 900);

		child_meter.charge(100);
		assert_eq!(child_meter.available_storage(), 800);

		child_meter.refund(50);
		assert_eq!(child_meter.available_storage(), 850);

		let mut child_meter_2 = child_meter.child_meter();
		assert_eq!(child_meter_2.available_storage(), 850);

		child_meter_2.charge(20);
		assert_eq!(child_meter_2.available_storage(), 830);

		assert_eq!(child_meter_2.finish(), Ok(20));

		assert_eq!(child_meter.finish(), Ok(50));
		assert_eq!(child_meter.merge(&child_meter_2), Ok(20));
		assert_eq!(child_meter.available_storage(), 830);

		let mut child_meter_3 = storage_meter.child_meter();
		assert_eq!(child_meter_3.available_storage(), 900);

		child_meter_3.charge(30);
		assert_eq!(child_meter_3.available_storage(), 870);
		assert_eq!(child_meter_3.finish(), Ok(30));
		assert_eq!(storage_meter.merge(&child_meter_3), Ok(30));

		assert_eq!(storage_meter.available_storage(), 870);
		assert_eq!(child_meter.finish(), Ok(70));
		assert_eq!(storage_meter.finish(), Ok(130));
		assert_eq!(storage_meter.merge(&child_meter), Ok(70));
		assert_eq!(storage_meter.available_storage(), 800);
	}
}
