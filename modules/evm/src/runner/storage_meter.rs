use frame_support::debug;
use sp_core::H160;
use sp_runtime::{DispatchError, DispatchResult};

pub trait StorageMeterHandler {
	fn reserve_storage(&mut self, limit: u32) -> DispatchResult;
	fn unreserve_storage(&mut self, limit: u32, used: u32, refunded: u32) -> DispatchResult;

	fn charge_storage(&mut self, contract: &H160, used: u32, refunded: u32) -> DispatchResult;

	fn out_of_storage_error(&self) -> DispatchError;
}

pub struct StorageMeter<'handler> {
	contract: H160,
	limit: u32,
	total_used: u32,
	self_used: u32,
	total_refunded: u32,
	self_refunded: u32,
	handler: &'handler mut dyn StorageMeterHandler,
	result: DispatchResult,
}

impl<'handler> StorageMeter<'handler> {
	/// Create a new storage_meter with given storage limit.
	pub fn new(
		handler: &'handler mut dyn StorageMeterHandler,
		contract: H160,
		limit: u32,
	) -> Result<Self, DispatchError> {
		debug::trace!(
			target: "evm",
			"StorageMeter: create: contract {:?} limit {:?}",
			contract, limit
		);
		handler.reserve_storage(limit)?;
		Ok(Self {
			contract,
			limit,
			total_used: 0,
			self_used: 0,
			total_refunded: 0,
			self_refunded: 0,
			handler,
			result: Ok(()),
		})
	}

	pub fn child_meter(&mut self, contract: H160) -> Result<StorageMeter<'_>, DispatchError> {
		self.handle(|this| {
			let storage = this.available_storage();
			// can't make this.result = Err if `new` fails
			// because some rust lifetime thing never happy
			StorageMeter::new(this, contract, storage)
		})
	}

	pub fn available_storage(&self) -> u32 {
		if self.result.is_ok() {
			self.limit
				.saturating_add(self.total_refunded)
				.saturating_sub(self.total_used)
		} else {
			0
		}
	}

	pub fn used_storage(&self) -> i32 {
		if self.total_used > self.total_refunded {
			(self.total_used - self.total_refunded) as i32
		} else {
			-((self.total_refunded - self.total_used) as i32)
		}
	}

	pub fn charge(&mut self, storage: u32) -> DispatchResult {
		debug::trace!(
			target: "evm",
			"StorageMeter: charge: storage {:?}",
			storage
		);
		self.handle(|this| {
			let used = this.total_used.saturating_add(storage);
			if this.limit < used.saturating_sub(this.total_refunded) {
				this.result = Err(this.out_of_storage_error());
				return this.result;
			}
			this.total_used = used;
			this.self_used = this.self_used.saturating_add(storage);
			Ok(())
		})
	}

	pub fn uncharge(&mut self, storage: u32) -> DispatchResult {
		debug::trace!(
			target: "evm",
			"StorageMeter: uncharge: storage {:?}",
			storage
		);
		self.handle(|this| {
			this.total_used = this.total_used.saturating_sub(storage);
			this.self_used = this.self_used.saturating_sub(storage);
			Ok(())
		})
	}

	pub fn refund(&mut self, storage: u32) -> DispatchResult {
		debug::trace!(
			target: "evm",
			"StorageMeter: refund: storage {:?}",
			storage
		);
		self.handle(|this| {
			this.total_refunded = this.total_refunded.saturating_add(storage);
			this.self_refunded = this.self_refunded.saturating_add(storage);
			Ok(())
		})
	}

	pub fn finish(mut self) -> DispatchResult {
		debug::trace!(
			target: "evm",
			"StorageMeter: finish: used {:?} refunded {:?}",
			self.total_used, self.total_refunded
		);
		self.handle(|this| {
			if let Err(x) = (|| {
				this.handler
					.charge_storage(&this.contract, this.self_used, this.self_refunded)?;
				let new_limit = this
					.limit
					.saturating_add(this.self_used)
					.saturating_add(this.total_refunded)
					.saturating_sub(this.total_used)
					.saturating_sub(this.self_refunded);
				this.handler
					.unreserve_storage(new_limit, this.self_used, this.self_refunded)
			})() {
				this.result = Err(x);
				Err(x)
			} else {
				Ok(())
			}
		})
	}

	fn handle<'a, R, F: FnOnce(&'a mut Self) -> Result<R, DispatchError>>(
		&'a mut self,
		f: F,
	) -> Result<R, DispatchError> {
		self.result?;
		f(self)
	}
}

impl<'handler> StorageMeterHandler for StorageMeter<'handler> {
	fn reserve_storage(&mut self, _limit: u32) -> DispatchResult {
		Ok(())
	}

