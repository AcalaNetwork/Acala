#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get};
use orml_traits::MultiCurrency;
use sp_runtime::traits::{EnsureOrigin, Zero};
use support::{
	AuctionManager, AuctionManagerExtended, CDPTreasury, CDPTreasuryExtended, EmergencyShutdown, Price, PriceProvider,
	Ratio,
};
use system::{ensure_root, ensure_signed};

mod mock;
mod tests;

type CurrencyIdOf<T> = <<T as loans::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;
type BalanceOf<T> = <<T as loans::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type AuctionIdOf<T> =
	<<T as Trait>::AuctionManagerHandler as AuctionManagerExtended<<T as system::Trait>::AccountId>>::AuctionId;

pub trait Trait: system::Trait + honzon::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type PriceSource: PriceProvider<CurrencyIdOf<Self>, Price>;
	type Treasury: CDPTreasuryExtended<Self::AccountId, Balance = BalanceOf<Self>, CurrencyId = CurrencyIdOf<Self>>;
	type AuctionManagerHandler: AuctionManagerExtended<
		Self::AccountId,
		Balance = BalanceOf<Self>,
		CurrencyId = CurrencyIdOf<Self>,
	>;
	type OnShutdown: EmergencyShutdown;
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
		ExistSurplus,
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

		pub fn emergency_shutdown(origin) {
			T::ShutdownOrigin::try_origin(origin)
				.map(|_| ())
				.or_else(ensure_root)?;
			ensure!(!Self::is_shutdown(), Error::<T>::AlreadyShutdown);

			// trigger shutdown in other related modules
			T::OnShutdown::on_emergency_shutdown();

			// get all collateral types
			let collateral_currency_ids = <T as cdp_engine::Trait>::CollateralCurrencyIds::get();

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
			ensure!(<T as Trait>::Treasury::get_surplus_pool().is_zero(), Error::<T>::ExistSurplus);	// these's no surplus in cdp treasury
			ensure!(
				<T as Trait>::AuctionManagerHandler::get_total_debit_in_auction().is_zero()
				&& <T as Trait>::AuctionManagerHandler::get_total_surplus_in_auction().is_zero(),
				Error::<T>::ExistPotentialSurplus,
			);	// there's no debit and surplus auction now

			let collateral_currency_ids = <T as cdp_engine::Trait>::CollateralCurrencyIds::get();
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

			<CanRefund>::put(true);
			Self::deposit_event(RawEvent::OpenRefund(<system::Module<T>>::block_number()));
		}

		pub fn refund_collaterals(origin, amount: BalanceOf<T>) {
			let who = ensure_signed(origin)?;
			ensure!(Self::can_refund(), Error::<T>::CanNotRefund);

			let refund_ratio: Ratio = <T as Trait>::Treasury::get_stable_currency_ratio(amount);
			let collateral_currency_ids = <T as cdp_engine::Trait>::CollateralCurrencyIds::get();

			// burn caller's stable currency by cdp treasury
			<T as Trait>::Treasury::withdraw_backed_debit(&who, amount)?;

			// refund collaterals to caller by cdp treasury
			for currency_id in collateral_currency_ids {
				let refund_amount = refund_ratio
					.saturating_mul_int(&<T as Trait>::Treasury::get_total_collaterals(currency_id));

				if !refund_amount.is_zero() {
					<T as Trait>::Treasury::transfer_system_collateral(currency_id, &who, refund_amount)?;
				}
			}

			Self::deposit_event(RawEvent::Refund(amount));
		}

		pub fn cancel_auction(_origin, id: AuctionIdOf<T>) {
			ensure!(Self::is_shutdown(), Error::<T>::MustAfterShutdown);
			<T as Trait>::AuctionManagerHandler::cancel_auction(id)?;
		}
	}
}
