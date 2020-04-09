#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use sp_runtime::{
	traits::{AccountIdConversion, CheckedAdd, CheckedSub, EnsureOrigin, Saturating, Zero},
	DispatchResult, ModuleId,
};
use support::{AuctionManager, CDPTreasury, CDPTreasuryExtended, DEXManager, OnEmergencyShutdown, Ratio};
use system::ensure_root;

mod mock;
mod tests;

const MODULE_ID: ModuleId = ModuleId(*b"aca/cdpt");

type BalanceOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type CurrencyIdOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type Currency: MultiCurrencyExtended<Self::AccountId>;
	type GetStableCurrencyId: Get<CurrencyIdOf<Self>>;
	type AuctionManagerHandler: AuctionManager<
		Self::AccountId,
		CurrencyId = CurrencyIdOf<Self>,
		Balance = BalanceOf<Self>,
	>;
	type UpdateOrigin: EnsureOrigin<Self::Origin>;
	type DEX: DEXManager<Self::AccountId, CurrencyIdOf<Self>, BalanceOf<Self>>;
}

decl_event!(
	pub enum Event<T>
	where
		CurrencyId = CurrencyIdOf<T>,
		Balance = BalanceOf<T>,
	{
		UpdateSurplusAuctionFixedSize(Balance),
		UpdateSurplusBufferSize(Balance),
		UpdateInitialAmountPerDebitAuction(Balance),
		UpdateDebitAuctionFixedSize(Balance),
		UpdateCollateralAuctionMaximumSize(CurrencyId, Balance),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as CDPTreasury {
		pub SurplusAuctionFixedSize get(fn surplus_auction_fixed_size) config(): BalanceOf<T>;
		pub SurplusBufferSize get(fn surplus_buffer_size) config(): BalanceOf<T>;
		pub InitialAmountPerDebitAuction get(fn initial_amount_per_debit_auction) config(): BalanceOf<T>;
		pub DebitAuctionFixedSize get(fn debit_auction_fixed_size) config(): BalanceOf<T>;
		pub CollateralAuctionMaximumSize get(fn collateral_auction_maximum_size): map hasher(twox_64_concat) CurrencyIdOf<T> => BalanceOf<T>;

		pub DebitPool get(fn debit_pool): BalanceOf<T>;
		pub SurplusPool get(fn surplus_pool): BalanceOf<T>;
		pub TotalCollaterals get(fn total_collaterals): map hasher(twox_64_concat) CurrencyIdOf<T> => BalanceOf<T>;
		pub IsShutdown get(fn is_shutdown): bool;
	}

	add_extra_genesis {
		config(collateral_auction_maximum_size): Vec<(CurrencyIdOf<T>, BalanceOf<T>)>;

		build(|config: &GenesisConfig<T>| {
			config.collateral_auction_maximum_size.iter().for_each(|(currency_id, size)| {
				<CollateralAuctionMaximumSize<T>>::insert(currency_id, size);
			})
		})
	}
}

decl_error! {
	/// Error for cdp treasury module.
	pub enum Error for Module<T: Trait> {
		CollateralNotEnough,
		CollateralOverflow,
		SurplusPoolNotEnough,
		SurplusPoolOverflow,
		DebitPoolOverflow,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		// module constant
		const GetStableCurrencyId: CurrencyIdOf<T> = T::GetStableCurrencyId::get();

		pub fn set_debit_and_surplus_handle_params(
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
				Self::deposit_event(RawEvent::UpdateSurplusAuctionFixedSize(amount));
			}
			if let Some(amount) = surplus_buffer_size {
				<SurplusBufferSize<T>>::put(amount);
				Self::deposit_event(RawEvent::UpdateSurplusBufferSize(amount));
			}
			if let Some(amount) = initial_amount_per_debit_auction {
				<InitialAmountPerDebitAuction<T>>::put(amount);
				Self::deposit_event(RawEvent::UpdateInitialAmountPerDebitAuction(amount));
			}
			if let Some(amount) = debit_auction_fixed_size {
				<DebitAuctionFixedSize<T>>::put(amount);
				Self::deposit_event(RawEvent::UpdateDebitAuctionFixedSize(amount));
			}
		}

		pub fn set_collateral_auction_maximum_size(origin, currency_id: CurrencyIdOf<T>, size: BalanceOf<T>) {
			T::UpdateOrigin::try_origin(origin)
				.map(|_| ())
				.or_else(ensure_root)?;
			<CollateralAuctionMaximumSize<T>>::insert(currency_id, size);
			Self::deposit_event(RawEvent::UpdateCollateralAuctionMaximumSize(currency_id, size));
		}

		fn on_finalize(_now: T::BlockNumber) {
			// offset the same amount between debit pool and surplus pool
			Self::offset_unlocked_surplus_and_debit();

			// Stop to create surplus auction and debit auction after emergency shutdown happend.
			if !Self::is_shutdown() {
				let surplus_auction_fixed_size = Self::surplus_auction_fixed_size();
				if !surplus_auction_fixed_size.is_zero() {
					let mut remain_surplus_pool = Self::get_unlocked_surplus();
					let surplus_buffer_size = Self::surplus_buffer_size();

					// create surplus auction requires:
					// surplus_pool > surplus_buffer_size + surplus_auction_fixed_size
					while remain_surplus_pool >= surplus_buffer_size + surplus_auction_fixed_size {
						T::AuctionManagerHandler::new_surplus_auction(surplus_auction_fixed_size);
						<SurplusPool<T>>::mutate(|surplus| *surplus -= surplus_auction_fixed_size);
						remain_surplus_pool -= surplus_auction_fixed_size;
					}
				}

				let debit_auction_fixed_size = Self::debit_auction_fixed_size();
				let initial_amount_per_debit_auction = Self::initial_amount_per_debit_auction();
				if !debit_auction_fixed_size.is_zero() && !initial_amount_per_debit_auction.is_zero() {
					let mut remain_debit_pool = Self::debit_pool();
					let total_debit_in_auction = T::AuctionManagerHandler::get_total_debit_in_auction();
					let total_target_in_auction = T::AuctionManagerHandler::get_total_target_in_auction();

					// create debit auction requires:
					// surplus_pool > surplus_buffer_size + surplus_auction_fixed_size
					while remain_debit_pool >= total_debit_in_auction + total_target_in_auction + debit_auction_fixed_size {
						T::AuctionManagerHandler::new_debit_auction(initial_amount_per_debit_auction, debit_auction_fixed_size);
						<DebitPool<T>>::mutate(|debit| *debit -= debit_auction_fixed_size);
						remain_debit_pool -= debit_auction_fixed_size;
					}
				}
			}
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		MODULE_ID.into_account()
	}

	pub fn get_unlocked_surplus() -> BalanceOf<T> {
		Self::surplus_pool().saturating_sub(T::AuctionManagerHandler::get_total_surplus_in_auction())
	}

	pub fn offset_unlocked_surplus_and_debit() {
		let offset_amount = rstd::cmp::min(Self::debit_pool(), Self::get_unlocked_surplus());
		if !offset_amount.is_zero()
			&& T::Currency::withdraw(T::GetStableCurrencyId::get(), &Self::account_id(), offset_amount).is_ok()
		{
			<DebitPool<T>>::mutate(|debit| *debit -= offset_amount);
			<SurplusPool<T>>::mutate(|surplus| *surplus -= offset_amount);
		}
	}

	pub fn emergency_shutdown() {
		<IsShutdown>::put(true);
	}
}

impl<T: Trait> CDPTreasury<T::AccountId> for Module<T> {
	type Balance = BalanceOf<T>;
	type CurrencyId = CurrencyIdOf<T>;

	fn get_surplus_pool() -> Self::Balance {
		Self::surplus_pool()
	}

	fn get_debit_pool() -> Self::Balance {
		Self::debit_pool()
	}

	fn get_total_collaterals(id: Self::CurrencyId) -> Self::Balance {
		Self::total_collaterals(id)
	}

	fn on_system_debit(amount: Self::Balance) -> DispatchResult {
		let new_debit_pool = Self::debit_pool()
			.checked_add(&amount)
			.ok_or(Error::<T>::DebitPoolOverflow)?;
		<DebitPool<T>>::put(new_debit_pool);
		Ok(())
	}

	fn on_system_surplus(amount: Self::Balance) -> DispatchResult {
		let new_surplus_pool = Self::surplus_pool()
			.checked_add(&amount)
			.ok_or(Error::<T>::SurplusPoolOverflow)?;
		T::Currency::deposit(T::GetStableCurrencyId::get(), &Self::account_id(), amount)?;
		<SurplusPool<T>>::put(new_surplus_pool);
		Ok(())
	}

	fn deposit_backed_debit(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		T::Currency::deposit(T::GetStableCurrencyId::get(), who, amount)
	}

	fn withdraw_backed_debit(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		T::Currency::withdraw(T::GetStableCurrencyId::get(), who, amount)
	}

	fn transfer_system_surplus(to: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		let new_surplus_pool = Self::surplus_pool()
			.checked_sub(&amount)
			.ok_or(Error::<T>::SurplusPoolNotEnough)?;
		T::Currency::transfer(T::GetStableCurrencyId::get(), &Self::account_id(), to, amount)?;
		<SurplusPool<T>>::put(new_surplus_pool);
		Ok(())
	}

	fn transfer_surplus_from(from: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		let new_surplus_pool = Self::surplus_pool()
			.checked_add(&amount)
			.ok_or(Error::<T>::SurplusPoolOverflow)?;
		T::Currency::transfer(T::GetStableCurrencyId::get(), from, &Self::account_id(), amount)?;
		<SurplusPool<T>>::put(new_surplus_pool);
		Ok(())
	}

	fn transfer_system_collateral(
		currency_id: Self::CurrencyId,
		to: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		let new_total_collateral = Self::total_collaterals(currency_id)
			.checked_sub(&amount)
			.ok_or(Error::<T>::CollateralNotEnough)?;
		T::Currency::ensure_can_withdraw(currency_id, &Self::account_id(), amount)?;
		T::Currency::transfer(currency_id, &Self::account_id(), to, amount).expect("never failed after check");
		<TotalCollaterals<T>>::insert(currency_id, new_total_collateral);
		Ok(())
	}

	fn transfer_collateral_from(
		currency_id: Self::CurrencyId,
		from: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		let new_total_collateral = Self::total_collaterals(currency_id)
			.checked_add(&amount)
			.ok_or(Error::<T>::CollateralOverflow)?;
		T::Currency::ensure_can_withdraw(currency_id, &from, amount)?;
		T::Currency::transfer(currency_id, from, &Self::account_id(), amount).expect("never failed after check");
		<TotalCollaterals<T>>::insert(currency_id, new_total_collateral);
		Ok(())
	}
}

impl<T: Trait> CDPTreasuryExtended<T::AccountId> for Module<T> {
	fn get_stable_currency_ratio(amount: Self::Balance) -> Ratio {
		let stable_total_supply = T::Currency::total_issuance(T::GetStableCurrencyId::get());
		Ratio::from_rational(amount, stable_total_supply)
	}

	fn swap_collateral_to_stable(
		currency_id: CurrencyIdOf<T>,
		supply_amount: BalanceOf<T>,
		target_amount: BalanceOf<T>,
	) -> DispatchResult {
		ensure!(
			Self::total_collaterals(currency_id) >= supply_amount,
			Error::<T>::CollateralNotEnough,
		);
		T::Currency::ensure_can_withdraw(currency_id, &Self::account_id(), supply_amount)?;

		let amount = T::DEX::exchange_currency(
			Self::account_id(),
			currency_id,
			supply_amount,
			T::GetStableCurrencyId::get(),
			target_amount,
		)?;

		<TotalCollaterals<T>>::mutate(currency_id, |balance| *balance -= supply_amount);
		<SurplusPool<T>>::mutate(|surplus| *surplus += amount);

		Ok(())
	}

	fn create_collateral_auctions(
		currency_id: CurrencyIdOf<T>,
		amount: BalanceOf<T>,
		target: BalanceOf<T>,
		refund_receiver: T::AccountId,
	) {
		if Self::total_collaterals(currency_id)
			>= amount + T::AuctionManagerHandler::get_total_collateral_in_auction(currency_id)
		{
			let collateral_auction_maximum_size = Self::collateral_auction_maximum_size(currency_id);
			let mut unhandled_collateral_amount = amount;
			let mut unhandled_target = target;

			while !unhandled_collateral_amount.is_zero() {
				let (lot_collateral_amount, lot_target) = if unhandled_collateral_amount
					> collateral_auction_maximum_size
					&& !collateral_auction_maximum_size.is_zero()
				{
					let proportion = Ratio::from_rational(collateral_auction_maximum_size, amount);
					(collateral_auction_maximum_size, proportion.saturating_mul_int(&target))
				} else {
					(unhandled_collateral_amount, unhandled_target)
				};

				T::AuctionManagerHandler::new_collateral_auction(
					&refund_receiver,
					currency_id,
					lot_collateral_amount,
					lot_target,
				);

				unhandled_collateral_amount -= lot_collateral_amount;
				unhandled_target -= lot_target;
			}
		}
	}
}

impl<T: Trait> OnEmergencyShutdown for Module<T> {
	fn on_emergency_shutdown() {
		Self::emergency_shutdown();
	}
}
