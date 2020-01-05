#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get};
use orml_traits::{Auction, AuctionHandler, MultiCurrency, OnNewBidResult};
use rstd::cmp::{Eq, PartialEq};
use sp_runtime::{
	traits::{AccountIdConversion, CheckedAdd, CheckedSub, Saturating, Zero},
	DispatchResult, ModuleId, RuntimeDebug,
};
use support::{AuctionManager, AuctionManagerExtended, CDPTreasury, Price, PriceProvider, Rate};

mod mock;
mod tests;

const MODULE_ID: ModuleId = ModuleId(*b"aca/amgr");

#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct CollateralAuctionItem<AccountId, CurrencyId, Balance, BlockNumber> {
	owner: AccountId,
	currency_id: CurrencyId,
	amount: Balance,
	target: Balance,
	start_time: BlockNumber,
}

#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct DebitAuctionItem<Balance, BlockNumber> {
	amount: Balance,
	fix: Balance,
	start_time: BlockNumber,
}

#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct SurplusAuctionItem<Balance, BlockNumber> {
	amount: Balance,
	start_time: BlockNumber,
}

type CurrencyIdOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;
type BalanceOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type AuctionIdOf<T> =
	<<T as Trait>::Auction as Auction<<T as system::Trait>::AccountId, <T as system::Trait>::BlockNumber>>::AuctionId;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type Currency: MultiCurrency<Self::AccountId>;
	type Auction: Auction<Self::AccountId, Self::BlockNumber, Balance = BalanceOf<Self>>;
	type MinimumIncrementSize: Get<Rate>;
	type AuctionTimeToClose: Get<Self::BlockNumber>;
	type AuctionDurationSoftCap: Get<Self::BlockNumber>;
	type GetStableCurrencyId: Get<CurrencyIdOf<Self>>;
	type GetNativeCurrencyId: Get<CurrencyIdOf<Self>>;
	type GetAmountAdjustment: Get<Rate>;
	type Treasury: CDPTreasury<Self::AccountId, Balance = BalanceOf<Self>, CurrencyId = CurrencyIdOf<Self>>;
	type PriceSource: PriceProvider<CurrencyIdOf<Self>, Price>;
}

decl_event!(
	pub enum Event<T>
	where
		AuctionId = AuctionIdOf<T>,
		CurrencyId = CurrencyIdOf<T>,
		Balance = BalanceOf<T>,
	{
		NewCollateralAuction(AuctionId, CurrencyId, Balance, Balance),
		NewDebitAuction(AuctionId, Balance, Balance),
		NewSurplusAuction(AuctionId, Balance),
		CancelAuction(AuctionId),
	}
);

