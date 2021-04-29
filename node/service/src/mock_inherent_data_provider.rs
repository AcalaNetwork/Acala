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

use cumulus_primitives_core::PersistedValidationData;
use cumulus_primitives_parachain_inherent::{ParachainInherentData, INHERENT_IDENTIFIER};
use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
use sp_inherents::{InherentData, InherentIdentifier, ProvideInherentData};
use sp_timestamp::InherentError;
use std::cell::RefCell;

/// Provide a mock duration starting at 0 in millisecond for timestamp inherent.
/// Each call will increment timestamp by slot_duration making block importer
/// think time has passed.
pub struct MockTimestampInherentDataProvider;

pub const TIMESTAMP_INHERENT_IDENTIFIER: InherentIdentifier = *b"timstap0";

thread_local!(static TIMESTAMP: RefCell<u64> = RefCell::new(0));

impl ProvideInherentData for MockTimestampInherentDataProvider {
	fn inherent_identifier(&self) -> &'static InherentIdentifier {
		&TIMESTAMP_INHERENT_IDENTIFIER
	}

	fn provide_inherent_data(&self, inherent_data: &mut InherentData) -> Result<(), sp_inherents::Error> {
		TIMESTAMP.with(|x| {
			*x.borrow_mut() += 4_000;
			inherent_data.put_data(TIMESTAMP_INHERENT_IDENTIFIER, &*x.borrow())
		})
	}

	fn error_to_string(&self, error: &[u8]) -> Option<String> {
		Some(String::from_utf8_lossy(error).into())
	}
}

pub struct MockParachainInherentDataProvider;
impl ProvideInherentData for MockParachainInherentDataProvider {
	fn inherent_identifier(&self) -> &'static InherentIdentifier {
		&INHERENT_IDENTIFIER
	}

	fn provide_inherent_data(&self, inherent_data: &mut InherentData) -> Result<(), sp_inherents::Error> {
		// Use the "sproof" (spoof proof) builder to build valid mock state root and
		// proof.
		let (relay_storage_root, proof) = RelayStateSproofBuilder::default().into_state_root_and_proof();

		let data = ParachainInherentData {
			validation_data: PersistedValidationData {
				parent_head: Default::default(),
				relay_parent_storage_root: relay_storage_root,
				relay_parent_number: Default::default(),
				max_pov_size: Default::default(),
			},
			downward_messages: Default::default(),
			horizontal_messages: Default::default(),
			relay_chain_state: proof,
		};

		inherent_data.put_data(INHERENT_IDENTIFIER, &data)
	}

	fn error_to_string(&self, error: &[u8]) -> Option<String> {
		InherentError::try_from(&INHERENT_IDENTIFIER, error).map(|e| format!("{:?}", e))
	}
}
