use evm::ExitError;
use frame_support::RuntimeDebug;

trait StorageMeterHandler {
	fn reserve_storage(&mut self, limit: u32) -> Result<(), ExitError>;
	fn unreserve_storage(&mut self, limit: u32, used: u32, refunded: u32) -> Result<(), ExitError>;

	fn charge_storage(&mut self, contract: H160, used: u32, refunded: u32) -> Result<(), ExitError>;
}

/// StorageMeter.
#[derive(Clone, RuntimeDebug)]
pub struct StorageMeter {
	contract: H160,
	limit: u32,
	used: u32,
	refunded: u32,
	handler: Box<dyn StorageMeterHandler>,
	reuslt: Result<(), ExitError>,
}

const OUT_OF_STORAGE_ERROR: ExitError = ExitError::Other("OutOfStorage".into());

impl StorageMeter {
	/// Create a new storage_meter with given storage limit.
	pub fn new(handler: Box<dyn StorageMeterHandler>, contract: H160, limit: u32) -> Result<Self, ExitError> {
		handler.reserve_storage(limit)?;
		Ok(Self {
			contract,
			limit,
			used: 0,
			refunded: 0,
			handler,
			reuslt: Ok(()),
		})
	}

	pub fn child_meter(&mut self, contract: H160) -> Result<Self, ExitError> {
		self.handle(|| Self::new(self, contract, self.available_storage()))
	}

	pub fn available_storage() -> u32 {
		if self.result.is_ok() {
			self.limit.saturating_add(self.refunded).saturating_sub(self.used)
		} else {
			0
		}
	}

	pub fn charge(&mut self, storage: u32) -> Result<(), ExitError> {
		self.handle(|| {
			let used = self.used.saturating_add(limit);
			if self.limit < used.saturating_sub(self.refunded) {
				self.result = Err(OUT_OF_STORAGE_ERROR);
				return self.result;
			}
			self.used = used;
			Ok(())
		})
	}

	pub fn uncharge(&mut self, storage: u32) -> Result<(), ExitError> {
		self.handle(|| {
			self.used = self.used.saturating_sub(storage);
			Ok(())
		})
	}

	pub fn refund(&mut self, storage: u32) -> Result<(), ExitError> {
		self.handle(|| {
			self.refunded = self.refunded.saturating_add(storage);
			Ok(())
		})
	}

	pub fn finish(self) -> Result<(), ExitError> {
		self.handle(|| {
			self.handler.charge_storage(self.contract, self.used, self.refunded)?;
			self.handler.unreserve_storage(self.limit, self.used, self.refunded)?;
		})
	}

	fn handle<F: FnOnce() -> Result<(), ExitError>>(f: F) -> Result<(), ExitError> {
		self.result?;
		self.result = f();
		self.result
	}
}

impl StorageMeterHandler for StorageMeter {
	fn reserve_storage(&mut self, limit: u32) -> Result<(), ExitError> {
		self.charge(limit)
	}

	fn unreserve_storage(&mut self, limit: u32, used: u32, refunded: u32) -> Result<(), ExitError> {
		self.uncharge(limit.saturating_add(refunded).saturating_sub(used))?;
	}

	fn charge_storage(&mut self, contract: H160, storage: u32, refunded: u32) -> Result<(), ExitError> {
		self.handler.charge_storage(contract, storage, refunded)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::assert_ok;
	struct DummyHandler {
		storage: std::collections::BTreeMap<H160, u32>,
		reserves: std::collections::BTreeMap<H160, u32>,
	}

	const ALICE: H160 = H160::from_low_u64_be(123);

	impl StorageMeterHandler for DummyHandler {
		fn reserve_storage(&mut self, limit: u32) -> Result<(), ExitError> {
			self.storage
				.get_mut(&ALICE)
				.checked_sub(limit)
				.ok_or(|_| ExitError::Other("error".into()))?;
			self.reserves.get_mut(&ALICE) += limit;
			Ok(())
		}

		fn unreserve_storage(&mut self, limit: u32, used: u32, refunded: u32) -> Result<(), ExitError> {
			if used > refunded {
				let diff = used - refunded;
				self.reserves
					.get_mut(&ALICE)
					.checked_sub(limit)
					.ok_or(|_| ExitError::Other("error".into()))?;
				self.storage.get_mut(&ALICE) += limit - diff;
			} else {
				self.storage.get_mut(&ALICE) += limit + refunded - used;
			}
			Ok(())
		}

		fn charge_storage(&mut self, contract: H160, used: u32, refunded: u32) -> Result<(), ExitError> {
			if used > refunded {
				let diff = used - refunded;
				self.reserves
					.get_mut(&ALICE)
					.checked_sub(diff)
					.ok_or(|_| ExitError::Other("error".into()))?;
			} else {
				self.reserves.get_mut(&ALICE) += refunded - used;
			}
			Ok(())
		}
	}

	#[test]
	fn test_storage_with_limit_zero() {
		let mut storage_meter = StorageMeter::new(0);
		assert_eq!(storage_meter.available_storage(), 0);

		// record_refund
		assert_ok!(storage_meter.refund(1));
		assert_eq!(storage_meter.available_storage(), 1);
		assert_eq!(storage_meter.refunded_storage(), 1);
		assert_eq!(storage_meter.total_used_storage(), 0);

		// record_stipend
		assert_ok!(storage_meter.record_stipend(1));
		assert_eq!(storage_meter.storage(), 2);
		assert_eq!(storage_meter.refunded_storage(), 2);
		assert_eq!(storage_meter.total_used_storage(), 0);

		// record_cost
		assert_eq!(
			storage_meter.record_cost(1),
			Err(ExitError::Other("OutOfStorageLimit".into()))
		);
	}

	#[test]
	fn test_storage_with_limit_ten() {
		let mut storage_meter = StorageMeter::new(10);
		assert_eq!(storage_meter.storage(), 10);
		assert_eq!(storage_meter.refunded_storage(), 0);
		assert_eq!(storage_meter.total_used_storage(), 0);

		// record_refund
		assert_ok!(storage_meter.record_refund(1));
		assert_eq!(storage_meter.storage(), 11);
		assert_eq!(storage_meter.refunded_storage(), 1);
		assert_eq!(storage_meter.total_used_storage(), 0);

		// record_cost
		assert_ok!(storage_meter.record_cost(1));
		assert_eq!(storage_meter.storage(), 10);
		assert_eq!(storage_meter.refunded_storage(), 1);
		assert_eq!(storage_meter.total_used_storage(), 1);

		// record_stipend
		assert_ok!(storage_meter.record_stipend(1));
		assert_eq!(storage_meter.storage(), 11);
		assert_eq!(storage_meter.refunded_storage(), 1);
		assert_eq!(storage_meter.total_used_storage(), 0);

		// record_cost
		assert_eq!(
			storage_meter.record_cost(11),
			Err(ExitError::Other("OutOfStorageLimit".into()))
		);
		assert_eq!(
			storage_meter.record_stipend(1),
			Err(ExitError::Other("OutOfStorageLimit".into()))
		);
		assert_eq!(
			storage_meter.record_refund(1),
			Err(ExitError::Other("OutOfStorageLimit".into()))
		);
	}
}
