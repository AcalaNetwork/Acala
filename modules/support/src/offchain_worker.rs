/// Error which may occur while executing the off-chain code.
#[cfg_attr(test, derive(PartialEq))]
pub enum OffchainErr {
	FailedToAcquireLock,
	SubmitTransaction,
	NotValidator,
	LockStillInLocked,
}

impl rstd::fmt::Debug for OffchainErr {
	fn fmt(&self, fmt: &mut rstd::fmt::Formatter) -> rstd::fmt::Result {
		match *self {
			OffchainErr::FailedToAcquireLock => write!(fmt, "Failed to acquire lock"),
			OffchainErr::SubmitTransaction => write!(fmt, "Failed to submit transaction"),
			OffchainErr::NotValidator => write!(fmt, "Not validator"),
			OffchainErr::LockStillInLocked => write!(fmt, "Liquidator lock is still in locked"),
		}
	}
}
