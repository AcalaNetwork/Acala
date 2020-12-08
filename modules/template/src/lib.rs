#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_event, decl_module, decl_storage, traits::Currency};

pub trait Config: frame_system::Config {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
	type Currency: Currency<Self::AccountId>;
}

type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

decl_storage! {
	trait Store for Module<T: Config> as Template {

	}
}

decl_event!(
	pub enum Event<T> where
		<T as frame_system::Config>::AccountId,
		Balance = BalanceOf<T>,
	{
		Dummy(AccountId, Balance),
	}
);

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

	}
}

impl<T: Config> Module<T> {}
