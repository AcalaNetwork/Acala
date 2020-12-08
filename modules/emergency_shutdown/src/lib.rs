//! # Emergency Shutdown Module
//!
//! ## Overview
//!
//! When a black swan occurs such as price plunge or fatal bug, the highest
//! priority is to minimize user losses as much as possible. When the decision
//! to shutdown system is made, emergency shutdown module needs to trigger all
//! related module to halt, and start a series of operations including close
//! some user entry, freeze feed prices, run offchain worker to settle
//! CDPs has debit, cancel all active auctions module, when debits and gaps are
//! settled, the stable currency holder are allowed to refund a basket of
//! remaining collateral assets.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::{EnsureOrigin, Get},
	transactional,
	weights::{DispatchClass, Weight},
};
use frame_system::{self as system, ensure_signed};
use primitives::{Balance, CurrencyId};
use sp_runtime::{traits::Zero, FixedPointNumber};
use sp_std::prelude::*;
use support::{AuctionManager, CDPTreasury, EmergencyShutdown, PriceProvider, Ratio};

mod default_weight;
mod mock;
mod tests;

pub trait WeightInfo {
	fn emergency_shutdown(c: u32) -> Weight;
	fn open_collateral_refund() -> Weight;
	fn refund_collaterals(c: u32) -> Weight;
}

pub trait Config: system::Config + loans::Config {
	type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;

	/// The list of valid collateral currency types
	type CollateralCurrencyIds: Get<Vec<CurrencyId>>;

	/// Price source to freeze currencies' price
	type PriceSource: PriceProvider<CurrencyId>;

	/// CDP treasury to escrow collateral assets after settlement
	type CDPTreasury: CDPTreasury<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;

	/// Check the auction cancellation to decide whether to open the final
	/// redemption
	type AuctionManagerHandler: AuctionManager<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;

	/// The origin which may trigger emergency shutdown. Root can always do
	/// this.
	type ShutdownOrigin: EnsureOrigin<Self::Origin>;

	/// Weight information for the extrinsics in this module.
	type WeightInfo: WeightInfo;
}

decl_event!(
	pub enum Event<T> where
		<T as system::Config>::AccountId,
		<T as system::Config>::BlockNumber,
		Balance = Balance,
		CurrencyId = CurrencyId,
	{
		/// Emergency shutdown occurs. \[block_number\]
		Shutdown(BlockNumber),
		/// The final redemption opened. \[block_number\]
		OpenRefund(BlockNumber),
		/// Refund info. \[caller, stable_coin_amount, refund_list\]
		Refund(AccountId, Balance, Vec<(CurrencyId, Balance)>),
	}
);

decl_error! {
	/// Error for emergency shutdown module.
	pub enum Error for Module<T: Config> {
		/// System has already been shutdown
		AlreadyShutdown,
		/// Must after system shutdown
		MustAfterShutdown,
		/// Final redemption is still not opened
		CanNotRefund,
		/// Exist potential surplus, means settlement has not been completed
		ExistPotentialSurplus,
		/// Exist unhandled debit, means settlement has not been completed
		ExistUnhandledDebit,
	}
}

