#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::{EnsureOrigin, Get},
};
use frame_system::{self as system, ensure_root};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use primitives::{Balance, CurrencyId};
use sp_runtime::{
	traits::{AccountIdConversion, Zero},
	DispatchResult, ModuleId,
};
use support::{AuctionManager, CDPTreasury, CDPTreasuryExtended, DEXManager, OnEmergencyShutdown, Ratio};

mod mock;
mod tests;

const MODULE_ID: ModuleId = ModuleId(*b"aca/cdpt");

pub trait Trait: system::Trait {
	type Event: From<Event> + Into<<Self as system::Trait>::Event>;
	type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;
	type GetStableCurrencyId: Get<CurrencyId>;
	type AuctionManagerHandler: AuctionManager<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;
	type UpdateOrigin: EnsureOrigin<Self::Origin>;
	type DEX: DEXManager<Self::AccountId, CurrencyId, Balance>;
}

decl_event!(
	pub enum Event {
		UpdateSurplusAuctionFixedSize(Balance),
		UpdateSurplusBufferSize(Balance),
		UpdateInitialAmountPerDebitAuction(Balance),
		UpdateDebitAuctionFixedSize(Balance),
		UpdateCollateralAuctionMaximumSize(CurrencyId, Balance),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as CDPTreasury {
		pub SurplusAuctionFixedSize get(fn surplus_auction_fixed_size) config(): Balance;
		pub SurplusBufferSize get(fn surplus_buffer_size) config(): Balance;
		pub InitialAmountPerDebitAuction get(fn initial_amount_per_debit_auction) config(): Balance;
		pub DebitAuctionFixedSize get(fn debit_auction_fixed_size) config(): Balance;
		pub CollateralAuctionMaximumSize get(fn collateral_auction_maximum_size): map hasher(twox_64_concat) CurrencyId => Balance;

		pub DebitPool get(fn debit_pool): Balance;
		pub SurplusPool get(fn surplus_pool): Balance;
		pub TotalCollaterals get(fn total_collaterals): map hasher(twox_64_concat) CurrencyId => Balance;
		pub IsShutdown get(fn is_shutdown): bool;
	}

	add_extra_genesis {
		config(collateral_auction_maximum_size): Vec<(CurrencyId, Balance)>;

		build(|config: &GenesisConfig| {
			config.collateral_auction_maximum_size.iter().for_each(|(currency_id, size)| {
				CollateralAuctionMaximumSize::insert(currency_id, size);
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
		type Error = Error<T>;
		fn deposit_event() = default;

		// module constant
		const GetStableCurrencyId: CurrencyId = T::GetStableCurrencyId::get();

		#[weight = frame_support::weights::SimpleDispatchInfo::default()]
		pub fn set_debit_and_surplus_handle_params(
			origin,
			surplus_auction_fixed_size: Option<Balance>,
			surplus_buffer_size: Option<Balance>,
			initial_amount_per_debit_auction: Option<Balance>,
			debit_auction_fixed_size: Option<Balance>,
		) {
			T::UpdateOrigin::try_origin(origin)
				.map(|_| ())
				.or_else(ensure_root)?;
			if let Some(amount) = surplus_auction_fixed_size {
				SurplusAuctionFixedSize::put(amount);
				Self::deposit_event(Event::UpdateSurplusAuctionFixedSize(amount));
			}
			if let Some(amount) = surplus_buffer_size {
				SurplusBufferSize::put(amount);
				Self::deposit_event(Event::UpdateSurplusBufferSize(amount));
			}
			if let Some(amount) = initial_amount_per_debit_auction {
				InitialAmountPerDebitAuction::put(amount);
				Self::deposit_event(Event::UpdateInitialAmountPerDebitAuction(amount));
			}
			if let Some(amount) = debit_auction_fixed_size {
				DebitAuctionFixedSize::put(amount);
				Self::deposit_event(Event::UpdateDebitAuctionFixedSize(amount));
			}
		}

		#[weight = frame_support::weights::SimpleDispatchInfo::default()]
		pub fn set_collateral_auction_maximum_size(origin, currency_id: CurrencyId, size: Balance) {
			T::UpdateOrigin::try_origin(origin)
				.map(|_| ())
				.or_else(ensure_root)?;
			CollateralAuctionMaximumSize::insert(currency_id, size);
			Self::deposit_event(Event::UpdateCollateralAuctionMaximumSize(currency_id, size));
		}

		fn on_finalize(_now: T::BlockNumber) {
			// offset the same amount between debit pool and surplus pool
			Self::offset_surplus_and_debit();

			// Stop to create surplus auction and debit auction after emergency shutdown happend.
			if !Self::is_shutdown() {
				let surplus_auction_fixed_size = Self::surplus_auction_fixed_size();
				if !surplus_auction_fixed_size.is_zero() {
					let mut remain_surplus_pool = Self::surplus_pool();
					let surplus_buffer_size = Self::surplus_buffer_size();
					let total_surplus_in_auction = T::AuctionManagerHandler::get_total_surplus_in_auction();

					// create surplus auction requires:
					// surplus_pool >= total_surplus_in_auction + surplus_buffer_size + surplus_auction_fixed_size
					while remain_surplus_pool >= total_surplus_in_auction + surplus_buffer_size + surplus_auction_fixed_size {
						T::AuctionManagerHandler::new_surplus_auction(surplus_auction_fixed_size);
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
					// debit_pool > total_debit_in_auction + total_target_in_auction + debit_auction_fixed_size
					while remain_debit_pool >= total_debit_in_auction + total_target_in_auction + debit_auction_fixed_size {
						T::AuctionManagerHandler::new_debit_auction(initial_amount_per_debit_auction, debit_auction_fixed_size);
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

	pub fn offset_surplus_and_debit() {
		let offset_amount = sp_std::cmp::min(Self::debit_pool(), Self::surplus_pool());
		if !offset_amount.is_zero()
			&& T::Currency::withdraw(T::GetStableCurrencyId::get(), &Self::account_id(), offset_amount).is_ok()
		{
			DebitPool::mutate(|debit| *debit -= offset_amount);
			SurplusPool::mutate(|surplus| *surplus -= offset_amount);
		}
	}

	pub fn emergency_shutdown() {
		<IsShutdown>::put(true);
	}
}

impl<T: Trait> CDPTreasury<T::AccountId> for Module<T> {
	type Balance = Balance;
	type CurrencyId = CurrencyId;

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
			.checked_add(amount)
			.ok_or(Error::<T>::DebitPoolOverflow)?;
		DebitPool::put(new_debit_pool);
		Ok(())
	}

	fn on_system_surplus(amount: Self::Balance) -> DispatchResult {
		let new_surplus_pool = Self::surplus_pool()
			.checked_add(amount)
			.ok_or(Error::<T>::SurplusPoolOverflow)?;
		T::Currency::deposit(T::GetStableCurrencyId::get(), &Self::account_id(), amount)?;
		SurplusPool::put(new_surplus_pool);
		Ok(())
	}

	fn deposit_backed_debit_to(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		T::Currency::deposit(T::GetStableCurrencyId::get(), who, amount)
	}

	fn deposit_unbacked_debit_to(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		Self::on_system_debit(amount)?;
		T::Currency::deposit(T::GetStableCurrencyId::get(), who, amount)
	}

	fn withdraw_backed_debit_from(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		T::Currency::withdraw(T::GetStableCurrencyId::get(), who, amount)
	}

	fn transfer_surplus_from(from: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		let new_surplus_pool = Self::surplus_pool()
			.checked_add(amount)
			.ok_or(Error::<T>::SurplusPoolOverflow)?;
		T::Currency::transfer(T::GetStableCurrencyId::get(), from, &Self::account_id(), amount)?;
		SurplusPool::put(new_surplus_pool);
		Ok(())
	}

	fn transfer_collateral_to(
		currency_id: Self::CurrencyId,
		to: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		let new_total_collateral = Self::total_collaterals(currency_id)
			.checked_sub(amount)
			.ok_or(Error::<T>::CollateralNotEnough)?;
		T::Currency::ensure_can_withdraw(currency_id, &Self::account_id(), amount)?;
		T::Currency::transfer(currency_id, &Self::account_id(), to, amount).expect("never failed after check");
		TotalCollaterals::insert(currency_id, new_total_collateral);
		Ok(())
	}

	fn transfer_collateral_from(
		currency_id: Self::CurrencyId,
		from: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		let new_total_collateral = Self::total_collaterals(currency_id)
			.checked_add(amount)
			.ok_or(Error::<T>::CollateralOverflow)?;
		T::Currency::ensure_can_withdraw(currency_id, &from, amount)?;
		T::Currency::transfer(currency_id, from, &Self::account_id(), amount).expect("never failed after check");
		TotalCollaterals::insert(currency_id, new_total_collateral);
		Ok(())
	}

	fn get_debit_proportion(amount: Self::Balance) -> Ratio {
		let stable_total_supply = T::Currency::total_issuance(T::GetStableCurrencyId::get());
		Ratio::from_rational(amount, stable_total_supply)
	}
}

impl<T: Trait> CDPTreasuryExtended<T::AccountId> for Module<T> {
	fn swap_collateral_to_stable(
		currency_id: CurrencyId,
		supply_amount: Balance,
		target_amount: Balance,
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

		TotalCollaterals::mutate(currency_id, |balance| *balance -= supply_amount);
		SurplusPool::mutate(|surplus| *surplus += amount);

		Ok(())
	}

	fn create_collateral_auctions(
		currency_id: CurrencyId,
		amount: Balance,
		target: Balance,
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
