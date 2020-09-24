#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get, weights::Weight};
use frame_system::{self as system, ensure_none, ensure_signed};
use orml_traits::BasicCurrency;
use primitives::Balance;
use sp_io::{crypto::secp256k1_ecdsa_recover, hashing::keccak_256};
use sp_runtime::{
	transaction_validity::{
		InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity, ValidTransaction,
	},
	DispatchResult,
};
use sp_std::vec::Vec;

mod mock;
mod tests;

#[derive(Encode, Decode, Clone)]
pub struct EcdsaSignature(pub [u8; 65]);

impl PartialEq for EcdsaSignature {
	fn eq(&self, other: &Self) -> bool {
		self.0[..] == other.0[..]
	}
}

impl sp_std::fmt::Debug for EcdsaSignature {
	fn fmt(&self, f: &mut sp_std::fmt::Formatter<'_>) -> sp_std::fmt::Result {
		write!(f, "EcdsaSignature({:?})", &self.0[..])
	}
}

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type Currency: BasicCurrency<Self::AccountId, Balance = Balance>;
	/// The RenVM split public key
	type PublicKey: Get<[u8; 20]>;
	/// The RenVM Currency identifier
	type CurrencyIdentifier: Get<[u8; 32]>;
	/// A configuration for base priority of unsigned transactions.
	///
	/// This is exposed so that it can be tuned for particular runtime, when
	/// multiple modules send unsigned transactions.
	type UnsignedPriority: Get<TransactionPriority>;

	/// Record burn event details when burn occurs until x blocks have passed
	type BurnEventStoreDuration: Get<Self::BlockNumber>;
}

decl_storage! {
	trait Store for Module<T: Trait> as Template {
		/// Signature blacklist. This is required to prevent double claim.
		Signatures get(fn signatures): map hasher(opaque_twox_256) EcdsaSignature => Option<()>;

		/// Record burn event details
		BurnEvents get(fn burn_events): map hasher(twox_64_concat) T::BlockNumber => Vec<([u8; 20], Balance)>
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		DestAddress = [u8; 20],
	{
		/// Asset minted. \[owner, amount\]
		Minted(AccountId, Balance),
		/// Asset burnt in this chain \[owner, dest, amount\]
		Burnt(AccountId, DestAddress, Balance),
	}
);

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// The mint signature is invalid.
		InvalidMintSignature,
		/// The mint signature has already been used.
		SignatureAlreadyUsed,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Allow a user to mint if they have a valid signature from RenVM.
		///
		/// The dispatch origin of this call must be _None_.
		///
		/// Verify input by `validate_unsigned`
		#[weight = 10_000]
		fn mint(
			origin,
			who: T::AccountId,
			p_hash: [u8; 32],
			#[compact] amount: Balance,
			n_hash: [u8; 32],
			sig: EcdsaSignature,
		) {
			ensure_none(origin)?;
			Self::do_mint(who, amount, sig)?;
		}

		/// Allow a user to burn assets.
		#[weight = 10_000]
		fn burn(
			origin,
			to: [u8; 20],
			#[compact] amount: Balance,
		) {
			let sender = ensure_signed(origin)?;

			T::Currency::withdraw(&sender, amount)?;
			BurnEvents::<T>::append(
				<frame_system::Module<T>>::block_number() + T::BurnEventStoreDuration::get(),
				(to, amount),
			);

			Self::deposit_event(RawEvent::Burnt(sender, to, amount));
		}

		/// dummy `on_initialize` to return the weight used in `on_finalize`.
		fn on_initialize(now: T::BlockNumber) -> Weight {
			0
		}

		fn on_finalize(now: T::BlockNumber) {
			BurnEvents::<T>::remove(now);
		}
	}
}

impl<T: Trait> Module<T> {
	fn do_mint(sender: T::AccountId, amount: Balance, sig: EcdsaSignature) -> DispatchResult {
		T::Currency::deposit(&sender, amount)?;
		Signatures::insert(&sig, ());

		Self::deposit_event(RawEvent::Minted(sender, amount));
		Ok(())
	}

	// ABI-encode the values for creating the signature hash.
	fn signable_message(p_hash: &[u8; 32], amount: u128, to: &[u8], n_hash: &[u8; 32], token: &[u8; 32]) -> Vec<u8> {
		// p_hash ++ amount ++ token ++ to ++ n_hash
		let length = 32 + 32 + 32 + 32 + 32;
		let mut v = Vec::with_capacity(length);
		v.extend_from_slice(&p_hash[..]);
		v.extend_from_slice(&[0u8; 16][..]);
		v.extend_from_slice(&amount.to_be_bytes()[..]);
		v.extend_from_slice(&token[..]);
		v.extend_from_slice(to);
		v.extend_from_slice(&n_hash[..]);
		v
	}

	// Verify that the signature has been signed by RenVM.
	fn verify_signature(
		p_hash: &[u8; 32],
		amount: u128,
		to: &[u8],
		n_hash: &[u8; 32],
		sig: &[u8; 65],
	) -> DispatchResult {
		let ren_btc_identifier = T::CurrencyIdentifier::get();

		let signed_message_hash = keccak_256(&Self::signable_message(p_hash, amount, to, n_hash, &ren_btc_identifier));
		let recoverd =
			secp256k1_ecdsa_recover(&sig, &signed_message_hash).map_err(|_| Error::<T>::InvalidMintSignature)?;
		let addr = &keccak_256(&recoverd)[12..];

		ensure!(addr == T::PublicKey::get(), Error::<T>::InvalidMintSignature);

		Ok(())
	}
}

#[allow(deprecated)]
impl<T: Trait> frame_support::unsigned::ValidateUnsigned for Module<T> {
	type Call = Call<T>;

	fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
		if let Call::mint(who, p_hash, amount, n_hash, sig) = call {
			// check if already exists
			if Signatures::contains_key(&sig) {
				return InvalidTransaction::Stale.into();
			}

			let verify_result = Encode::using_encoded(&who, |encoded| -> DispatchResult {
				Self::verify_signature(&p_hash, *amount, encoded, &n_hash, &sig.0)
			});

			// verify signature
			if verify_result.is_err() {
				return InvalidTransaction::BadProof.into();
			}

			ValidTransaction::with_tag_prefix("renvm-bridge")
				.priority(T::UnsignedPriority::get())
				.and_provides(sig)
				.longevity(64_u64)
				.propagate(true)
				.build()
		} else {
			InvalidTransaction::Call.into()
		}
	}
}
