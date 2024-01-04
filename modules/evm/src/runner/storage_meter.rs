// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

#[derive(Default, Clone, Debug)]
pub struct StorageMeter {
	limit: u32,
	used: u32,
	refunded: u32,
	// save storage of children
	child_used: u32,
	child_refunded: u32,
}

impl StorageMeter {
	/// Create a new storage_meter with given storage limit.
	pub fn new(limit: u32) -> Self {
		Self {
			limit,
			used: 0,
			refunded: 0,
			child_used: 0,
			child_refunded: 0,
		}
	}

	pub fn child_meter(&mut self) -> Self {
		let storage = self.available_storage();
		StorageMeter::new(storage)
	}

	pub fn storage_limit(&self) -> u32 {
		self.limit
	}

	pub fn used(&self) -> u32 {
		self.used
	}

	pub fn refunded(&self) -> u32 {
		self.refunded
	}

	pub fn total_used(&self) -> u32 {
		self.used.saturating_add(self.child_used)
	}

	pub fn total_refunded(&self) -> u32 {
		self.refunded.saturating_add(self.child_refunded)
	}

	pub fn available_storage(&self) -> u32 {
		self.limit
			.saturating_add(self.refunded)
			.saturating_add(self.child_refunded)
			.saturating_sub(self.used)
			.saturating_sub(self.child_used)
	}

	pub fn used_storage(&self) -> i32 {
		if self.used > self.refunded {
			(self.used - self.refunded) as i32
		} else {
			-((self.refunded - self.used) as i32)
		}
	}

	pub fn finish(&self) -> Option<i32> {
		let total_used = self.total_used();
		let total_refunded = self.total_refunded();
		log::trace!(
			target: "evm",
			"StorageMeter: finish: used {:?} refunded {:?}",
			total_used, total_refunded
		);
		if self.limit < total_used.saturating_sub(total_refunded) {
			// OutOfStorage
			return None;
		}

		if total_used > total_refunded {
			Some((total_used - total_refunded) as i32)
		} else {
			Some(-((total_refunded - total_used) as i32))
		}
	}

	pub fn charge(&mut self, storage: u32) {
		log::trace!(
			target: "evm",
			"StorageMeter: charge: storage {:?}",
			storage
		);
		self.used = self.used.saturating_add(storage);
	}

	pub fn uncharge(&mut self, storage: u32) {
		log::trace!(
			target: "evm",
			"StorageMeter: uncharge: storage {:?}",
			storage
		);
		self.used = self.used.saturating_sub(storage);
	}

	pub fn refund(&mut self, storage: u32) {
		log::trace!(
			target: "evm",
			"StorageMeter: refund: storage {:?}",
			storage
		);
		self.refunded = self.refunded.saturating_add(storage);
	}

	pub fn merge(&mut self, other: &Self) {
		self.child_used = self.child_used.saturating_add(other.total_used());
		self.child_refunded = self.child_refunded.saturating_add(other.total_refunded());
	}
}

#[cfg(test)]
mod tests {
	use super::*;

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
		assert_eq!(storage_meter.finish(), Some(-1));
	}

	#[test]
	fn test_out_of_storage() {
		let mut storage_meter = StorageMeter::new(1000);
		assert_eq!(storage_meter.available_storage(), 1000);

		storage_meter.charge(200);
		assert_eq!(storage_meter.finish(), Some(200));

		storage_meter.charge(2000);
		assert_eq!(storage_meter.finish(), None);

		storage_meter.refund(2000);
		assert_eq!(storage_meter.finish(), Some(200));
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

		assert_eq!(child_meter.finish(), Some(0));
		assert_eq!(storage_meter.finish(), Some(900));
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

		assert_eq!(child_meter_2.finish(), Some(20));

		assert_eq!(child_meter.finish(), Some(50));

		let mut child_meter_3 = storage_meter.child_meter();
		assert_eq!(child_meter_3.available_storage(), 900);

		child_meter_3.charge(30);
		assert_eq!(child_meter_3.available_storage(), 870);
		assert_eq!(child_meter_3.finish(), Some(30));

		assert_eq!(storage_meter.available_storage(), 900);
		assert_eq!(storage_meter.finish(), Some(100));
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

		assert_eq!(child_meter_2.finish(), Some(20));

		assert_eq!(child_meter.finish(), Some(50));
		child_meter.merge(&child_meter_2);
		assert_eq!(child_meter.available_storage(), 830);

		let mut child_meter_3 = storage_meter.child_meter();
		assert_eq!(child_meter_3.available_storage(), 900);

		child_meter_3.charge(30);
		assert_eq!(child_meter_3.available_storage(), 870);
		assert_eq!(child_meter_3.finish(), Some(30));
		storage_meter.merge(&child_meter_3);

		assert_eq!(storage_meter.available_storage(), 870);
		assert_eq!(child_meter.finish(), Some(70));
		assert_eq!(storage_meter.finish(), Some(130));
		storage_meter.merge(&child_meter);
		assert_eq!(storage_meter.available_storage(), 800);
	}
}
