// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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
use frame_system::{BlockHash, Config, Pallet};
use scale_info::TypeInfo;
use sp_runtime::{
	generic::Era,
	traits::{DispatchInfoOf, SaturatedConversion, SignedExtension},
	transaction_validity::{InvalidTransaction, TransactionValidity, TransactionValidityError, ValidTransaction},
};

/// Check for transaction mortality.
///
/// # Transaction Validity
///
/// The extension affects `longevity` of the transaction according to the [`Era`] definition.
#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct CheckMortality<T: Config + Send + Sync>(pub Era, sp_std::marker::PhantomData<T>);

impl<T: Config + Send + Sync> CheckMortality<T> {
	/// utility constructor. Used only in client/factory code.
	pub fn from(era: Era) -> Self {
		Self(era, sp_std::marker::PhantomData)
	}
}

impl<T: Config + Send + Sync> sp_std::fmt::Debug for CheckMortality<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "CheckMortality({:?})", self.0)
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T: Config + Send + Sync> SignedExtension for CheckMortality<T> {
	type AccountId = T::AccountId;
	type Call = T::Call;
	type AdditionalSigned = T::Hash;
	type Pre = ();
	const IDENTIFIER: &'static str = "CheckMortality";

	fn validate(
		&self,
		_who: &Self::AccountId,
		_call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> TransactionValidity {
		let current_u64 = <Pallet<T>>::block_number().saturated_into::<u64>();
		let valid_till = self.0.death(current_u64);
		Ok(ValidTransaction {
			longevity: valid_till.saturating_sub(current_u64),
			..Default::default()
		})
	}

	fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
		let current_u64 = <Pallet<T>>::block_number().saturated_into::<u64>();
		let n = self.0.birth(current_u64).saturated_into::<T::BlockNumber>();
		if !<BlockHash<T>>::contains_key(n) {
			Err(InvalidTransaction::AncientBirthBlock.into())
		} else {
			Ok(<Pallet<T>>::block_hash(n))
		}
	}

	fn pre_dispatch(
		self,
		who: &Self::AccountId,
		call: &Self::Call,
		info: &DispatchInfoOf<Self::Call>,
		len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		self.validate(who, call, info, len).map(|_| ())
	}
}
