#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_module, decl_storage, ensure, traits::Get};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use sp_runtime::{
	traits::{AccountIdConversion, CheckedAdd, CheckedSub, EnsureOrigin, Saturating, Zero},
	DispatchResult, ModuleId,
};
use support::{AuctionManager, CDPTreasury, CDPTreasuryExtended, EmergencyShutdown, Ratio};
use system::ensure_root;

mod mock;
mod tests;

const MODULE_ID: ModuleId = ModuleId(*b"aca/trsy");

type BalanceOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type CurrencyIdOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;

pub trait Trait: system::Trait {
	type Currency: MultiCurrencyExtended<Self::AccountId>;
	type GetStableCurrencyId: Get<CurrencyIdOf<Self>>;
	type AuctionManagerHandler: AuctionManager<Self::AccountId, Balance = BalanceOf<Self>>;
	type UpdateOrigin: EnsureOrigin<Self::Origin>;
}

decl_storage! {
	trait Store for Module<T: Trait> as CDPTreasury {
		pub DebitPool get(fn debit_pool): BalanceOf<T>;
		pub SurplusPool get(fn surplus_pool): BalanceOf<T>;
		pub SurplusAuctionFixedSize get(fn surplus_auction_fixed_size): BalanceOf<T>;
		pub SurplusBufferSize get(fn surplus_buffer_size): BalanceOf<T>;
		pub InitialAmountPerDebitAuction get(fn initial_amount_per_debit_auction): BalanceOf<T>;
		pub DebitAuctionFixedSize get(fn debit_auction_fixed_size): BalanceOf<T>;
		pub IsShutdown get(fn is_shutdown): bool;
		pub TotalCollaterals get(fn total_collaterals): map CurrencyIdOf<T> => BalanceOf<T>;
	}
}

