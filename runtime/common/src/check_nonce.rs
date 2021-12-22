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

use codec::{Decode, Encode};
use frame_support::weights::DispatchInfo;
use module_support::AddressMapping;
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{DispatchInfoOf, Dispatchable, One, SignedExtension},
	transaction_validity::{
		InvalidTransaction, TransactionLongevity, TransactionValidity, TransactionValidityError, ValidTransaction,
	},
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
pub struct CheckNonce<T: frame_system::Config + module_evm::Config>(
	#[codec(compact)] pub T::Index,
	#[codec(skip)] pub bool, /* should check evm nonce */
);

impl<T: frame_system::Config + module_evm::Config> CheckNonce<T> {
	/// utility constructor. Used only in client/factory code.
	pub fn from(nonce: T::Index) -> Self {
		Self(nonce, false)
	}
}

impl<T: frame_system::Config + module_evm::Config> sp_std::fmt::Debug for CheckNonce<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "CheckNonce({}, evm: {})", self.0, self.1)
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T: frame_system::Config + module_evm::Config> SignedExtension for CheckNonce<T>
where
	T::Call: Dispatchable<Info = DispatchInfo>,
	T::AddressMapping: AddressMapping<T::AccountId>,
{
	type AccountId = T::AccountId;
	type Call = T::Call;
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
	) -> Result<(), TransactionValidityError> {
		let mut account = frame_system::Account::<T>::get(who);
		if self.1 {
			// should check evm nonce
			let address = <T as module_evm::Config>::AddressMapping::get_evm_address(who)
				.unwrap_or_else(|| <T as module_evm::Config>::AddressMapping::get_default_evm_address(who));
			let account = module_evm::Accounts::<T>::get(&address)
				.unwrap_or_else(|| module_evm::AccountInfo::<T::Index>::new(Default::default(), None));
			if self.0 != account.nonce {
				return Err(if self.0 < account.nonce {
					InvalidTransaction::Stale
				} else {
					InvalidTransaction::Future
				}
				.into());
			}
		} else if self.0 != account.nonce {
			return Err(if self.0 < account.nonce {
				InvalidTransaction::Stale
			} else {
				InvalidTransaction::Future
			}
			.into());
		}
		account.nonce += T::Index::one();
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
		if self.1 {
			// should check evm nonce
			let address = <T as module_evm::Config>::AddressMapping::get_evm_address(who)
				.unwrap_or_else(|| <T as module_evm::Config>::AddressMapping::get_default_evm_address(who));
			let account = module_evm::Accounts::<T>::get(&address)
				.unwrap_or_else(|| module_evm::AccountInfo::<T::Index>::new(Default::default(), None));
			if self.0 < account.nonce {
				return InvalidTransaction::Stale.into();
			}

			let provides = vec![Encode::encode(&(who, self.0, true /* evm */))];
			let requires = if account.nonce < self.0 {
				vec![Encode::encode(&(who, self.0 - One::one(), true /* evm */))]
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
		} else {
			// check index
			let account = frame_system::Account::<T>::get(who);
			if self.0 < account.nonce {
				return InvalidTransaction::Stale.into();
			}

			let provides = vec![Encode::encode(&(who, self.0))];
			let requires = if account.nonce < self.0 {
				vec![Encode::encode(&(who, self.0 - One::one()))]
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
	use crate::mock::{new_test_ext, AccountId32, Call, TestRuntime};
	use frame_support::{assert_noop, assert_ok};

	/// A simple call, which one doesn't matter.
	pub const CALL: &<TestRuntime as frame_system::Config>::Call =
		&Call::System(frame_system::Call::set_heap_pages { pages: 0u64 });

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
				CheckNonce::<TestRuntime>(0, false).validate(&alice, CALL, &info, 0),
				InvalidTransaction::Stale
			);
			assert_noop!(
				CheckNonce::<TestRuntime>(0, false).pre_dispatch(&alice, CALL, &info, 0),
				InvalidTransaction::Stale
			);
			// correct
			assert_ok!(CheckNonce::<TestRuntime>(1, false).validate(&alice, CALL, &info, 0));
			assert_ok!(CheckNonce::<TestRuntime>(1, false).pre_dispatch(&alice, CALL, &info, 0));
			// future
			assert_ok!(CheckNonce::<TestRuntime>(5, false).validate(&alice, CALL, &info, 0));
			assert_noop!(
				CheckNonce::<TestRuntime>(5, false).pre_dispatch(&alice, CALL, &info, 0),
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
				CheckNonce::<TestRuntime>(0, true).validate(&alice, CALL, &info, 0),
				InvalidTransaction::Stale
			);
			assert_noop!(
				CheckNonce::<TestRuntime>(0, true).pre_dispatch(&alice, CALL, &info, 0),
				InvalidTransaction::Stale
			);

			let nonce: primitives::Nonce = 1;
			assert_eq!(
				CheckNonce::<TestRuntime>(nonce, true).validate(&alice, CALL, &info, 0),
				Ok(ValidTransaction {
					priority: 0,
					requires: vec![],
					provides: vec![Encode::encode(&(alice.clone(), nonce, true))],
					longevity: TransactionLongevity::MAX,
					propagate: true,
				})
			);
			assert_ok!(CheckNonce::<TestRuntime>(nonce, true).pre_dispatch(&alice, CALL, &info, 0));

			let nonce: primitives::Nonce = 3;
			assert_eq!(
				CheckNonce::<TestRuntime>(nonce, true).validate(&alice, CALL, &info, 0),
				Ok(ValidTransaction {
					priority: 0,
					requires: vec![Encode::encode(&(alice.clone(), nonce - 1, true))],
					provides: vec![Encode::encode(&(alice.clone(), nonce, true))],
					longevity: TransactionLongevity::MAX,
					propagate: true,
				})
			);
			assert_noop!(
				CheckNonce::<TestRuntime>(nonce, true).pre_dispatch(&alice, CALL, &info, 0),
				InvalidTransaction::Future
			);
		})
	}
}
