#![cfg_attr(not(feature = "std"), no_std)]

use support::{decl_event, decl_module, decl_storage};

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_storage! {
	trait Store for Module<T: Trait> as Oracle {

	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId
	{
		Dummy(AccountId),
	}
);

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

	}
}

impl<T: Trait> Module<T> {}
