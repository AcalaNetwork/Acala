// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

use frame_support::dispatch::DispatchInfo;
use frame_system::pallet_prelude::*;
use module_support::AddressMapping;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{DispatchInfoOf, Dispatchable, One, SignedExtension, Zero},
	transaction_validity::{
		InvalidTransaction, TransactionLongevity, TransactionValidity, TransactionValidityError, ValidTransaction,
	},
	SaturatedConversion,
};
use sp_std::vec;

/// Nonce check and increment to give replay protection for transactions.
///
/// # Transaction Validity
///
/// This extension affects `requires` and `provides` tags of validity, but DOES NOT
/// set the `priority` field. Make sure that AT LEAST one of the signed extension sets
/// some kind of priority upon validating transactions.
#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct CheckNonce<T: frame_system::Config + module_evm::Config> {
	#[codec(compact)]
	pub nonce: T::Nonce,
	#[codec(skip)]
	pub is_eth_tx: bool,
	#[codec(skip)]
	pub eth_tx_valid_until: BlockNumberFor<T>,
}

impl<T: frame_system::Config + module_evm::Config> Default for CheckNonce<T> {
	fn default() -> Self {
		Self {
			nonce: 0u32.into(),
			is_eth_tx: false,
			eth_tx_valid_until: 0u32.into(),
		}
	}
}

impl<T: frame_system::Config + module_evm::Config> CheckNonce<T> {
	/// utility constructor. Used only in client/factory code.
	pub fn from(nonce: T::Nonce) -> Self {
		Self {
			nonce,
			is_eth_tx: false,
			eth_tx_valid_until: Zero::zero(),
		}
	}

	pub fn mark_as_ethereum_tx(&mut self, valid_until: BlockNumberFor<T>) {
		self.is_eth_tx = true;
		self.eth_tx_valid_until = valid_until;
	}
}