decl_error! {
	/// Error for auction manager module.
	pub enum Error for Module<T: Trait> {
		AuctionNotExsits,
		InReservedStage,
		BalanceNotEnough,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as AuctionManager {
		pub CollateralAuctions get(fn collateral_auctions): map AuctionIdOf<T> =>
			Option<CollateralAuctionItem<T::AccountId, CurrencyIdOf<T>, BalanceOf<T>, T::BlockNumber>>;
		pub DebitAuctions get(fn debit_auctions): map AuctionIdOf<T> =>
			Option<DebitAuctionItem<BalanceOf<T>, T::BlockNumber>>;
		pub SurplusAuctions get(fn surplus_auctions): map AuctionIdOf<T> =>
			Option<SurplusAuctionItem<BalanceOf<T>, T::BlockNumber>>;
		pub TotalCollateralInAuction get(fn total_collateral_in_auction): map CurrencyIdOf<T> => BalanceOf<T>;
		pub TotalTargetInAuction get(fn total_target_in_auction): BalanceOf<T>;
		pub TotalDebitInAuction get(fn total_debit_in_auction): BalanceOf<T>;
		pub TotalSurplusInAuction get(fn total_surplus_in_auction): BalanceOf<T>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		MODULE_ID.into_account()
	}

	pub fn cancel_surplus_auction(id: AuctionIdOf<T>) -> DispatchResult {
		if let Some(surplus_auction) = <SurplusAuctions<T>>::take(id) {
			if let Some(auction_info) = T::Auction::auction_info(id) {
				// if these's bid, refund native token to the bidder
				if let Some((bidder, bid_price)) = auction_info.bid {
					let native_currency_id = T::GetNativeCurrencyId::get();
					if T::Currency::balance(native_currency_id, &bidder)
						.checked_add(&bid_price)
						.is_some()
					{
						T::Currency::deposit(native_currency_id, &bidder, bid_price)
							.expect("never failed after overflow check");
					}
				}
			}

			// move stable token of this surplus auction from module account to cdp treasury
			let stable_currency_id = T::GetStableCurrencyId::get();
			if T::Currency::ensure_can_withdraw(stable_currency_id, &Self::account_id(), surplus_auction.amount).is_ok()
			{
				T::Currency::withdraw(stable_currency_id, &Self::account_id(), surplus_auction.amount)
					.expect("never failed after balance check");
				T::Treasury::on_system_surplus(surplus_auction.amount);
			}

			// decrease total surplus in auction
			<TotalSurplusInAuction<T>>::mutate(|balance| *balance = balance.saturating_sub(surplus_auction.amount));

			// remove the auction info in auction module
			T::Auction::remove_auction(id);

			<Module<T>>::deposit_event(RawEvent::CancelAuction(id));
			Ok(())
		} else {
			Err(Error::<T>::AuctionNotExsits.into())
		}
	}

	pub fn cancel_debit_auction(id: AuctionIdOf<T>) -> DispatchResult {
		if let Some(debit_auction) = <DebitAuctions<T>>::take(id) {
			if let Some(auction_info) = T::Auction::auction_info(id) {
				// if these's bid, refund stable token to the bidder
				if let Some((bidder, _)) = auction_info.bid {
					let stable_currency_id = T::GetStableCurrencyId::get();
					if T::Currency::balance(stable_currency_id, &bidder)
						.checked_add(&debit_auction.fix)
						.is_some()
					{
						T::Currency::deposit(stable_currency_id, &bidder, debit_auction.fix)
							.expect("never failed after overflow check");
					}
				}
			}

			// add debit to cdp treasury and decrease total debit in auction
			T::Treasury::on_system_debit(debit_auction.fix);
			<TotalDebitInAuction<T>>::mutate(|balance| *balance = balance.saturating_sub(debit_auction.fix));

			// remove the auction info in auction module
			T::Auction::remove_auction(id);

			<Module<T>>::deposit_event(RawEvent::CancelAuction(id));
			Ok(())
		} else {
			Err(Error::<T>::AuctionNotExsits.into())
		}
	}

	pub fn cancel_collateral_auction(id: AuctionIdOf<T>) -> DispatchResult {
		if let Some(collateral_auction) = Self::collateral_auctions(id) {
			// must not in reserve bid stage
			ensure!(
				!Self::collateral_auction_in_reverse_stage(id),
				Error::<T>::InReservedStage
			);

			let stable_currency_id = T::GetStableCurrencyId::get();
			if let Some(auction_info) = T::Auction::auction_info(id) {
				// if these's bid, refund stable token to the bidder
				if let Some((bidder, bid_price)) = auction_info.bid {
					if T::Currency::balance(stable_currency_id, &bidder)
						.checked_add(&bid_price)
						.is_some()
					{
						T::Currency::deposit(stable_currency_id, &bidder, bid_price)
							.expect("never failed after overflow check");
					}
					T::Treasury::on_system_debit(bid_price);
				}
			}

			// calculate which amount of collateral to offset target
			// in price stable:collateral
			let price: Price =
				T::PriceSource::get_price(collateral_auction.currency_id, stable_currency_id).unwrap_or_default();
			let confiscate_collateral_amount = rstd::cmp::min(
				price.saturating_mul_int(&collateral_auction.target),
				collateral_auction.amount,
			);
			let refund_collateral_amount = collateral_auction.amount.saturating_sub(confiscate_collateral_amount);

			ensure!(
				T::Currency::ensure_can_withdraw(
					collateral_auction.currency_id,
					&Self::account_id(),
					collateral_auction.amount
				)
				.is_ok(),
				Error::<T>::BalanceNotEnough,
			);

			// confiscate colleteral from module account to cdp treasury
			T::Currency::withdraw(
				collateral_auction.currency_id,
				&Self::account_id(),
				confiscate_collateral_amount,
			)
			.expect("never failed after balance check");
			T::Treasury::deposit_system_collateral(collateral_auction.currency_id, confiscate_collateral_amount)
				.expect("never failed because this amount can not cause overflow");

			// refund remain collateral to auction owner
			if !refund_collateral_amount.is_zero() {
				T::Currency::transfer(
					collateral_auction.currency_id,
					&Self::account_id(),
					&collateral_auction.owner,
					refund_collateral_amount,
				)
				.expect("never failed after balance check");
			}

			// decrease total collateral in auction
			<TotalCollateralInAuction<T>>::mutate(collateral_auction.currency_id, |balance| {
				*balance = balance.saturating_sub(collateral_auction.amount)
			});

			// remove collateral auction
			<TotalTargetInAuction<T>>::mutate(|balance| *balance = balance.saturating_sub(collateral_auction.target));
			<CollateralAuctions<T>>::remove(id);
			T::Auction::remove_auction(id);

			<Module<T>>::deposit_event(RawEvent::CancelAuction(id));
			Ok(())
		} else {
			Err(Error::<T>::AuctionNotExsits.into())
		}
	}

	pub fn collateral_auction_in_reverse_stage(id: AuctionIdOf<T>) -> bool {
		if let Some(collateral_auction) = <CollateralAuctions<T>>::get(id) {
			if let Some(auction_info) = T::Auction::auction_info(id) {
				if let Some((_, bid_price)) = auction_info.bid {
					return bid_price >= collateral_auction.target;
				}
			}
		}

		false
	}

	/// Check `new_price` is larger than minimum increment
	/// Formula: new_price - last_price >= max(last_price, target_price) * minimum_increment
	pub fn check_minimum_increment(
		new_price: BalanceOf<T>,
		last_price: BalanceOf<T>,
		target_price: BalanceOf<T>,
		minimum_increment: Rate,
	) -> bool {
		if let (Some(target), Some(result)) = (
			minimum_increment.checked_mul_int(rstd::cmp::max(&target_price, &last_price)),
			new_price.checked_sub(&last_price),
		) {
			return result >= target;
		}
		false
	}

	pub fn get_minimum_increment_size(now: T::BlockNumber, start_block: T::BlockNumber) -> Rate {
		if now >= start_block + T::AuctionDurationSoftCap::get() {
			T::MinimumIncrementSize::get().saturating_mul(Rate::from_natural(2))
		} else {
			T::MinimumIncrementSize::get()
		}
	}

	pub fn get_auction_time_to_close(now: T::BlockNumber, start_block: T::BlockNumber) -> T::BlockNumber {
		if now >= start_block + T::AuctionDurationSoftCap::get() {
			T::AuctionTimeToClose::get() / 2.into()
		} else {
			T::AuctionTimeToClose::get()
		}
	}

	pub fn collateral_auction_bid_handler(
		now: T::BlockNumber,
		id: AuctionIdOf<T>,
		new_bid: (T::AccountId, BalanceOf<T>),
		last_bid: Option<(T::AccountId, BalanceOf<T>)>,
	) -> OnNewBidResult<T::BlockNumber> {
		if let Some(mut collateral_auction) = Self::collateral_auctions(id) {
			// get last price, if these's no bid set 0
			let last_price: BalanceOf<T> = match last_bid {
				None => 0.into(),
				Some((_, price)) => price,
			};
			let stable_currency_id = T::GetStableCurrencyId::get();
			let payment = rstd::cmp::min(collateral_auction.target, new_bid.1);

			// check new price is larger than minimum increment and new bidder has enough stable coin
			if Self::check_minimum_increment(
				new_bid.1,
				last_price,
				collateral_auction.target,
				Self::get_minimum_increment_size(now, collateral_auction.start_time),
			) && T::Currency::balance(stable_currency_id, &(new_bid.0)) >= payment
			{
				let module_account = Self::account_id();
				let mut surplus_increment = payment;

				// first: deduct amount of stablecoin from new bidder, add this to auction manager module
				T::Currency::withdraw(stable_currency_id, &(new_bid.0), payment)
					.expect("never failed after balance check");

				// second: if these's bid before, return stablecoin from auction manager module to last bidder
				if let Some((last_bidder, last_price)) = last_bid {
					let refund = rstd::cmp::min(last_price, collateral_auction.target);
					surplus_increment -= refund;

					T::Currency::deposit(stable_currency_id, &last_bidder, refund)
						.expect("never failed because payment >= refund");
				}

				if !surplus_increment.is_zero() {
					T::Treasury::on_system_surplus(surplus_increment);
				}

				// third: if bid_price > target, the auction is in reverse, refund collateral to it's origin from auction manager module
				if new_bid.1 > collateral_auction.target {
					let new_amount =
						Rate::from_rational(rstd::cmp::max(last_price, collateral_auction.target), new_bid.1)
							.checked_mul_int(&collateral_auction.amount)
							.unwrap_or(collateral_auction.amount);
					let deduct_amount = collateral_auction.amount.saturating_sub(new_amount);

					// ensure have sufficient collateral in auction module
					if Self::total_collateral_in_auction(collateral_auction.currency_id) >= deduct_amount {
						T::Currency::transfer(
							collateral_auction.currency_id,
							&module_account,
							&(collateral_auction.owner),
							deduct_amount,
						)
						.expect("never failed after balance check");
						<TotalCollateralInAuction<T>>::mutate(collateral_auction.currency_id, |balance| {
							*balance -= deduct_amount
						});
					}

					// update collateral auction
					collateral_auction.amount = new_amount;
					<CollateralAuctions<T>>::insert(id, collateral_auction.clone());
				}

				return OnNewBidResult {
					accept_bid: true,
					auction_end: Some(Some(
						now + Self::get_auction_time_to_close(now, collateral_auction.start_time),
					)),
				};
			}
		}

		OnNewBidResult {
			accept_bid: false,
			auction_end: None,
		}
	}

	pub fn debit_auction_bid_handler(
		now: T::BlockNumber,
		id: AuctionIdOf<T>,
		new_bid: (T::AccountId, BalanceOf<T>),
		last_bid: Option<(T::AccountId, BalanceOf<T>)>,
	) -> OnNewBidResult<T::BlockNumber> {
		if let Some(mut debit_auction) = Self::debit_auctions(id) {
			let last_price: BalanceOf<T> = match last_bid {
				None => 0.into(),
				Some((_, price)) => price,
			};
			let stable_currency_id = T::GetStableCurrencyId::get();

			if Self::check_minimum_increment(
				new_bid.1,
				last_price,
				debit_auction.fix,
				Self::get_minimum_increment_size(now, debit_auction.start_time),
			) && new_bid.1 >= debit_auction.fix
				&& T::Currency::ensure_can_withdraw(stable_currency_id, &new_bid.0, debit_auction.fix).is_ok()
			{
				if let Some((last_bidder, _)) = last_bid {
					// these's bid before, transfer the stablecoin from new bidder to last bidder
					T::Currency::transfer(stable_currency_id, &new_bid.0, &last_bidder, debit_auction.fix)
						.expect("never failed after balance check");
				} else {
					// these's no bid before, on_surplus to treasury
					T::Currency::withdraw(stable_currency_id, &new_bid.0, debit_auction.fix)
						.expect("never failed after balance check");

					// add surplus for cdp treasury
					T::Treasury::on_system_surplus(debit_auction.fix);
				}

				// if bid price is more than fix
				// calculate new amount of issue native token
				if new_bid.1 > debit_auction.fix {
					debit_auction.amount =
						Rate::from_rational(rstd::cmp::max(last_price, debit_auction.fix), new_bid.1)
							.checked_mul_int(&debit_auction.amount)
							.unwrap_or(debit_auction.amount);
					<DebitAuctions<T>>::insert(id, debit_auction.clone());
				}

				return OnNewBidResult {
					accept_bid: true,
					auction_end: Some(Some(
						now + Self::get_auction_time_to_close(now, debit_auction.start_time),
					)),
				};
			}
		}

		OnNewBidResult {
			accept_bid: false,
			auction_end: None,
		}
	}

	pub fn surplus_auction_bid_handler(
		now: T::BlockNumber,
		id: AuctionIdOf<T>,
		new_bid: (T::AccountId, BalanceOf<T>),
		last_bid: Option<(T::AccountId, BalanceOf<T>)>,
	) -> OnNewBidResult<T::BlockNumber> {
		if let Some(surplus_auction) = Self::surplus_auctions(id) {
			let last_price: BalanceOf<T> = match last_bid {
				None => 0.into(),
				Some((_, price)) => price,
			};
			let native_currency_id = T::GetNativeCurrencyId::get();

			// check new price is larger than minimum increment and new bidder has enough native token
			if Self::check_minimum_increment(
				new_bid.1,
				last_price,
				0.into(),
				Self::get_minimum_increment_size(now, surplus_auction.start_time),
			) && T::Currency::ensure_can_withdraw(native_currency_id, &(new_bid.0), new_bid.1).is_ok()
				&& !new_bid.1.is_zero()
			{
				let mut burn_native_currency_amount = new_bid.1;

				// if these's bid before, transfer the  stablecoin from auction manager module to last bidder
				if let Some((last_bidder, last_price)) = last_bid {
					burn_native_currency_amount = burn_native_currency_amount.saturating_sub(last_price);
					T::Currency::transfer(native_currency_id, &new_bid.0, &last_bidder, last_price)
						.expect("never failed after balance check");
				}

				// burn remain native token from new bidder
				T::Currency::withdraw(native_currency_id, &new_bid.0, burn_native_currency_amount)
					.expect("never failed after balance check");

				return OnNewBidResult {
					accept_bid: true,
					auction_end: Some(Some(
						now + Self::get_auction_time_to_close(now, surplus_auction.start_time),
					)),
				};
			}
		}

		OnNewBidResult {
			accept_bid: false,
			auction_end: None,
		}
	}

	pub fn collateral_auction_end_handler(id: AuctionIdOf<T>, winner: Option<(T::AccountId, BalanceOf<T>)>) {
		if let (Some(collateral_auction), Some((bidder, _))) = (Self::collateral_auctions(id), winner) {
			let amount = rstd::cmp::min(
				collateral_auction.amount,
				Self::total_collateral_in_auction(collateral_auction.currency_id),
			);
			if T::Currency::balance(collateral_auction.currency_id, &bidder)
				.checked_add(&amount)
				.is_some()
			{
				T::Currency::transfer(collateral_auction.currency_id, &Self::account_id(), &bidder, amount)
					.expect("never failed after overflow check");
			}
			<TotalCollateralInAuction<T>>::mutate(collateral_auction.currency_id, |balance| *balance -= amount);
			<TotalTargetInAuction<T>>::mutate(|balance| *balance -= collateral_auction.target);
			<CollateralAuctions<T>>::remove(id);
		}
	}

	pub fn debit_auction_end_handler(id: AuctionIdOf<T>, winner: Option<(T::AccountId, BalanceOf<T>)>) {
		if let Some(debit_auction) = Self::debit_auctions(id) {
			if let Some((bidder, _)) = winner {
				// issue the amount of native token to winner
				if T::Currency::balance(T::GetNativeCurrencyId::get(), &bidder)
					.checked_add(&debit_auction.amount)
					.is_some()
				{
					T::Currency::deposit(T::GetNativeCurrencyId::get(), &bidder, debit_auction.amount)
						.expect("never failed after overflow check");
				}
				// decrease debit in auction and delete auction
				<TotalDebitInAuction<T>>::mutate(|balance| *balance -= debit_auction.fix);
				<DebitAuctions<T>>::remove(id);
			} else {
				// there's no bidder until auction closed, adjust the native token amount
				let start_block = <system::Module<T>>::block_number();
				let end_block = start_block + T::AuctionTimeToClose::get();
				let new_debit_auction_id: AuctionIdOf<T> = T::Auction::new_auction(start_block, Some(end_block));
				let new_amount = debit_auction
					.amount
					.saturating_add(T::GetAmountAdjustment::get().saturating_mul_int(&debit_auction.amount));
				let new_debit_auction = DebitAuctionItem {
					amount: new_amount,
					fix: debit_auction.fix,
					start_time: start_block,
				};
				<DebitAuctions<T>>::insert(new_debit_auction_id, new_debit_auction.clone());
				<DebitAuctions<T>>::remove(id);
				<Module<T>>::deposit_event(RawEvent::NewDebitAuction(
					new_debit_auction_id,
					new_debit_auction.amount,
					new_debit_auction.fix,
				));
			}
		}
	}

	pub fn surplus_auction_end_handler(id: AuctionIdOf<T>, winner: Option<(T::AccountId, BalanceOf<T>)>) {
		if let (Some(surplus_auction), Some((bidder, _))) = (Self::surplus_auctions(id), winner) {
			// transfer the amount of stable token from module to winner
			if T::Currency::balance(T::GetStableCurrencyId::get(), &bidder)
				.checked_add(&surplus_auction.amount)
				.is_some() && T::Currency::ensure_can_withdraw(
				T::GetStableCurrencyId::get(),
				&Self::account_id(),
				surplus_auction.amount,
			)
			.is_ok()
			{
				T::Currency::transfer(
					T::GetStableCurrencyId::get(),
					&Self::account_id(),
					&bidder,
					surplus_auction.amount,
				)
				.expect("never failed after overflow check");
			}

			// decrease surplus in auction
			<TotalSurplusInAuction<T>>::mutate(|balance| *balance -= surplus_auction.amount);
			<SurplusAuctions<T>>::remove(id);
		}
	}
}

impl<T: Trait> AuctionHandler<T::AccountId, BalanceOf<T>, T::BlockNumber, AuctionIdOf<T>> for Module<T> {
	fn on_new_bid(
		now: T::BlockNumber,
		id: AuctionIdOf<T>,
		new_bid: (T::AccountId, BalanceOf<T>),
		last_bid: Option<(T::AccountId, BalanceOf<T>)>,
	) -> OnNewBidResult<T::BlockNumber> {
		if <CollateralAuctions<T>>::exists(id) {
			Self::collateral_auction_bid_handler(now, id, new_bid, last_bid)
		} else if <DebitAuctions<T>>::exists(id) {
			Self::debit_auction_bid_handler(now, id, new_bid, last_bid)
		} else if <SurplusAuctions<T>>::exists(id) {
			Self::surplus_auction_bid_handler(now, id, new_bid, last_bid)
		} else {
			OnNewBidResult {
				accept_bid: false,
				auction_end: None,
			}
		}
	}

	fn on_auction_ended(id: AuctionIdOf<T>, winner: Option<(T::AccountId, BalanceOf<T>)>) {
		if <CollateralAuctions<T>>::exists(id) {
			Self::collateral_auction_end_handler(id, winner)
		} else if <DebitAuctions<T>>::exists(id) {
			Self::debit_auction_end_handler(id, winner)
		} else if <SurplusAuctions<T>>::exists(id) {
			Self::surplus_auction_end_handler(id, winner)
		}
	}
}

impl<T: Trait> AuctionManager<T::AccountId> for Module<T> {
	type CurrencyId = CurrencyIdOf<T>;
	type Balance = BalanceOf<T>;

	fn new_collateral_auction(
		who: &T::AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
		target: Self::Balance,
		bad_debt: Self::Balance,
	) {
		if Self::total_collateral_in_auction(currency_id)
			.checked_add(&amount)
			.is_some() && Self::total_target_in_auction().checked_add(&target).is_some()
			&& T::Currency::balance(currency_id, &Self::account_id())
				.checked_add(&amount)
				.is_some()
		{
			T::Currency::deposit(currency_id, &Self::account_id(), amount).expect("never failed after overflow check");
			<TotalCollateralInAuction<T>>::mutate(currency_id, |balance| *balance += amount);
			<TotalTargetInAuction<T>>::mutate(|balance| *balance += target);
			T::Treasury::on_system_debit(bad_debt);

			let block_number = <system::Module<T>>::block_number();
			let auction_id: AuctionIdOf<T> = T::Auction::new_auction(block_number, None);
			let collateral_aution = CollateralAuctionItem {
				owner: who.clone(),
				currency_id: currency_id,
				amount: amount,
				target: target,
				start_time: block_number,
			};

			<CollateralAuctions<T>>::insert(auction_id, collateral_aution);
			<Module<T>>::deposit_event(RawEvent::NewCollateralAuction(auction_id, currency_id, amount, target));
		}
	}

	fn new_debit_auction(initial_amount: Self::Balance, fix_debit: Self::Balance) {
		if Self::total_debit_in_auction().checked_add(&fix_debit).is_some() {
			<TotalDebitInAuction<T>>::mutate(|balance| *balance += fix_debit);
			let start_block = <system::Module<T>>::block_number();
			let end_block = start_block + T::AuctionTimeToClose::get();
			// set close time for initial debit auction
			let auction_id: AuctionIdOf<T> = T::Auction::new_auction(start_block, Some(end_block));
			let debit_auction = DebitAuctionItem {
				amount: initial_amount,
				fix: fix_debit,
				start_time: start_block,
			};
			<DebitAuctions<T>>::insert(auction_id, debit_auction);
			<Module<T>>::deposit_event(RawEvent::NewDebitAuction(auction_id, initial_amount, fix_debit));
		}
	}

	fn new_surplus_auction(amount: Self::Balance) {
		if Self::total_surplus_in_auction().checked_add(&amount).is_some()
			&& T::Currency::balance(T::GetStableCurrencyId::get(), &Self::account_id())
				.checked_add(&amount)
				.is_some()
		{
			T::Currency::deposit(T::GetStableCurrencyId::get(), &Self::account_id(), amount)
				.expect("never failed after overflow check");
			<TotalSurplusInAuction<T>>::mutate(|balance| *balance += amount);
			let auction_id: AuctionIdOf<T> = T::Auction::new_auction(<system::Module<T>>::block_number(), None);
			let surplus_auction = SurplusAuctionItem {
				amount: amount,
				start_time: <system::Module<T>>::block_number(),
			};
			<SurplusAuctions<T>>::insert(auction_id, surplus_auction);
			<Module<T>>::deposit_event(RawEvent::NewSurplusAuction(auction_id, amount));
		}
	}

	fn get_total_debit_in_auction() -> Self::Balance {
		Self::total_debit_in_auction()
	}

	fn get_total_target_in_auction() -> Self::Balance {
		Self::total_target_in_auction()
	}
}

impl<T: Trait> AuctionManagerExtended<T::AccountId> for Module<T> {
	type AuctionId = AuctionIdOf<T>;

	fn get_total_collateral_in_auction(id: Self::CurrencyId) -> Self::Balance {
		Self::total_collateral_in_auction(id)
	}

	fn get_total_surplus_in_auction() -> Self::Balance {
		Self::total_surplus_in_auction()
	}

	fn cancel_auction(id: Self::AuctionId) -> DispatchResult {
		if <CollateralAuctions<T>>::exists(id) {
			Self::cancel_collateral_auction(id)
		} else if <DebitAuctions<T>>::exists(id) {
			Self::cancel_debit_auction(id)
		} else if <SurplusAuctions<T>>::exists(id) {
			Self::cancel_surplus_auction(id)
		} else {
			Err(Error::<T>::AuctionNotExsits.into())
		}
	}
}
