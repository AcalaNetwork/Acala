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

pub trait StorageMeterHandler {
	fn reserve_storage(&mut self, limit: u32) -> DispatchResult;
	fn unreserve_storage(&mut self, limit: u32, used: u32, refunded: u32) -> DispatchResult;

	fn charge_storage(&mut self, contract: &H160, used: u32, refunded: u32) -> DispatchResult;

	fn out_of_storage_error(&self) -> DispatchError;
}

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
}

//#[cfg(test)]
//mod tests {
//	use super::*;
//	use frame_support::{assert_err, assert_ok};
//
//	const ALICE: H160 = H160::repeat_byte(11);
//	const CONTRACT: H160 = H160::repeat_byte(22);
//	const CONTRACT_2: H160 = H160::repeat_byte(33);
//	const CONTRACT_3: H160 = H160::repeat_byte(44);
//	struct DummyHandler {
//		pub storages: std::collections::BTreeMap<H160, u32>,
//		pub reserves: std::collections::BTreeMap<H160, u32>,
//	}
//
//	impl DummyHandler {
//		fn new() -> Self {
//			let mut val = Self {
//				storages: Default::default(),
//				reserves: Default::default(),
//			};
//			val.storages.insert(ALICE, 0);
//			val.reserves.insert(ALICE, 0);
//			val.storages.insert(CONTRACT, 0);
//			val.reserves.insert(CONTRACT, 0);
//			val.storages.insert(CONTRACT_2, 0);
//			val.reserves.insert(CONTRACT_2, 0);
//			val.storages.insert(CONTRACT_3, 0);
//			val.reserves.insert(CONTRACT_3, 0);
//			val
//		}
//	}
//
//	#[test]
//	fn test_storage_with_limit_zero() {
//		let mut handler = DummyHandler::new();
//
//		let mut storage_meter = StorageMeter::new(&mut handler, CONTRACT, 0).unwrap();
//		assert_eq!(storage_meter.available_storage(), 0);
//
//		// refund
//		assert_ok!(storage_meter.refund(1));
//		assert_eq!(storage_meter.available_storage(), 1);
//
//		// charge
//		assert_ok!(storage_meter.charge(1));
//		assert_eq!(storage_meter.available_storage(), 0);
//
//		assert_ok!(storage_meter.finish());
//
//		assert_eq!(handler.storages.get(&ALICE).cloned().unwrap_or_default(), 0);
//		assert_eq!(handler.reserves.get(&ALICE).cloned().unwrap_or_default(), 0);
//		assert_eq!(handler.storages.get(&CONTRACT).cloned().unwrap_or_default(), 0);
//		assert_eq!(handler.reserves.get(&CONTRACT).cloned().unwrap_or_default(), 0);
//	}
//
//	#[test]
//	fn test_charge_storage_fee() {
//		let mut handler = DummyHandler::new();
//		handler.storages.insert(ALICE, 1000);
//
//		let mut storage_meter = StorageMeter::new(&mut handler, CONTRACT, 1000).unwrap();
//		assert_eq!(storage_meter.available_storage(), 1000);
//
//		assert_ok!(storage_meter.refund(1));
//		assert_eq!(storage_meter.available_storage(), 1001);
//
//		assert_ok!(storage_meter.charge(101));
//		assert_eq!(storage_meter.available_storage(), 900);
//
//		assert_ok!(storage_meter.charge(50));
//		assert_eq!(storage_meter.available_storage(), 850);
//
//		assert_ok!(storage_meter.refund(20));
//		assert_eq!(storage_meter.available_storage(), 870);
//
//		assert_ok!(storage_meter.finish());
//
//		assert_eq!(handler.storages.get(&ALICE).cloned().unwrap_or_default(), 870);
//		assert_eq!(handler.reserves.get(&ALICE).cloned().unwrap_or_default(), 0);
//		assert_eq!(handler.storages.get(&CONTRACT).cloned().unwrap_or_default(), 0);
//		assert_eq!(handler.reserves.get(&CONTRACT).cloned().unwrap_or_default(), 130);
//	}
//
//	#[test]
//	fn test_refund_storage_fee() {
//		let mut handler = DummyHandler::new();
//		handler.storages.insert(ALICE, 1000);
//		handler.reserves.insert(CONTRACT, 1000);
//
//		let mut storage_meter = StorageMeter::new(&mut handler, CONTRACT, 1000).unwrap();
//		assert_eq!(storage_meter.available_storage(), 1000);
//
//		assert_ok!(storage_meter.refund(100));
//		assert_eq!(storage_meter.available_storage(), 1100);
//
//		assert_ok!(storage_meter.charge(50));
//		assert_eq!(storage_meter.available_storage(), 1050);
//
//		assert_ok!(storage_meter.finish());
//
//		assert_eq!(handler.storages.get(&ALICE).cloned().unwrap_or_default(), 1050);
//		assert_eq!(handler.reserves.get(&ALICE).cloned().unwrap_or_default(), 0);
//		assert_eq!(handler.storages.get(&CONTRACT).cloned().unwrap_or_default(), 0);
//		assert_eq!(handler.reserves.get(&CONTRACT).cloned().unwrap_or_default(), 950);
//	}
//
//	#[test]
//	fn test_out_of_storage() {
//		let mut handler = DummyHandler::new();
//		handler.storages.insert(ALICE, 1000);
//
//		assert!(StorageMeter::new(&mut handler, CONTRACT, 1001).is_err());
//		let mut storage_meter = StorageMeter::new(&mut handler, CONTRACT, 1000).unwrap();
//		assert_eq!(storage_meter.available_storage(), 1000);
//
//		assert_ok!(storage_meter.charge(500));
//		assert_eq!(storage_meter.available_storage(), 500);
//
//		assert_ok!(storage_meter.charge(500));
//		assert_eq!(storage_meter.available_storage(), 0);
//
//		assert_ok!(storage_meter.charge(2));
//		assert_ok!(storage_meter.refund(1));
//		assert_ok!(storage_meter.child_meter(CONTRACT_2).map(|_| ()));
//		assert_err!(storage_meter.finish(), DispatchError::Other("OutOfStorage"));
//	}
//
//	#[test]
//	fn test_high_use_and_refund() {
//		let mut handler = DummyHandler::new();
//		handler.storages.insert(ALICE, 1000);
//
//		let mut storage_meter = StorageMeter::new(&mut handler, CONTRACT, 1000).unwrap();
//		assert_eq!(storage_meter.available_storage(), 1000);
//
//		assert_ok!(storage_meter.charge(1000));
//		assert_eq!(storage_meter.available_storage(), 0);
//
//		assert_ok!(storage_meter.charge(100));
//		assert_eq!(storage_meter.available_storage(), 0);
//		assert_ok!(storage_meter.refund(200));
//		assert_eq!(storage_meter.available_storage(), 100);
//		assert_ok!(storage_meter.child_meter(CONTRACT_2).map(|_| ()));
//		assert_ok!(storage_meter.finish());
//	}
//
//	#[test]
//	fn test_child_meter() {
//		let mut handler = DummyHandler::new();
//		handler.storages.insert(ALICE, 1000);
//
//		let mut storage_meter = StorageMeter::new(&mut handler, CONTRACT, 1000).unwrap();
//
//		assert_ok!(storage_meter.charge(100));
//
//		let mut child_meter = storage_meter.child_meter(CONTRACT_2).unwrap();
//		assert_eq!(child_meter.available_storage(), 900);
//
//		assert_ok!(child_meter.charge(100));
//		assert_eq!(child_meter.available_storage(), 800);
//
//		assert_ok!(child_meter.refund(50));
//		assert_eq!(child_meter.available_storage(), 850);
//
//		let mut child_meter_2 = child_meter.child_meter(CONTRACT_3).unwrap();
//
//		assert_eq!(child_meter_2.available_storage(), 850);
//
//		assert_ok!(child_meter_2.charge(20));
//		assert_eq!(child_meter_2.available_storage(), 830);
//
//		assert_ok!(child_meter_2.finish());
//
//		assert_ok!(child_meter.finish());
//
//		let mut child_meter_3 = storage_meter.child_meter(CONTRACT_2).unwrap();
//
//		assert_eq!(child_meter_3.available_storage(), 830);
//
//		assert_ok!(child_meter_3.charge(30));
//
//		assert_eq!(child_meter_3.available_storage(), 800);
//
//		assert_ok!(child_meter_3.finish());
//
//		assert_eq!(storage_meter.available_storage(), 800);
//
//		assert_ok!(storage_meter.finish());
//
//		assert_eq!(handler.storages.get(&ALICE).cloned().unwrap_or_default(), 800);
//		assert_eq!(handler.reserves.get(&ALICE).cloned().unwrap_or_default(), 0);
//		assert_eq!(handler.reserves.get(&CONTRACT).cloned().unwrap_or_default(), 100);
//		assert_eq!(handler.reserves.get(&CONTRACT_2).cloned().unwrap_or_default(), 80);
//		assert_eq!(handler.reserves.get(&CONTRACT_3).cloned().unwrap_or_default(), 20);
//	}
//}