impl<T: frame_system::Config + module_evm::Config> sp_std::fmt::Debug for CheckNonce<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(
			f,
			"CheckNonce(nonce: {}, is_eth_tx: {}, eth_tx_valid_until: {})",
			self.nonce, self.is_eth_tx, self.eth_tx_valid_until
		)
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T: frame_system::Config + module_evm::Config> SignedExtension for CheckNonce<T>
where
	T::RuntimeCall: Dispatchable<Info = DispatchInfo>,
	T::AddressMapping: AddressMapping<T::AccountId>,
{
	type AccountId = T::AccountId;
	type Call = T::RuntimeCall;
	type AdditionalSigned = ();
	type Pre = ();
	const IDENTIFIER: &'static str = "CheckNonce";

	fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> {
		Ok(())
	}

	fn pre_dispatch(
		self,
		who: &Self::AccountId,
		_call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		let mut account = frame_system::Account::<T>::get(who);
		if self.is_eth_tx {
			// should check evm nonce
			let address = <T as module_evm::Config>::AddressMapping::get_evm_address(who)
				.unwrap_or_else(|| <T as module_evm::Config>::AddressMapping::get_default_evm_address(who));
			let evm_nonce = module_evm::Accounts::<T>::get(address)
				.map(|x| x.nonce)
				.unwrap_or_default();

			if cfg!(feature = "tracing") {
				// skip check when enable tracing feature
			} else if self.nonce != evm_nonce {
				return Err(if self.nonce < evm_nonce {
					InvalidTransaction::Stale
				} else {
					InvalidTransaction::Future
				}
				.into());
			}
		} else if self.nonce != account.nonce {
			return Err(if self.nonce < account.nonce {
				InvalidTransaction::Stale
			} else {
				InvalidTransaction::Future
			}
			.into());
		}
		account.nonce += T::Nonce::one();
		frame_system::Account::<T>::insert(who, account);
		Ok(())
	}

	fn validate(
		&self,
		who: &Self::AccountId,
		_call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> TransactionValidity {
		if self.is_eth_tx {
			// should check evm nonce
			let address = <T as module_evm::Config>::AddressMapping::get_evm_address(who)
				.unwrap_or_else(|| <T as module_evm::Config>::AddressMapping::get_default_evm_address(who));
			let evm_nonce = module_evm::Accounts::<T>::get(address)
				.map(|x| x.nonce)
				.unwrap_or_default();

			if cfg!(feature = "tracing") {
				// skip check when enable tracing feature
			} else if self.nonce < evm_nonce {
				return InvalidTransaction::Stale.into();
			}

			let provides = vec![Encode::encode(&(address, self.nonce))];
			let requires = if evm_nonce < self.nonce {
				vec![Encode::encode(&(address, self.nonce - One::one()))]
			} else {
				vec![]
			};

			let longevity: TransactionLongevity = self.eth_tx_valid_until.saturated_into();

			Ok(ValidTransaction {
				priority: 0,
				requires,
				provides,
				longevity,
				propagate: true,
			})
		} else {
			// check index
			let account = frame_system::Account::<T>::get(who);
			if self.nonce < account.nonce {
				return InvalidTransaction::Stale.into();
			}

			let provides = vec![Encode::encode(&(who, self.nonce))];
			let requires = if account.nonce < self.nonce {
				vec![Encode::encode(&(who, self.nonce - One::one()))]
			} else {
				vec![]
			};

			Ok(ValidTransaction {
				priority: 0,
				requires,
				provides,
				longevity: TransactionLongevity::MAX,
				propagate: true,
			})
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{new_test_ext, AccountId32, RuntimeCall, TestRuntime};
	use frame_support::{assert_noop, assert_ok};

	/// A simple call, which one doesn't matter.
	pub const CALL: &<TestRuntime as frame_system::Config>::RuntimeCall =
		&RuntimeCall::System(frame_system::Call::set_heap_pages { pages: 0u64 });

	#[test]
	fn check_nonce_works() {
		new_test_ext().execute_with(|| {
			let alice = AccountId32::from([8; 32]);
			frame_system::Account::<TestRuntime>::insert(
				&alice,
				frame_system::AccountInfo {
					nonce: 1,
					consumers: 0,
					providers: 0,
					sufficients: 0,
					data: pallet_balances::AccountData::default(),
				},
			);
			let info = DispatchInfo::default();
			// stale
			assert_noop!(
				CheckNonce::<TestRuntime>::from(0).validate(&alice, CALL, &info, 0),
				InvalidTransaction::Stale
			);
			assert_noop!(
				CheckNonce::<TestRuntime>::from(0).pre_dispatch(&alice, CALL, &info, 0),
				InvalidTransaction::Stale
			);
			// correct
			assert_ok!(CheckNonce::<TestRuntime>::from(1).validate(&alice, CALL, &info, 0));
			assert_ok!(CheckNonce::<TestRuntime>::from(1).pre_dispatch(&alice, CALL, &info, 0));
			// future
			assert_ok!(CheckNonce::<TestRuntime>::from(5).validate(&alice, CALL, &info, 0));
			assert_noop!(
				CheckNonce::<TestRuntime>::from(5).pre_dispatch(&alice, CALL, &info, 0),
				InvalidTransaction::Future
			);
		})
	}

	#[test]
	fn check_evm_nonce_works() {
		new_test_ext().execute_with(|| {
			let alice = AccountId32::from([8; 32]);
			frame_system::Account::<TestRuntime>::insert(
				&alice,
				frame_system::AccountInfo {
					nonce: 2,
					consumers: 0,
					providers: 0,
					sufficients: 0,
					data: pallet_balances::AccountData::default(),
				},
			);

			let address =
				<TestRuntime as module_evm::Config>::AddressMapping::get_evm_address(&alice).unwrap_or_else(|| {
					<TestRuntime as module_evm::Config>::AddressMapping::get_default_evm_address(&alice)
				});

			module_evm::Accounts::<TestRuntime>::insert(
				&address,
				module_evm::AccountInfo {
					nonce: 1,
					contract_info: None,
				},
			);

			let info = DispatchInfo::default();
			// stale
			assert_noop!(
				CheckNonce::<TestRuntime> {
					nonce: 0u32,
					is_eth_tx: true,
					eth_tx_valid_until: 10
				}
				.validate(&alice, CALL, &info, 0),
				InvalidTransaction::Stale
			);
			assert_noop!(
				CheckNonce::<TestRuntime> {
					nonce: 0u32,
					is_eth_tx: true,
					eth_tx_valid_until: 10
				}
				.pre_dispatch(&alice, CALL, &info, 0),
				InvalidTransaction::Stale
			);

			assert_eq!(
				CheckNonce::<TestRuntime> {
					nonce: 1u32,
					is_eth_tx: true,
					eth_tx_valid_until: 10
				}
				.validate(&alice, CALL, &info, 0),
				Ok(ValidTransaction {
					priority: 0,
					requires: vec![],
					provides: vec![Encode::encode(&(address, 1u32))],
					longevity: 10,
					propagate: true,
				})
			);
			assert_ok!(CheckNonce::<TestRuntime> {
				nonce: 1u32,
				is_eth_tx: true,
				eth_tx_valid_until: 10
			}
			.pre_dispatch(&alice, CALL, &info, 0),);

			assert_eq!(
				CheckNonce::<TestRuntime> {
					nonce: 3u32,
					is_eth_tx: true,
					eth_tx_valid_until: 10
				}
				.validate(&alice, CALL, &info, 0),
				Ok(ValidTransaction {
					priority: 0,
					requires: vec![Encode::encode(&(address, 2u32))],
					provides: vec![Encode::encode(&(address, 3u32))],
					longevity: 10,
					propagate: true,
				})
			);
			assert_noop!(
				CheckNonce::<TestRuntime> {
					nonce: 3u32,
					is_eth_tx: true,
					eth_tx_valid_until: 10
				}
				.pre_dispatch(&alice, CALL, &info, 0),
				InvalidTransaction::Future
			);
		})
	}
}
