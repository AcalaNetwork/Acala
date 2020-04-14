#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get};
use orml_traits::MultiCurrency;
use rstd::prelude::*;
use sp_runtime::traits::{EnsureOrigin, Zero};
use support::{AuctionManager, CDPTreasury, CDPTreasuryExtended, OnEmergencyShutdown, PriceProvider, Ratio};
use system::{ensure_root, ensure_signed};

mod mock;
mod tests;

type CurrencyIdOf<T> = <<T as loans::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;
type BalanceOf<T> = <<T as loans::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type AuctionIdOf<T> =
	<<T as Trait>::AuctionManagerHandler as AuctionManager<<T as system::Trait>::AccountId>>::AuctionId;

pub trait Trait: system::Trait + loans::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type CollateralCurrencyIds: Get<Vec<CurrencyIdOf<Self>>>;
	type PriceSource: PriceProvider<CurrencyIdOf<Self>>;
	type CDPTreasury: CDPTreasuryExtended<Self::AccountId, Balance = BalanceOf<Self>, CurrencyId = CurrencyIdOf<Self>>;
	type AuctionManagerHandler: AuctionManager<
		Self::AccountId,
		Balance = BalanceOf<Self>,
		CurrencyId = CurrencyIdOf<Self>,
	>;
	type OnShutdown: OnEmergencyShutdown;
	type ShutdownOrigin: EnsureOrigin<Self::Origin>;
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::BlockNumber,
		Balance = BalanceOf<T>,
	{
		Shutdown(BlockNumber),
		OpenRefund(BlockNumber),
		Refund(Balance),
	}
);

decl_error! {
	pub enum Error for Module<T: Trait> {
		AlreadyShutdown,
		MustAfterShutdown,
		CanNotRefund,
		ExistPotentialSurplus,
		ExistUnhandleDebit,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as EmergencyShutdown {
		pub IsShutdown get(fn is_shutdown): bool;
		pub CanRefund get(fn can_refund): bool;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;
		fn deposit_event() = default;

		const CollateralCurrencyIds: Vec<CurrencyIdOf<T>> = T::CollateralCurrencyIds::get();

		pub fn emergency_shutdown(origin) {
			T::ShutdownOrigin::try_origin(origin)
				.map(|_| ())
				.or_else(ensure_root)?;
			ensure!(!Self::is_shutdown(), Error::<T>::AlreadyShutdown);

			// trigger shutdown in other related modules
			T::OnShutdown::on_emergency_shutdown();

			// get all collateral types
			let collateral_currency_ids = T::CollateralCurrencyIds::get();

			// lock price for every collateral
			for currency_id in collateral_currency_ids {
				<T as Trait>::PriceSource::lock_price(currency_id);
			}

			<IsShutdown>::put(true);
			Self::deposit_event(RawEvent::Shutdown(<system::Module<T>>::block_number()));
		}

		pub fn open_collateral_refund(origin) {
			T::ShutdownOrigin::try_origin(origin)
				.map(|_| ())
				.or_else(ensure_root)?;
			ensure!(Self::is_shutdown(), Error::<T>::MustAfterShutdown);	// must after shutdown

			// Ensure there's no debit and surplus auction now, these maybe bring uncertain surplus to system.
			// Cancel all surplus auctions and debit auctions to pass the check!
			ensure!(
				<T as Trait>::AuctionManagerHandler::get_total_debit_in_auction().is_zero()
				&& <T as Trait>::AuctionManagerHandler::get_total_surplus_in_auction().is_zero(),
				Error::<T>::ExistPotentialSurplus,
			);

			// Ensure all debits of CDPs have been settled, and all collateral auction has been done or canceled.
			// Settle all collaterals type CDPs which have debit, cancel all collateral auctions in forward stage and
			// wait for all collateral auctions in reverse stage to be ended.
			let collateral_currency_ids = T::CollateralCurrencyIds::get();
			for currency_id in collateral_currency_ids {
				// these's no collateral auction
				ensure!(
					<T as Trait>::AuctionManagerHandler::get_total_collateral_in_auction(currency_id).is_zero(),
					Error::<T>::ExistPotentialSurplus,
				);
				// there's on debit in cdp
				ensure!(
					<loans::Module<T>>::total_debits(currency_id).is_zero(),
					Error::<T>::ExistUnhandleDebit,
				);
			}

			// Open refund stage
			<CanRefund>::put(true);
			Self::deposit_event(RawEvent::OpenRefund(<system::Module<T>>::block_number()));
		}

		pub fn refund_collaterals(origin, #[compact] amount: BalanceOf<T>) {
			let who = ensure_signed(origin)?;
			ensure!(Self::can_refund(), Error::<T>::CanNotRefund);

			let refund_ratio: Ratio = <T as Trait>::CDPTreasury::get_stable_currency_ratio(amount);
			let collateral_currency_ids = T::CollateralCurrencyIds::get();

			// burn caller's stable currency by cdp treasury
			<T as Trait>::CDPTreasury::withdraw_backed_debit(&who, amount)?;

			// refund collaterals to caller by cdp treasury
			for currency_id in collateral_currency_ids {
				let refund_amount = refund_ratio
					.saturating_mul_int(&<T as Trait>::CDPTreasury::get_total_collaterals(currency_id));

				if !refund_amount.is_zero() {
					<T as Trait>::CDPTreasury::transfer_system_collateral(currency_id, &who, refund_amount)?;
				}
			}

			Self::deposit_event(RawEvent::Refund(amount));
		}

		pub fn cancel_auction(origin, id: AuctionIdOf<T>) {
			let _ = ensure_signed(origin)?;
			ensure!(Self::is_shutdown(), Error::<T>::MustAfterShutdown);
			<T as Trait>::AuctionManagerHandler::cancel_auction(id)?;
		}
	}
}