	fn unreserve_storage(&mut self, _limit: u32, _used: u32, _refunded: u32) -> DispatchResult {
		Ok(())
	}

	fn charge_storage(&mut self, contract: &H160, used: u32, refunded: u32) -> DispatchResult {
		self.handle(|this| {
			this.total_refunded = this.total_refunded.saturating_add(refunded);
			this.total_used = this.total_used.saturating_add(used);
			this.handler.charge_storage(contract, used, refunded)
		})
	}

	fn out_of_storage_error(&self) -> DispatchError {
		"OutOfStorage".into()
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
	struct DummyHandler {
		pub storages: std::collections::BTreeMap<H160, u32>,
		pub reserves: std::collections::BTreeMap<H160, u32>,
	}

	impl DummyHandler {
		fn new() -> Self {
			let mut val = Self {
				storages: Default::default(),
				reserves: Default::default(),
			};
			val.storages.insert(ALICE, 0);
			val.reserves.insert(ALICE, 0);
			val.storages.insert(CONTRACT, 0);
			val.reserves.insert(CONTRACT, 0);
			val.storages.insert(CONTRACT_2, 0);
			val.reserves.insert(CONTRACT_2, 0);
			val.storages.insert(CONTRACT_3, 0);
			val.reserves.insert(CONTRACT_3, 0);
			val
		}
	}

	impl StorageMeterHandler for DummyHandler {
		fn reserve_storage(&mut self, limit: u32) -> DispatchResult {
			if limit == 0 {
				return Ok(());
			}
			let val = self.storages.get_mut(&ALICE).ok_or("error")?;
			*val = val.checked_sub(limit).ok_or("error")?;
			if let Some(v) = self.reserves.get_mut(&ALICE) {
				*v += limit;
			} else {
				self.reserves.insert(ALICE, limit);
			}
			Ok(())
		}

		fn unreserve_storage(&mut self, limit: u32, used: u32, refunded: u32) -> DispatchResult {
			let diff = limit + refunded - used;
			if diff == 0 {
				return Ok(());
			}
			let val = self.reserves.get_mut(&ALICE).ok_or("error")?;
			*val = val.checked_sub(diff).ok_or("error")?;
			if let Some(v) = self.storages.get_mut(&ALICE) {
				*v += diff;
			} else {
				self.storages.insert(ALICE, diff);
			}
			Ok(())
		}

		fn charge_storage(&mut self, contract: &H160, used: u32, refunded: u32) -> DispatchResult {
			if used == refunded {
				return Ok(());
			}
			let alice = self.reserves.get_mut(&ALICE).ok_or("error")?;
			if used > refunded {
				*alice = alice.checked_sub(used - refunded).ok_or("error")?;
			} else {
				*alice = alice.checked_add(refunded - used).ok_or("error")?;
			}

			let contract_val = self.reserves.get_mut(&contract).ok_or("error")?;
			if used > refunded {
				*contract_val = contract_val.checked_add(used - refunded).ok_or("error")?;
			} else {
				*contract_val = contract_val.checked_sub(refunded - used).ok_or("error")?;
			}
			Ok(())
		}

		fn out_of_storage_error(&self) -> DispatchError {
			"OutOfStorage".into()
		}
	}

	#[test]
	fn test_storage_with_limit_zero() {
		let mut handler = DummyHandler::new();

		let mut storage_meter = StorageMeter::new(&mut handler, CONTRACT, 0).unwrap();
		assert_eq!(storage_meter.available_storage(), 0);

		// refund
		assert_ok!(storage_meter.refund(1));
		assert_eq!(storage_meter.available_storage(), 1);

		// charge
		assert_ok!(storage_meter.charge(1));
		assert_eq!(storage_meter.available_storage(), 0);

		assert_ok!(storage_meter.finish());

		assert_eq!(handler.storages.get(&ALICE).cloned().unwrap_or_default(), 0);
		assert_eq!(handler.reserves.get(&ALICE).cloned().unwrap_or_default(), 0);
		assert_eq!(handler.storages.get(&CONTRACT).cloned().unwrap_or_default(), 0);
		assert_eq!(handler.reserves.get(&CONTRACT).cloned().unwrap_or_default(), 0);
	}

	#[test]
	fn test_charge_storage_fee() {
		let mut handler = DummyHandler::new();
		handler.storages.insert(ALICE, 1000);

		let mut storage_meter = StorageMeter::new(&mut handler, CONTRACT, 1000).unwrap();
		assert_eq!(storage_meter.available_storage(), 1000);

		assert_ok!(storage_meter.refund(1));
		assert_eq!(storage_meter.available_storage(), 1001);

		assert_ok!(storage_meter.charge(101));
		assert_eq!(storage_meter.available_storage(), 900);

		assert_ok!(storage_meter.charge(50));
		assert_eq!(storage_meter.available_storage(), 850);

		assert_ok!(storage_meter.refund(20));
		assert_eq!(storage_meter.available_storage(), 870);

		assert_ok!(storage_meter.finish());

		assert_eq!(handler.storages.get(&ALICE).cloned().unwrap_or_default(), 870);
		assert_eq!(handler.reserves.get(&ALICE).cloned().unwrap_or_default(), 0);
		assert_eq!(handler.storages.get(&CONTRACT).cloned().unwrap_or_default(), 0);
		assert_eq!(handler.reserves.get(&CONTRACT).cloned().unwrap_or_default(), 130);
	}

	#[test]
	fn test_refund_storage_fee() {
		let mut handler = DummyHandler::new();
		handler.storages.insert(ALICE, 1000);
		handler.reserves.insert(CONTRACT, 1000);

		let mut storage_meter = StorageMeter::new(&mut handler, CONTRACT, 1000).unwrap();
		assert_eq!(storage_meter.available_storage(), 1000);

		assert_ok!(storage_meter.refund(100));
		assert_eq!(storage_meter.available_storage(), 1100);

		assert_ok!(storage_meter.charge(50));
		assert_eq!(storage_meter.available_storage(), 1050);

		assert_ok!(storage_meter.finish());

		assert_eq!(handler.storages.get(&ALICE).cloned().unwrap_or_default(), 1050);
		assert_eq!(handler.reserves.get(&ALICE).cloned().unwrap_or_default(), 0);
		assert_eq!(handler.storages.get(&CONTRACT).cloned().unwrap_or_default(), 0);
		assert_eq!(handler.reserves.get(&CONTRACT).cloned().unwrap_or_default(), 950);
	}

	#[test]
	fn test_out_of_storage() {
		let mut handler = DummyHandler::new();
		handler.storages.insert(ALICE, 1000);

		assert!(StorageMeter::new(&mut handler, CONTRACT, 1001).is_err());
		let mut storage_meter = StorageMeter::new(&mut handler, CONTRACT, 1000).unwrap();
		assert_eq!(storage_meter.available_storage(), 1000);

		assert_ok!(storage_meter.charge(500));
		assert_eq!(storage_meter.available_storage(), 500);

		assert_ok!(storage_meter.charge(500));
		assert_eq!(storage_meter.available_storage(), 0);

		assert_err!(storage_meter.charge(1), DispatchError::Other("OutOfStorage"));
		assert_err!(storage_meter.refund(1), DispatchError::Other("OutOfStorage"));
		assert_err!(
			storage_meter.child_meter(CONTRACT_2).map(|_| ()),
			DispatchError::Other("OutOfStorage")
		);
		assert_err!(storage_meter.finish(), DispatchError::Other("OutOfStorage"));
	}

	#[test]
	fn test_child_meter() {
		let mut handler = DummyHandler::new();
		handler.storages.insert(ALICE, 1000);

		let mut storage_meter = StorageMeter::new(&mut handler, CONTRACT, 1000).unwrap();

		assert_ok!(storage_meter.charge(100));

		let mut child_meter = storage_meter.child_meter(CONTRACT_2).unwrap();
		assert_eq!(child_meter.available_storage(), 900);

		assert_ok!(child_meter.charge(100));
		assert_eq!(child_meter.available_storage(), 800);

		assert_ok!(child_meter.refund(50));
		assert_eq!(child_meter.available_storage(), 850);

		let mut child_meter_2 = child_meter.child_meter(CONTRACT_3).unwrap();

		assert_eq!(child_meter_2.available_storage(), 850);

		assert_ok!(child_meter_2.charge(20));
		assert_eq!(child_meter_2.available_storage(), 830);

		assert_ok!(child_meter_2.finish());

		assert_ok!(child_meter.finish());

		let mut child_meter_3 = storage_meter.child_meter(CONTRACT_2).unwrap();

		assert_eq!(child_meter_3.available_storage(), 830);

		assert_ok!(child_meter_3.charge(30));

		assert_eq!(child_meter_3.available_storage(), 800);

		assert_ok!(child_meter_3.finish());

		assert_eq!(storage_meter.available_storage(), 800);

		assert_ok!(storage_meter.finish());

		assert_eq!(handler.storages.get(&ALICE).cloned().unwrap_or_default(), 800);
		assert_eq!(handler.reserves.get(&ALICE).cloned().unwrap_or_default(), 0);
		assert_eq!(handler.reserves.get(&CONTRACT).cloned().unwrap_or_default(), 100);
		assert_eq!(handler.reserves.get(&CONTRACT_2).cloned().unwrap_or_default(), 80);
		assert_eq!(handler.reserves.get(&CONTRACT_3).cloned().unwrap_or_default(), 20);
	}
}
