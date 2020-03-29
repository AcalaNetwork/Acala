#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, traits::Get, Parameter};
use orml_traits::BasicCurrency;
use rstd::fmt::Debug;
use rstd::prelude::*;
use sp_runtime::traits::{CheckedAdd, CheckedSub, MaybeDisplay, MaybeSerializeDeserialize, Member, Saturating, Zero};
use support::{
	EraIndex, OnNewEra, PolkadotBridge, PolkadotBridgeCall, PolkadotBridgeState, PolkadotBridgeType,
	PolkadotStakingLedger, PolkadotUnlockChunk, Rate,
};
use system::{self as system, ensure_root, ensure_signed};

type BalanceOf<T> = <<T as Trait>::DOTCurrency as BasicCurrency<<T as system::Trait>::AccountId>>::Balance;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type DOTCurrency: BasicCurrency<Self::AccountId>;
	type OnNewEra: OnNewEra;
	type BondingDuration: Get<EraIndex>;
	type EraLength: Get<Self::BlockNumber>;
	type PolkadotAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord + Default;
}

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
		Balance = BalanceOf<T>,
	{
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
		pub Bonded get(bonded): BalanceOf<T>;	// active
		pub Available get(available): BalanceOf<T>; // balance - bonded
		pub Unbonding get(unbonding): Vec<(BalanceOf<T>, EraIndex)>;
		pub CurrentEra get(current_era): EraIndex;
		pub EraStartBlockNumber get(fn era_start_block_number): T::BlockNumber;
		pub ForcedEra get(forced_era): Option<T::BlockNumber>;
		pub MockRewardRate get(fn mock_reward_rate) config(): Option<Rate>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;
		fn deposit_event() = default;

		const BondingDuration: EraIndex = T::BondingDuration::get();
		const EraLength: T::BlockNumber = T::EraLength::get();

		pub fn set_mock_reward_rate(origin, mock_reward_rate: Option<Rate>) {
			ensure_root(origin)?;
			if let Some(mock_reward_rate) = mock_reward_rate {
				MockRewardRate::put(mock_reward_rate);
			} else {
				MockRewardRate::kill();
			}
		}

		pub fn simulate_slash(origin, amount: BalanceOf<T>) {
			ensure_root(origin)?;
			<Bonded<T>>::mutate(|balance| *balance = balance.saturating_sub(amount));
		}

		pub fn simualte_receive(origin, to: T::AccountId, amount: BalanceOf<T>) {
			ensure_root(origin)?;
			let new_available = Self::available().checked_sub(&amount).ok_or(Error::<T>::NotEnough)?;
			T::DOTCurrency::deposit(&to, amount)?;
			<Available<T>>::put(new_available);
		}

		pub fn simulate_redeem(origin, _to: T::PolkadotAccountId, amount: BalanceOf<T>) {
			let from = ensure_signed(origin)?;
			let new_available = Self::available().checked_add(&amount).ok_or(Error::<T>::Overflow)?;
			T::DOTCurrency::withdraw(&from, amount)?;
			<Available<T>>::put(new_available);
		}

		pub fn force_era(origin, at: T::BlockNumber) {
			ensure_root(origin)?;
			if at > <system::Module<T>>::block_number() {
				<ForcedEra<T>>::put(at);
			}
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

impl<T: Trait> PolkadotBridgeType<T::BlockNumber> for Module<T> {
	type BondingDuration = T::BondingDuration;
	type EraLength = T::EraLength;
	type PolkadotAccountId = T::PolkadotAccountId;
}

impl<T: Trait> PolkadotBridgeCall<T::BlockNumber, BalanceOf<T>, T::AccountId> for Module<T> {
	// simulate bond extra
	fn bond_extra(amount: BalanceOf<T>) {
		let free_balance = Self::available();
		let amount = amount.min(free_balance);

		if !amount.is_zero() {
			<Bonded<T>>::mutate(|balance| *balance += amount);
			<Available<T>>::mutate(|balance| *balance -= amount);
		}
	}

	// simulate unbond
	fn unbond(amount: BalanceOf<T>) {
		let amount = amount.min(Self::bonded());

		if !amount.is_zero() {
			let mut unbonding = Self::unbonding();
			unbonding.push((amount, Self::current_era() + T::BondingDuration::get()));

			<Bonded<T>>::mutate(|bonded| *bonded -= amount);
			<Unbonding<T>>::put(unbonding);
		}
	}

	// simulate rebond
	fn rebond(amount: BalanceOf<T>) {
		let mut unbonding = Self::unbonding();
		let mut bonded = Self::bonded();
		let mut unbonding_balance: BalanceOf<T> = Zero::zero();

		while let Some(last) = unbonding.last_mut() {
			if unbonding_balance + last.0 <= amount {
				unbonding_balance += last.0;
				bonded += last.0;
				unbonding.pop();
			} else {
				let diff = amount - unbonding_balance;

				unbonding_balance += diff;
				bonded += diff;
				last.0 -= diff;
			}

			if unbonding_balance >= amount {
				break;
			}
		}

		<Bonded<T>>::put(bonded);
		<Unbonding<T>>::put(unbonding);
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

		<Available<T>>::put(available);
		<Unbonding<T>>::put(unbonding);
	}

	fn nominate(_targets: Vec<Self::PolkadotAccountId>) {}

	// simulate transfer dot from acala to parachain account in polkadot
	fn transfer_to_bridge(from: &T::AccountId, amount: BalanceOf<T>) {
		if T::DOTCurrency::ensure_can_withdraw(from, amount).is_ok() {
			T::DOTCurrency::withdraw(from, amount).expect("never failed after check");
			<Available<T>>::mutate(|balance| *balance = balance.saturating_add(amount));
		}
	}

	// simulate receive dot from parachain account in polkadot to acala
	fn receive_from_bridge(to: &T::AccountId, amount: BalanceOf<T>) {
		if let Some(new_available) = Self::available().checked_sub(&amount) {
			<Available<T>>::put(new_available);
			T::DOTCurrency::deposit(&to, amount).expect("shouldn't fail");
		}
	}

	// simulate receive staking reward
	fn payout_nominator() {
		if let Some(mock_reward_rate) = Self::mock_reward_rate() {
			let reward = mock_reward_rate.saturating_mul_int(&Self::bonded());
			<Available<T>>::mutate(|balance| *balance = balance.saturating_add(reward));
		}
	}
}

impl<T: Trait> PolkadotBridgeState<BalanceOf<T>> for Module<T> {
	fn ledger() -> PolkadotStakingLedger<BalanceOf<T>> {
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
			total: total,
			active: active,
			unlocking: unlocking,
		}
	}

	fn balance() -> BalanceOf<T> {
		// bonded + total_unlocking + available
		let mut total = Self::bonded().saturating_add(Self::available());

		for (balance, _) in Self::unbonding() {
			total = total.saturating_add(balance);
		}

		total
	}

	fn current_era() -> EraIndex {
		Self::current_era()
	}
}

impl<T: Trait> PolkadotBridge<T::BlockNumber, BalanceOf<T>, T::AccountId> for Module<T> {}
