#![cfg_attr(not(feature = "std"), no_std)]

pub mod offchain_lock;

pub use offchain_lock::{LockItem, OffchainLock};

/// Error which may occur while executing the off-chain code.
#[cfg_attr(test, derive(PartialEq))]
pub enum OffchainErr {
	OffchainStore,
	SubmitTransaction,
	NotValidator,
	OffchainLock,
}

impl rstd::fmt::Debug for OffchainErr {
	fn fmt(&self, fmt: &mut rstd::fmt::Formatter) -> rstd::fmt::Result {
		match *self {
			OffchainErr::OffchainStore => write!(fmt, "Failed to manipulate offchain store"),
			OffchainErr::SubmitTransaction => write!(fmt, "Failed to submit transaction"),
			OffchainErr::NotValidator => write!(fmt, "Is not validator"),
			OffchainErr::OffchainLock => write!(fmt, "Failed to manipulate offchain lock"),
		}
	}
}