decl_error! {
	/// Error for cdp treasury module.
	pub enum Error for Module<T: Trait> {
		CollateralNotEnough,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn set_debit_and_surplus_handle_params(
			origin,
			surplus_auction_fixed_size: Option<BalanceOf<T>>,
			surplus_buffer_size: Option<BalanceOf<T>>,
			initial_amount_per_debit_auction: Option<BalanceOf<T>>,
			debit_auction_fixed_size: Option<BalanceOf<T>>,
		) {
			T::UpdateOrigin::try_origin(origin)
				.map(|_| ())
				.or_else(ensure_root)?;
			if let Some(amount) = surplus_auction_fixed_size {
				<SurplusAuctionFixedSize<T>>::put(amount);
			}
			if let Some(amount) = surplus_buffer_size {
				<SurplusBufferSize<T>>::put(amount);
			}
			if let Some(amount) = initial_amount_per_debit_auction {
				<InitialAmountPerDebitAuction<T>>::put(amount);
			}
			if let Some(amount) = debit_auction_fixed_size {
				<DebitAuctionFixedSize<T>>::put(amount);
			}
		}

		fn on_finalize(_now: T::BlockNumber) {
			// offset the same amount between debit pool and surplus
			let offset_amount = rstd::cmp::min(Self::debit_pool(), Self::surplus_pool());
			let stable_currency_id = T::GetStableCurrencyId::get();
			if !offset_amount.is_zero() {
				if T::Currency::ensure_can_withdraw(stable_currency_id, &Self::account_id(), offset_amount).is_ok() {
					T::Currency::withdraw(stable_currency_id, &Self::account_id(), offset_amount)
					.expect("never fail after balance check");
				}
				<DebitPool<T>>::mutate(|debit| *debit -= offset_amount);
				<SurplusPool<T>>::mutate(|surplus| *surplus -= offset_amount);
			}

			if !Self::is_shutdown() {
				// create surplus auction requires:
				// 1. debit_pool == 0
				// 2. surplus_pool > surplus_buffer_size + surplus_auction_fixed_size
				let mut remain_surplus_pool = Self::surplus_pool();
				let surplus_buffer_size = Self::surplus_buffer_size();
				let surplus_auction_fixed_size = Self::surplus_auction_fixed_size();
				while remain_surplus_pool >= surplus_buffer_size + surplus_auction_fixed_size
				&& !surplus_auction_fixed_size.is_zero() {
					if T::Currency::ensure_can_withdraw(stable_currency_id, &Self::account_id(), surplus_auction_fixed_size).is_ok() {
						T::Currency::withdraw(stable_currency_id, &Self::account_id(), surplus_auction_fixed_size)
						.expect("never fail after balance check");
						T::AuctionManagerHandler::new_surplus_auction(surplus_auction_fixed_size);
					}
					<SurplusPool<T>>::mutate(|surplus| *surplus -= surplus_auction_fixed_size);
					remain_surplus_pool -= surplus_auction_fixed_size;
				}

				// create debit auction requires:
				// 1. surplus_pool == 0
				// 2. debit_pool >= total_debit_in_auction + get_total_target_in_auction + debit_auction_fixed_size
				let mut remain_debit_pool = Self::debit_pool();
				let debit_auction_fixed_size = Self::debit_auction_fixed_size();
				let initial_amount_per_debit_auction = Self::initial_amount_per_debit_auction();
				let total_debit_in_auction = T::AuctionManagerHandler::get_total_debit_in_auction();
				let total_target_in_auction = T::AuctionManagerHandler::get_total_target_in_auction();
				while remain_debit_pool >= total_debit_in_auction + total_target_in_auction + debit_auction_fixed_size
				&& !initial_amount_per_debit_auction.is_zero()
				&& !debit_auction_fixed_size.is_zero() {
					T::AuctionManagerHandler::new_debit_auction(initial_amount_per_debit_auction, debit_auction_fixed_size);
					<DebitPool<T>>::mutate(|debit| *debit -= debit_auction_fixed_size);
					remain_debit_pool -= debit_auction_fixed_size;
				}
			}
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		MODULE_ID.into_account()
	}

	pub fn emergency_shutdown() {
		<IsShutdown>::put(true);
	}
}

impl<T: Trait> CDPTreasury<T::AccountId> for Module<T> {
	type Balance = BalanceOf<T>;
	type CurrencyId = CurrencyIdOf<T>;

	fn on_system_debit(amount: Self::Balance) {
		<DebitPool<T>>::mutate(|debit| *debit = debit.saturating_add(amount));
	}

	fn on_system_surplus(amount: Self::Balance) {
		if T::Currency::balance(T::GetStableCurrencyId::get(), &Self::account_id())
			.checked_add(&amount)
			.is_some()
		{
			T::Currency::deposit(T::GetStableCurrencyId::get(), &Self::account_id(), amount)
				.expect("never failed after overflow check");
			<SurplusPool<T>>::mutate(|surplus| *surplus += amount);
		}
	}

	fn deposit_backed_debit(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		T::Currency::deposit(T::GetStableCurrencyId::get(), who, amount)
	}

	fn withdraw_backed_debit(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		T::Currency::withdraw(T::GetStableCurrencyId::get(), who, amount)
	}

	fn deposit_system_collateral(currency_id: Self::CurrencyId, amount: Self::Balance) -> DispatchResult {
		T::Currency::deposit(currency_id, &Self::account_id(), amount)?;
		<TotalCollaterals<T>>::mutate(currency_id, |balance| *balance += amount);
		Ok(())
	}

	fn transfer_system_collateral(
		currency_id: Self::CurrencyId,
		to: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		ensure!(
			Self::total_collaterals(currency_id).checked_sub(&amount).is_some(),
			Error::<T>::CollateralNotEnough,
		);
		T::Currency::transfer(currency_id, &Self::account_id(), to, amount)?;
		<TotalCollaterals<T>>::mutate(currency_id, |balance| *balance -= amount);
		Ok(())
	}
}

impl<T: Trait> CDPTreasuryExtended<T::AccountId> for Module<T> {
	fn get_total_collaterals(id: Self::CurrencyId) -> Self::Balance {
		Self::total_collaterals(id)
	}

	fn get_surplus_pool() -> Self::Balance {
		Self::surplus_pool()
	}

	fn get_stable_currency_ratio(amount: Self::Balance) -> Ratio {
		let stable_total_supply = T::Currency::total_issuance(T::GetStableCurrencyId::get());
		Ratio::from_rational(amount, stable_total_supply)
	}
}

impl<T: Trait> EmergencyShutdown for Module<T> {
	fn on_emergency_shutdown() {
		Self::emergency_shutdown();
	}
}