decl_storage! {
	trait Store for Module<T: Config> as EmergencyShutdown {
		/// Emergency shutdown flag
		pub IsShutdown get(fn is_shutdown): bool;
		/// Open final redemption flag
		pub CanRefund get(fn can_refund): bool;
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;
		fn deposit_event() = default;

		/// The list of valid collateral currency types
		const CollateralCurrencyIds: Vec<CurrencyId> = T::CollateralCurrencyIds::get();

		/// Start emergency shutdown
		///
		/// The dispatch origin of this call must be `ShutdownOrigin`.
		///
		/// # <weight>
		/// - Preconditions:
		/// 	- T::CDPTreasury is module_cdp_treasury
		/// 	- T::AuctionManagerHandler is module_auction_manager
		/// 	- T::OnShutdown is (module_cdp_treasury, module_cdp_engine, module_honzon, module_dex)
		/// - Complexity: `O(1)`
		/// - Db reads: `IsShutdown`, (length of collateral_ids) items in modules related to module_emergency_shutdown
		/// - Db writes: `IsShutdown`, (4 + length of collateral_ids) items in modules related to module_emergency_shutdown
		/// -------------------
		/// Base Weight: 148.3 µs
		/// # </weight>
		#[weight = (T::WeightInfo::emergency_shutdown(T::CollateralCurrencyIds::get().len() as u32), DispatchClass::Operational)]
		#[transactional]
		pub fn emergency_shutdown(origin) {
			T::ShutdownOrigin::ensure_origin(origin)?;
			ensure!(!Self::is_shutdown(), Error::<T>::AlreadyShutdown);

			// get all collateral types
			let collateral_currency_ids = T::CollateralCurrencyIds::get();

			// lock price for every collateral
			for currency_id in collateral_currency_ids {
				<T as Config>::PriceSource::lock_price(currency_id);
			}

			<IsShutdown>::put(true);
			Self::deposit_event(RawEvent::Shutdown(<system::Module<T>>::block_number()));
		}

		/// Open final redemption if settlement is completed.
		///
		/// The dispatch origin of this call must be `ShutdownOrigin`.
		///
		/// # <weight>
		/// - Preconditions:
		/// 	- T::CDPTreasury is module_cdp_treasury
		/// 	- T::AuctionManagerHandler is module_auction_manager
		/// 	- T::OnShutdown is (module_cdp_treasury, module_cdp_engine, module_honzon, module_dex)
		/// - Complexity: `O(1)`
		/// - Db reads: `IsShutdown`, (2 + 2 * length of collateral_ids) items in modules related to module_emergency_shutdown
		/// - Db writes: `CanRefund`
		/// -------------------
		/// Base Weight: 71.8 µs
		/// # </weight>
		#[weight = (T::WeightInfo::open_collateral_refund(), DispatchClass::Operational)]
		#[transactional]
		pub fn open_collateral_refund(origin) {
			T::ShutdownOrigin::ensure_origin(origin)?;
			ensure!(Self::is_shutdown(), Error::<T>::MustAfterShutdown);	// must after shutdown

			// Ensure there's no debit and surplus auction now, they may bring uncertain surplus to system.
			// Cancel all surplus auctions and debit auctions to pass the check!
			ensure!(
				<T as Config>::AuctionManagerHandler::get_total_debit_in_auction().is_zero()
				&& <T as Config>::AuctionManagerHandler::get_total_surplus_in_auction().is_zero(),
				Error::<T>::ExistPotentialSurplus,
			);

			// Ensure all debits of CDPs have been settled, and all collateral auction has been done or canceled.
			// Settle all collaterals type CDPs which have debit, cancel all collateral auctions in forward stage and
			// wait for all collateral auctions in reverse stage to be ended.
			let collateral_currency_ids = T::CollateralCurrencyIds::get();
			for currency_id in collateral_currency_ids {
				// there's no collateral auction
				ensure!(
					<T as Config>::AuctionManagerHandler::get_total_collateral_in_auction(currency_id).is_zero(),
					Error::<T>::ExistPotentialSurplus,
				);
				// there's on debit in CDP
				ensure!(
					<loans::Module<T>>::total_positions(currency_id).debit.is_zero(),
					Error::<T>::ExistUnhandledDebit,
				);
			}

			// Open refund stage
			<CanRefund>::put(true);
			Self::deposit_event(RawEvent::OpenRefund(<system::Module<T>>::block_number()));
		}

		/// Refund a basket of remaining collateral assets to caller
		///
		/// - `amount`: stable currency amount used to refund.
		///
		/// # <weight>
		/// - Preconditions:
		/// 	- T::CDPTreasury is module_cdp_treasury
		/// 	- T::AuctionManagerHandler is module_auction_manager
		/// 	- T::OnShutdown is (module_cdp_treasury, module_cdp_engine, module_honzon, module_dex)
		/// - Complexity: `O(1)`
		/// - Db reads: `CanRefund`, (2 + 3 * length of collateral_ids) items in modules related to module_emergency_shutdown
		/// - Db writes: (3 * length of collateral_ids) items in modules related to module_emergency_shutdown
		/// -------------------
		/// Base Weight: 455.1 µs
		/// # </weight>
		#[weight = T::WeightInfo::refund_collaterals(T::CollateralCurrencyIds::get().len() as u32)]
		#[transactional]
		pub fn refund_collaterals(origin, #[compact] amount: Balance) {
			let who = ensure_signed(origin)?;
			ensure!(Self::can_refund(), Error::<T>::CanNotRefund);

			let refund_ratio: Ratio = <T as Config>::CDPTreasury::get_debit_proportion(amount);
			let collateral_currency_ids = T::CollateralCurrencyIds::get();

			// burn caller's stable currency by CDP treasury
			<T as Config>::CDPTreasury::burn_debit(&who, amount)?;

			let mut refund_assets: Vec<(CurrencyId, Balance)> = vec![];
			// refund collaterals to caller by CDP treasury
			for currency_id in collateral_currency_ids {
				let refund_amount = refund_ratio
					.saturating_mul_int(<T as Config>::CDPTreasury::get_total_collaterals(currency_id));

				if !refund_amount.is_zero() {
					<T as Config>::CDPTreasury::withdraw_collateral(&who, currency_id, refund_amount)?;
					refund_assets.push((currency_id, refund_amount));
				}
			}

			Self::deposit_event(RawEvent::Refund(who, amount, refund_assets));
		}
	}
}

impl<T: Config> EmergencyShutdown for Module<T> {
	fn is_shutdown() -> bool {
		Self::is_shutdown()
	}
}
