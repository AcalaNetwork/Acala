#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{debug, decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get, Parameter};
use frame_system::{self as system, ensure_root, ensure_signed};
use orml_traits::BasicCurrency;
use orml_utilities::with_transaction_result;
use primitives::{Balance, EraIndex};
use sp_runtime::{
	traits::{CheckedSub, MaybeDisplay, MaybeSerializeDeserialize, Member, Zero},
	DispatchResult, FixedPointNumber,
};
use sp_std::{fmt::Debug, prelude::*};
use support::{
	OnNewEra, PolkadotBridge, PolkadotBridgeCall, PolkadotBridgeState, PolkadotBridgeType, PolkadotStakingLedger,
	PolkadotUnlockChunk, Rate,
};

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type DOTCurrency: BasicCurrency<Self::AccountId, Balance = Balance>;
	type OnNewEra: OnNewEra<EraIndex>;
	type BondingDuration: Get<EraIndex>;
	type EraLength: Get<Self::BlockNumber>;
	type PolkadotAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord + Default;
}

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
		Balance = Balance,
	{
		/// [account, amount]
		Mint(AccountId, Balance),
	}
);

decl_error! {
	/// Error for polkadot bridge module.
	pub enum Error for Module<T: Trait> {
		NotEnough,
		Overflow,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as PolkadotBridge {
		pub Bonded get(fn bonded): Balance;	// active
		pub Available get(fn available): Balance; // balance - bonded
		pub Unbonding get(fn unbonding): Vec<(Balance, EraIndex)>;
		pub CurrentEra get(fn current_era): EraIndex;
		pub EraStartBlockNumber get(fn era_start_block_number): T::BlockNumber;
		pub ForcedEra get(fn forced_era): Option<T::BlockNumber>;
		pub MockRewardRate get(fn mock_reward_rate) config(): Option<Rate>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;
		fn deposit_event() = default;

		const BondingDuration: EraIndex = T::BondingDuration::get();
		const EraLength: T::BlockNumber = T::EraLength::get();

		#[weight = 10_000]
		pub fn set_mock_reward_rate(origin, mock_reward_rate: Option<Rate>) {
			with_transaction_result(|| {
				ensure_root(origin)?;
				if let Some(mock_reward_rate) = mock_reward_rate {
					MockRewardRate::put(mock_reward_rate);
				} else {
					MockRewardRate::kill();
				}
				Ok(())
			})?;
		}

		#[weight = 10_000]
		pub fn simulate_bond(origin, amount: Balance) {
			with_transaction_result(|| {
				ensure_root(origin)?;
				Self::bond_extra(amount)?;
				Ok(())
			})?;
		}

		#[weight = 10_000]
		pub fn simulate_unbond(origin, amount: Balance) {
			with_transaction_result(|| {
				ensure_root(origin)?;
				Self::unbond(amount)?;
				Ok(())
			})?;
		}

		#[weight = 10_000]
		pub fn simulate_withdraw_unbonded(origin) {
			with_transaction_result(|| {
				let _ = ensure_signed(origin)?;
				Self::withdraw_unbonded();
				Ok(())
			})?;
		}

		#[weight = 10_000]
		pub fn simulate_slash(origin, amount: Balance) {
			with_transaction_result(|| {
				ensure_root(origin)?;
				Bonded::mutate(|balance| *balance = balance.saturating_sub(amount));
				Ok(())
			})?;
		}

		#[weight = 10_000]
		pub fn simualte_receive(origin, to: T::AccountId, amount: Balance) {
			with_transaction_result(|| {
				ensure_root(origin)?;
				let new_available = Self::available().checked_sub(amount).ok_or(Error::<T>::NotEnough)?;
				T::DOTCurrency::deposit(&to, amount)?;
				Available::put(new_available);
				Ok(())
			})?;
		}

		#[weight = 10_000]
		pub fn simulate_redeem(origin, _to: T::PolkadotAccountId, amount: Balance) {
			with_transaction_result(|| {
				let from = ensure_signed(origin)?;
				let new_available = Self::available().checked_add(amount).ok_or(Error::<T>::Overflow)?;
				T::DOTCurrency::withdraw(&from, amount)?;
				Available::put(new_available);
				Ok(())
			})?;
		}

		#[weight = 10_000]
		pub fn force_era(origin, at: T::BlockNumber) {
			with_transaction_result(|| {
				ensure_root(origin)?;
				if at > <system::Module<T>>::block_number() {
					<ForcedEra<T>>::put(at);
				}
				Ok(())
			})?;
		}

		fn on_finalize(now: T::BlockNumber) {
			let force_era = Self::forced_era().map_or(false, |block| {
				if block == now {
					<ForcedEra<T>>::kill();
					true
				} else {
					false
				}
			});
			let len = now.checked_sub(&Self::era_start_block_number()).unwrap_or_default();

			if len >= T::EraLength::get() || force_era {
				Self::new_era(now);
			}
		}
	}
}

impl<T: Trait> Module<T> {
	fn new_era(now: T::BlockNumber) {
		let new_era = CurrentEra::mutate(|era| {
			*era += 1;
			*era
		});
		<EraStartBlockNumber<T>>::put(now);
		T::OnNewEra::on_new_era(new_era);
	}
}

impl<T: Trait> PolkadotBridgeType<T::BlockNumber, EraIndex> for Module<T> {
	type BondingDuration = T::BondingDuration;
	type EraLength = T::EraLength;
	type PolkadotAccountId = T::PolkadotAccountId;
}

impl<T: Trait> PolkadotBridgeCall<T::AccountId, T::BlockNumber, Balance, EraIndex> for Module<T> {
	// simulate bond extra
	fn bond_extra(amount: Balance) -> DispatchResult {
		let free_balance = Self::available();

		if !amount.is_zero() {
			ensure!(free_balance >= amount, Error::<T>::NotEnough);
			Bonded::mutate(|balance| *balance += amount);
			Available::mutate(|balance| *balance -= amount);

			debug::debug!(
				target: "polkadot bridge simulator",
				"bond extra: {:?}",
				amount,
			);
		}

		Ok(())
	}

	// simulate unbond
	fn unbond(amount: Balance) -> DispatchResult {
		let bonded = Self::bonded();

		if !amount.is_zero() {
			ensure!(bonded >= amount, Error::<T>::NotEnough);
			let mut unbonding = Self::unbonding();
			let current_era = Self::current_era();
			let unbonded_era_index = current_era + T::BondingDuration::get();
			unbonding.push((amount, unbonded_era_index));

			Bonded::mutate(|bonded| *bonded -= amount);
			Unbonding::put(unbonding);

			debug::debug!(
				target: "polkadot bridge simulator",
				"unbond: {:?} at {:?}",
				amount, current_era,
			);
		}

		Ok(())
	}

	// simulate rebond
	fn rebond(amount: Balance) -> DispatchResult {
		let mut unbonding = Self::unbonding();
		let mut bonded = Self::bonded();
		let mut rebond_balance: Balance = Zero::zero();

		while let Some(last) = unbonding.last_mut() {
			if rebond_balance + last.0 <= amount {
				rebond_balance += last.0;
				bonded += last.0;
				unbonding.pop();
			} else {
				let diff = amount - rebond_balance;

				rebond_balance += diff;
				bonded += diff;
				last.0 -= diff;
			}

			if rebond_balance >= amount {
				break;
			}
		}
		ensure!(rebond_balance >= amount, Error::<T>::NotEnough);
		if !rebond_balance.is_zero() {
			Bonded::put(bonded);
			Unbonding::put(unbonding);

			debug::debug!(
				target: "polkadot bridge simulator",
				"rebond: {:?}",
				rebond_balance,
			);
		}
		Ok(())
	}

	// simulate withdraw unbonded
	fn withdraw_unbonded() {
		let current_era = Self::current_era();
		let mut available = Self::available();
		let unbonding = Self::unbonding()
			.into_iter()
			.filter(|(value, era_index)| {
				if *era_index > current_era {
					true
				} else {
					available = available.saturating_add(*value);
					false
				}
			})
			.collect::<Vec<_>>();

		Available::put(available);
		Unbonding::put(unbonding);
	}

	// simulate receive staking reward
	fn payout_nominator() {
		if let Some(mock_reward_rate) = Self::mock_reward_rate() {
			let reward = mock_reward_rate.saturating_mul_int(Self::bonded());
			Available::mutate(|balance| *balance = balance.saturating_add(reward));

			debug::debug!(
				target: "polkadot bridge simulator",
				"get reward: {:?}",
				reward,
			);
		}
	}

	fn nominate(_targets: Vec<Self::PolkadotAccountId>) {}

	// simulate transfer dot from acala to parachain account in polkadot
	fn transfer_to_bridge(from: &T::AccountId, amount: Balance) -> DispatchResult {
		T::DOTCurrency::withdraw(from, amount)?;
		Available::mutate(|balance| *balance = balance.saturating_add(amount));
		Ok(())
	}

	// simulate receive dot from parachain account in polkadot to acala
	fn receive_from_bridge(to: &T::AccountId, amount: Balance) -> DispatchResult {
		let new_available = Self::available().checked_sub(amount).ok_or(Error::<T>::NotEnough)?;
		Available::put(new_available);
		T::DOTCurrency::deposit(&to, amount)?;
		Ok(())
	}
}

impl<T: Trait> PolkadotBridgeState<Balance, EraIndex> for Module<T> {
	fn ledger() -> PolkadotStakingLedger<Balance, EraIndex> {
		let active = Self::bonded();
		let mut total = active;
		let unlocking = Self::unbonding()
			.into_iter()
			.map(|(balance, era_index)| {
				total = total.saturating_add(balance);
				PolkadotUnlockChunk {
					value: balance,
					era: era_index,
				}
			})
			.collect::<_>();

		PolkadotStakingLedger {
			total,
			active,
			unlocking,
		}
	}

	fn balance() -> Balance {
		// bonded + total_unlocking + available
		Self::unbonding()
			.iter()
			.fold(Self::bonded().saturating_add(Self::available()), |x, (balance, _)| {
				x.saturating_add(*balance)
			})
	}

	fn current_era() -> EraIndex {
		Self::current_era()
	}
}

impl<T: Trait> PolkadotBridge<T::AccountId, T::BlockNumber, Balance, EraIndex> for Module<T> {}
