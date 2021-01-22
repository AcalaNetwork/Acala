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
	used: u32,
	refunded: u32,
	handler: Box<&'handler mut dyn StorageMeterHandler>,
	result: DispatchResult,
}

impl<'handler> StorageMeter<'handler> {
	/// Create a new storage_meter with given storage limit.
	pub fn new(
		handler: Box<&'handler mut dyn StorageMeterHandler>,
		contract: H160,
		limit: u32,
	) -> Result<Self, DispatchError> {
		handler.reserve_storage(limit)?;
		Ok(Self {
			contract,
			limit,
			used: 0,
			refunded: 0,
			handler,
			result: Ok(()),
		})
	}

	pub fn child_meter<'a>(&'a mut self, contract: H160) -> Result<StorageMeter<'a>, DispatchError> {
		self.handle(|this| {
			let storage = this.available_storage();
			// can't make this.result = Err if `new` fails
			// because some rust lifetime thing never happy
			StorageMeter::<'a>::new(Box::new(this), contract, storage)
		})
	}

	pub fn available_storage(&self) -> u32 {
		if self.result.is_ok() {
			self.limit.saturating_add(self.refunded).saturating_sub(self.used)
		} else {
			0
		}
	}

	pub fn used_storage(&self) -> i32 {
		if self.used > self.refunded {
			(self.used - self.refunded) as i32
		} else {
			-((self.refunded - self.used) as i32)
		}
	}

	pub fn charge(&mut self, storage: u32) -> DispatchResult {
		self.handle(|this| {
			let used = this.used.saturating_add(storage);
			if this.limit < used.saturating_sub(this.refunded) {
				this.result = Err(this.out_of_storage_error());
				return this.result;
			}
			this.used = used;
			Ok(())
		})
	}

	pub fn uncharge(&mut self, storage: u32) -> DispatchResult {
		self.handle(|this| {
			this.used = this.used.saturating_sub(storage);
			Ok(())
		})
	}

	pub fn refund(&mut self, storage: u32) -> DispatchResult {
		self.handle(|this| {
			this.refunded = this.refunded.saturating_add(storage);
			Ok(())
		})
	}

	pub fn finish(mut self) -> DispatchResult {
		self.handle(|this| {
			if let Err(x) = (|| {
				this.handler.charge_storage(&this.contract, this.used, this.refunded)?;
				this.handler.unreserve_storage(this.limit, this.used, this.refunded)
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
	fn reserve_storage(&mut self, limit: u32) -> DispatchResult {
		self.charge(limit)
	}

	fn unreserve_storage(&mut self, limit: u32, used: u32, refunded: u32) -> DispatchResult {
		self.uncharge(limit.saturating_add(refunded).saturating_sub(used))
	}

	fn charge_storage(&mut self, contract: &H160, used: u32, refunded: u32) -> DispatchResult {
		self.handler.charge_storage(contract, used, refunded)
	}

	fn out_of_storage_error(&self) -> DispatchError {
		"OutOfStorage".into()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::assert_ok;

	const ALICE: H160 = H160::repeat_byte(11);
	const CONTRACT: H160 = H160::repeat_byte(22);

	#[derive(Default)]
	struct DummyHandler {
		pub storages: std::collections::BTreeMap<H160, u32>,
		pub reserves: std::collections::BTreeMap<H160, u32>,
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
		let mut handler = DummyHandler::default();

		let mut storage_meter = StorageMeter::new(Box::new(&mut handler), CONTRACT, 0).unwrap();
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
}
