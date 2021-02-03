#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

mod mock;
mod tests;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use codec::Encode;
	use frame_support::{pallet_prelude::*, traits::LockIdentifier, transactional};
	use frame_system::pallet_prelude::*;
	use orml_traits::{BasicCurrency, BasicLockableCurrency};
	use primitives::{Balance, EraIndex};
	use sp_runtime::{
		traits::{MaybeDisplay, MaybeSerializeDeserialize, Member, Zero},
		RuntimeDebug, SaturatedConversion,
	};
	use sp_std::{fmt::Debug, prelude::*};
	use support::{NomineesProvider, OnNewEra};

	pub const NOMINEES_ELECTION_ID: LockIdentifier = *b"nomelect";

	/// Just a Balance/BlockNumber tuple to encode when a chunk of funds will be
	/// unlocked.
	#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
	pub struct UnlockChunk {
		/// Amount of funds to be unlocked.
		pub(crate) value: Balance,
		/// Era number at which point it'll be unlocked.
		era: EraIndex,
	}

	/// The ledger of a (bonded) account.
	#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Default)]
	pub struct BondingLedger {
		/// The total amount of the account's balance that we are currently
		/// accounting for. It's just `active` plus all the `unlocking`
		/// balances.
		pub total: Balance,
		/// The total amount of the account's balance that will be at stake in
		/// any forthcoming rounds.
		pub active: Balance,
		/// Any balance that is becoming free, which may eventually be
		/// transferred out of the account.
		pub unlocking: Vec<UnlockChunk>,
	}

	impl BondingLedger {
		/// Remove entries from `unlocking` that are sufficiently old and reduce
		/// the total by the sum of their balances.
		fn consolidate_unlocked(self, current_era: EraIndex) -> Self {
			let mut total = self.total;
			let unlocking = self
				.unlocking
				.into_iter()
				.filter(|chunk| {
					if chunk.era > current_era {
						true
					} else {
						total = total.saturating_sub(chunk.value);
						false
					}
				})
				.collect();

			Self {
				total,
				active: self.active,
				unlocking,
			}
		}

		/// Re-bond funds that were scheduled for unlocking.
		fn rebond(mut self, value: Balance) -> Self {
			let mut unlocking_balance: Balance = Zero::zero();

			while let Some(last) = self.unlocking.last_mut() {
				if unlocking_balance + last.value <= value {
					unlocking_balance += last.value;
					self.active += last.value;
					self.unlocking.pop();
				} else {
					let diff = value - unlocking_balance;

					unlocking_balance += diff;
					self.active += diff;
					last.value -= diff;
				}

				if unlocking_balance >= value {
					break;
				}
			}

			self
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Currency: BasicLockableCurrency<Self::AccountId, Moment = Self::BlockNumber, Balance = Balance>;
		type PolkadotAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord + Default;
		#[pallet::constant]
		type MinBondThreshold: Get<Balance>;
		#[pallet::constant]
		type BondingDuration: Get<EraIndex>;
		#[pallet::constant]
		type NominateesCount: Get<u32>;
		#[pallet::constant]
		type MaxUnlockingChunks: Get<u32>;
	}

	#[pallet::error]
	pub enum Error<T> {
		BelowMinBondThreshold,
		InvalidTargetsLength,
		TooManyChunks,
		NoBonded,
		NoUnlockChunk,
	}

	#[pallet::storage]
	#[pallet::getter(fn nominations)]
	pub type Nominations<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, Vec<T::PolkadotAccountId>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn ledger)]
	pub type Ledger<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, BondingLedger, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn votes)]
	pub type Votes<T: Config> = StorageMap<_, Twox64Concat, T::PolkadotAccountId, Balance, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn nominees)]
	pub type Nominees<T: Config> = StorageValue<_, Vec<T::PolkadotAccountId>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn current_era)]
	pub type CurrentEra<T: Config> = StorageValue<_, EraIndex, ValueQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10000)]
		#[transactional]
		pub fn bond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let mut ledger = Self::ledger(&who);
			let free_balance = T::Currency::free_balance(&who);
			if let Some(extra) = free_balance.checked_sub(ledger.total) {
				let extra = extra.min(amount);
				let old_active = ledger.active;
				ledger.active += extra;
				ensure!(
					ledger.active >= T::MinBondThreshold::get(),
					Error::<T>::BelowMinBondThreshold
				);
				ledger.total += extra;
				let old_nominations = Self::nominations(&who);

				Self::update_votes(old_active, &old_nominations, ledger.active, &old_nominations);
				Self::update_ledger(&who, &ledger);
			}
			Ok(().into())
		}

		#[pallet::weight(10000)]
		#[transactional]
		pub fn unbond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let mut ledger = Self::ledger(&who);
			ensure!(
				ledger.unlocking.len() < T::MaxUnlockingChunks::get().saturated_into(),
				Error::<T>::TooManyChunks,
			);

			let amount = amount.min(ledger.active);

			if !amount.is_zero() {
				let old_active = ledger.active;
				ledger.active -= amount;

				ensure!(
					ledger.active.is_zero() || ledger.active >= T::MinBondThreshold::get(),
					Error::<T>::BelowMinBondThreshold,
				);

				// Note: in case there is no current era it is fine to bond one era more.
				let era = Self::current_era() + T::BondingDuration::get();
				ledger.unlocking.push(UnlockChunk { value: amount, era });
				let old_nominations = Self::nominations(&who);

				Self::update_votes(old_active, &old_nominations, ledger.active, &old_nominations);
				Self::update_ledger(&who, &ledger);
			}
			Ok(().into())
		}

		#[pallet::weight(10000)]
		#[transactional]
		pub fn rebond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let ledger = Self::ledger(&who);
			ensure!(!ledger.unlocking.is_empty(), Error::<T>::NoUnlockChunk,);
			let old_active = ledger.active;
			let old_nominations = Self::nominations(&who);
			let ledger = ledger.rebond(amount);

			Self::update_votes(old_active, &old_nominations, ledger.active, &old_nominations);
			Self::update_ledger(&who, &ledger);
			Ok(().into())
		}

		#[pallet::weight(10000)]
		#[transactional]
		pub fn withdraw_unbonded(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let ledger = Self::ledger(&who).consolidate_unlocked(Self::current_era());

			if ledger.unlocking.is_empty() && ledger.active.is_zero() {
				Self::remove_ledger(&who);
			} else {
				// This was the consequence of a partial unbond. just update the ledger and move
				// on.
				Self::update_ledger(&who, &ledger);
			}
			Ok(().into())
		}

		#[pallet::weight(10000)]
		#[transactional]
		pub fn nominate(origin: OriginFor<T>, targets: Vec<T::PolkadotAccountId>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(
				!targets.is_empty() && targets.len() <= T::NominateesCount::get().saturated_into(),
				Error::<T>::InvalidTargetsLength,
			);

			let ledger = Self::ledger(&who);
			ensure!(!ledger.total.is_zero(), Error::<T>::NoBonded);

			let mut targets = targets;
			targets.sort();
			targets.dedup();

			let old_nominations = Self::nominations(&who);
			let old_active = Self::ledger(&who).active;

			Self::update_votes(old_active, &old_nominations, old_active, &targets);
			Nominations::<T>::insert(&who, &targets);
			Ok(().into())
		}

		#[pallet::weight(10000)]
		#[transactional]
		pub fn chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let old_nominations = Self::nominations(&who);
			let old_active = Self::ledger(&who).active;

			Self::update_votes(old_active, &old_nominations, Zero::zero(), &[]);
			Nominations::<T>::remove(&who);
			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		fn update_ledger(who: &T::AccountId, ledger: &BondingLedger) {
			let _ = T::Currency::set_lock(NOMINEES_ELECTION_ID, who, ledger.total);
			Ledger::<T>::insert(who, ledger);
		}

		fn remove_ledger(who: &T::AccountId) {
			let _ = T::Currency::remove_lock(NOMINEES_ELECTION_ID, who);
			Ledger::<T>::remove(who);
			Nominations::<T>::remove(who);
		}

		pub(crate) fn update_votes(
			old_active: Balance,
			old_nominations: &[T::PolkadotAccountId],
			new_active: Balance,
			new_nominations: &[T::PolkadotAccountId],
		) {
			if !old_active.is_zero() && !old_nominations.is_empty() {
				for account in old_nominations {
					Votes::<T>::mutate(account, |balance| *balance = balance.saturating_sub(old_active));
				}
			}

			if !new_active.is_zero() && !new_nominations.is_empty() {
				for account in new_nominations {
					Votes::<T>::mutate(account, |balance| *balance = balance.saturating_add(new_active));
				}
			}
		}

		pub(crate) fn rebalance() {
			let mut voters = Votes::<T>::iter().collect::<Vec<(T::PolkadotAccountId, Balance)>>();

			voters.sort_by(|a, b| b.1.cmp(&a.1));

			let new_nominees = voters
				.into_iter()
				.take(T::NominateesCount::get().saturated_into())
				.map(|(nominee, _)| nominee)
				.collect::<Vec<_>>();

			Nominees::<T>::put(new_nominees);
		}
	}

	impl<T: Config> NomineesProvider<T::PolkadotAccountId> for Pallet<T> {
		fn nominees() -> Vec<T::PolkadotAccountId> {
			Self::rebalance();
			Nominees::<T>::get()
		}
	}

	impl<T: Config> OnNewEra<EraIndex> for Pallet<T> {
		fn on_new_era(era: EraIndex) {
			CurrentEra::<T>::put(era);
			Self::rebalance();
		}
	}
}
