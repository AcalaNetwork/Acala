use super::*;
use codec::{Codec, Decode, Encode};
use rstd::prelude::Vec;
use sp_runtime::{
	offchain::{storage::StorageValueRef, Duration, Timestamp},
	RuntimeDebug,
};

// constant for offchain lock
const LOCK_EXPIRE_DURATION: u64 = 300_000; // 5 min
const LOCK_UPDATE_DURATION: u64 = 240_000; // 4 min

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct LockItem<T: Codec> {
	pub expire_timestamp: Timestamp,
	pub extra_data: T,
}

pub struct OffchainLock {
	key: Vec<u8>,
}

impl OffchainLock {
	pub fn new(key: Vec<u8>) -> Self {
		OffchainLock { key }
	}

	pub fn acquire_offchain_lock<T, F>(&self, f: F) -> Result<LockItem<T>, OffchainErr>
	where
		T: codec::Codec,
		F: FnOnce(Option<T>) -> T,
	{
		let storage = StorageValueRef::persistent(&self.key);
		let acquire_lock = storage.mutate(|lock: Option<Option<LockItem<T>>>| match lock {
			None => Ok(LockItem {
				expire_timestamp: runtime_io::offchain::timestamp().add(Duration::from_millis(LOCK_EXPIRE_DURATION)),
				extra_data: f(None),
			}),
			Some(Some(item)) => Ok(LockItem {
				expire_timestamp: runtime_io::offchain::timestamp().add(Duration::from_millis(LOCK_EXPIRE_DURATION)),
				extra_data: f(Some(item.extra_data)),
			}),
			_ => Err(OffchainErr::OffchainLock),
		})?;

		acquire_lock.map_err(|_| OffchainErr::OffchainStore)
	}

	pub fn release_offchain_lock<T, F>(&self, f: F)
	where
		T: codec::Codec + Copy,
		F: FnOnce(T) -> bool,
	{
		let storage = StorageValueRef::persistent(&self.key);

		if let Some(Some(lock)) = storage.get::<LockItem<T>>() {
			if f(lock.extra_data) {
				storage.set(&LockItem {
					expire_timestamp: runtime_io::offchain::timestamp(),
					extra_data: lock.extra_data,
				});
			}
		}
	}

	pub fn extend_offchain_lock_if_needed<T: codec::Codec>(&self) {
		let storage = StorageValueRef::persistent(&self.key);

		if let Some(Some(lock)) = storage.get::<LockItem<T>>() {
			if lock.expire_timestamp
				<= runtime_io::offchain::timestamp().add(Duration::from_millis(LOCK_UPDATE_DURATION))
			{
				storage.set(&LockItem {
					expire_timestamp: runtime_io::offchain::timestamp()
						.add(Duration::from_millis(LOCK_EXPIRE_DURATION)),
					extra_data: lock.extra_data,
				});
			}
		}
	}
}
