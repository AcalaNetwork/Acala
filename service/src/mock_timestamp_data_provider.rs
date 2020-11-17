use sp_inherents::{InherentData, InherentIdentifier, ProvideInherentData};
use std::cell::RefCell;

#[cfg(feature = "with-acala-runtime")]
use acala_runtime::SLOT_DURATION;
#[cfg(feature = "with-karura-runtime")]
use karura_runtime::SLOT_DURATION;
#[cfg(feature = "with-mandala-runtime")]
use mandala_runtime::SLOT_DURATION;

/// Provide a mock duration starting at 0 in millisecond for timestamp inherent.
/// Each call will increment timestamp by slot_duration making block importer
/// think time has passed.
pub struct MockTimestampInherentDataProvider;

pub const INHERENT_IDENTIFIER: InherentIdentifier = *b"timstap0";

thread_local!(static TIMESTAMP: RefCell<u64> = RefCell::new(0));

impl ProvideInherentData for MockTimestampInherentDataProvider {
	fn inherent_identifier(&self) -> &'static InherentIdentifier {
		&INHERENT_IDENTIFIER
	}

	fn provide_inherent_data(&self, inherent_data: &mut InherentData) -> Result<(), sp_inherents::Error> {
		TIMESTAMP.with(|x| {
			*x.borrow_mut() += SLOT_DURATION;
			inherent_data.put_data(INHERENT_IDENTIFIER, &*x.borrow())
		})
	}

	fn error_to_string(&self, error: &[u8]) -> Option<String> {
		Some(String::from_utf8_lossy(error).into())
	}
}
