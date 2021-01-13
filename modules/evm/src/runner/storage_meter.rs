use evm::ExitError;

/// Storagemeter.
#[derive(Clone, Debug)]
pub struct Storagemeter {
	storage_limit: u32,
	inner: Result<Inner, ExitError>,
}

#[derive(Clone, Debug)]
struct Inner {
	used_storage: u32,
	refunded_storage: u32,
}

impl Storagemeter {
	/// Create a new storagemeter with given storage limit.
	pub fn new(storage_limit: u32) -> Self {
		Self {
			storage_limit,
			inner: Ok(Inner {
				used_storage: 0,
				refunded_storage: 0,
			}),
		}
	}

	fn inner_mut(&mut self) -> Result<&mut Inner, ExitError> {
		self.inner.as_mut().map_err(|e| e.clone())
	}

	/// Remaining storage.
	pub fn storage(&self) -> u32 {
		match self.inner.as_ref() {
			Ok(inner) => self.storage_limit - inner.used_storage + inner.refunded_storage,
			Err(_) => 0,
		}
	}

	/// Total used gas.
	pub fn total_used_storage(&self) -> u32 {
		match self.inner.as_ref() {
			Ok(inner) => inner.used_storage,
			Err(_) => self.storage_limit,
		}
	}

	/// Refunded storage.
	pub fn refunded_storage(&self) -> u32 {
		match self.inner.as_ref() {
			Ok(inner) => inner.refunded_storage,
			Err(_) => 0,
		}
	}

	/// Record an explict cost.
	pub fn record_cost(&mut self, cost: u32) -> Result<(), ExitError> {
		let all_storage_cost = self.total_used_storage() + cost;
		if self.storage_limit < all_storage_cost {
			self.inner = Err(ExitError::Other("OutOfStorageLimit".into()));
			return Err(ExitError::Other("OutOfStorageLimit".into()));
		}

		self.inner_mut()?.used_storage += cost;
		Ok(())
	}

	/// Record an explict refund.
	pub fn record_refund(&mut self, refund: u32) -> Result<(), ExitError> {
		self.inner_mut()?.refunded_storage += refund;
		Ok(())
	}

	/// Record an explict stipend.
	pub fn record_stipend(&mut self, stipend: u32) -> Result<(), ExitError> {
		let inner = self.inner_mut()?;

		if inner.used_storage > stipend {
			inner.used_storage -= stipend;
		} else {
			// stipend > used_storage
			inner.refunded_storage += stipend - inner.used_storage;
			inner.used_storage = 0;
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::assert_ok;

	#[test]
	fn test_storage_with_limit_zero() {
		let mut storagemeter = Storagemeter::new(0);
		assert_eq!(storagemeter.storage(), 0);
		assert_eq!(storagemeter.refunded_storage(), 0);
		assert_eq!(storagemeter.total_used_storage(), 0);

		// record_refund
		assert_ok!(storagemeter.record_refund(1));
		assert_eq!(storagemeter.storage(), 1);
		assert_eq!(storagemeter.refunded_storage(), 1);
		assert_eq!(storagemeter.total_used_storage(), 0);

		// record_stipend
		assert_ok!(storagemeter.record_stipend(1));
		assert_eq!(storagemeter.storage(), 2);
		assert_eq!(storagemeter.refunded_storage(), 2);
		assert_eq!(storagemeter.total_used_storage(), 0);

		// record_cost
		assert_eq!(
			storagemeter.record_cost(1),
			Err(ExitError::Other("OutOfStorageLimit".into()))
		);
	}

	#[test]
	fn test_storage_with_limit_ten() {
		let mut storagemeter = Storagemeter::new(10);
		assert_eq!(storagemeter.storage(), 10);
		assert_eq!(storagemeter.refunded_storage(), 0);
		assert_eq!(storagemeter.total_used_storage(), 0);

		// record_refund
		assert_ok!(storagemeter.record_refund(1));
		assert_eq!(storagemeter.storage(), 11);
		assert_eq!(storagemeter.refunded_storage(), 1);
		assert_eq!(storagemeter.total_used_storage(), 0);

		// record_cost
		assert_ok!(storagemeter.record_cost(1));
		assert_eq!(storagemeter.storage(), 10);
		assert_eq!(storagemeter.refunded_storage(), 1);
		assert_eq!(storagemeter.total_used_storage(), 1);

		// record_stipend
		assert_ok!(storagemeter.record_stipend(1));
		assert_eq!(storagemeter.storage(), 11);
		assert_eq!(storagemeter.refunded_storage(), 1);
		assert_eq!(storagemeter.total_used_storage(), 0);

		// record_cost
		assert_eq!(
			storagemeter.record_cost(11),
			Err(ExitError::Other("OutOfStorageLimit".into()))
		);
		assert_eq!(
			storagemeter.record_stipend(1),
			Err(ExitError::Other("OutOfStorageLimit".into()))
		);
		assert_eq!(
			storagemeter.record_refund(1),
			Err(ExitError::Other("OutOfStorageLimit".into()))
		);
	}
}
