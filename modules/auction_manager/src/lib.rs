#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{decl_event, decl_module, decl_storage, traits::Get, Parameter};
use orml_traits::{Auction, AuctionHandler, MultiCurrency, OnNewBidResult};
use sp_runtime::{
	traits::{
		AccountIdConversion, CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, MaybeSerializeDeserialize, Member,
		SimpleArithmetic, Zero,
	},
	ModuleId, RuntimeDebug,
};
use support::{AuctionManager, CDPTreasury, Rate};
use system::ensure_root;

mod mock;
mod tests;

const MODULE_ID: ModuleId = ModuleId(*b"aca/amgr");

#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct AuctionItem<AccountId, CurrencyId, Balance, BlockNumber> {
	owner: AccountId,
	currency_id: CurrencyId,
	amount: Balance,
	target: Balance,
	start_time: BlockNumber,
}

type AuctionIdOf<T> =
	<<T as Trait>::Auction as Auction<<T as system::Trait>::AccountId, <T as system::Trait>::BlockNumber>>::AuctionId;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type CurrencyId: Parameter + Member + Copy + MaybeSerializeDeserialize;
	type Balance: Parameter + Member + SimpleArithmetic + Default + Copy + MaybeSerializeDeserialize;
	type Currency: MultiCurrency<Self::AccountId, CurrencyId = Self::CurrencyId, Balance = Self::Balance>;
	type Auction: Auction<Self::AccountId, Self::BlockNumber>;
	type MinimumIncrementSize: Get<Rate>;
	type AuctionTimeToClose: Get<Self::BlockNumber>;
	type AuctionDurationSoftCap: Get<Self::BlockNumber>;
	type GetStableCurrencyId: Get<Self::CurrencyId>;
	type Treasury: CDPTreasury<Balance = Self::Balance>;
}

decl_event!(
	pub enum Event<T>
	where
		AuctionId = AuctionIdOf<T>,
		CurrencyId = <T as Trait>::CurrencyId,
		Balance = <T as Trait>::Balance,
	{
		CollateralAuction(AuctionId, CurrencyId, Balance, Balance),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as AuctionManager {
		MaximumAuctionSize get(fn maximum_auction_size): map T::CurrencyId => T::Balance;
		Auctions get(fn auctions): map AuctionIdOf<T> =>
			Option<AuctionItem<T::AccountId, T::CurrencyId, T::Balance, T::BlockNumber>>;
		TotalCollateralInAuction get(fn total_collateral_in_auction): map T::CurrencyId => T::Balance;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		fn set_maximum_auction_size(origin, currency_id: T::CurrencyId, size: T::Balance) {
			ensure_root(origin)?;
			<MaximumAuctionSize<T>>::insert(currency_id, size);
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		MODULE_ID.into_account()
	}

	/// Check `new_price` is larger than minimum increment
	/// Formula: bid_price - last_price >= max(last_price, target) * minimum_increment_size
	pub fn check_minimum_increment(
		new_price: &T::Balance,
		last_price: &T::Balance,
		target_price: &T::Balance,
		minimum_increment: &Rate,
	) -> bool {
		if let (Some(target), Some(result)) = (
			minimum_increment.checked_mul_int(rstd::cmp::max(target_price, last_price)),
			new_price.checked_sub(last_price),
		) {
			return result >= target;
		}

		false
	}
}

impl<T: Trait> AuctionHandler<T::AccountId, T::Balance, T::BlockNumber, AuctionIdOf<T>> for Module<T> {
	fn on_new_bid(
		now: T::BlockNumber,
		id: AuctionIdOf<T>,
		new_bid: (T::AccountId, T::Balance),
		last_bid: Option<(T::AccountId, T::Balance)>,
	) -> OnNewBidResult<T::BlockNumber> {
		if let Some(mut auction_item) = Self::auctions(id) {
			// calculate min_increment_size and auction_time_to_close according to elapsed time

			let (minimum_increment_size, auction_time_to_close) =
				if now >= auction_item.start_time + T::AuctionDurationSoftCap::get() {
					T::MinimumIncrementSize::get()
						.checked_mul(&Rate::from_natural(2))
						.and_then(|increment| Some((increment, T::AuctionTimeToClose::get() / 2.into())))
						.unwrap_or((T::MinimumIncrementSize::get(), T::AuctionTimeToClose::get()))
				} else {
					(T::MinimumIncrementSize::get(), T::AuctionTimeToClose::get())
				};

			// get last price, if these's no bid set 0
			let last_price: T::Balance = match last_bid {
				None => 0.into(),
				Some((_, price)) => price,
			};

			let stable_currency_id = T::GetStableCurrencyId::get();
			let payment = rstd::cmp::min(auction_item.target, new_bid.1);

			// check new price is larger than minimum increment
			// check new bidder has enough stable coin
			if Self::check_minimum_increment(&new_bid.1, &last_price, &auction_item.target, &minimum_increment_size)
				&& T::Currency::balance(stable_currency_id, &(new_bid.0)) >= payment
			{
				let module_account = Self::account_id();
				let mut surplus_increment = payment;

				// first: deduct amount of stablecoin from new bidder, add this to auction manager module
				T::Currency::withdraw(stable_currency_id, &(new_bid.0), payment)
					.expect("never failed after balance check");

				// second: if these's bid before, return stablecoin from auction manager module to last bidder
				if let Some((last_bidder, last_price)) = last_bid {
					let refund = rstd::cmp::min(last_price, auction_item.target);
					surplus_increment -= refund;

					T::Currency::deposit(stable_currency_id, &last_bidder, refund)
						.expect("never failed because payment >= refund");
				}

				if !surplus_increment.is_zero() {
					T::Treasury::on_surplus(surplus_increment);
				}

				// third: if bid_price > target, the auction is in reverse, refund collateral to it's origin from auction manager module
				if new_bid.1 > auction_item.target {
					let new_amount = auction_item
						.amount
						.checked_mul(rstd::cmp::max(&last_price, &auction_item.target))
						.and_then(|n| n.checked_div(&new_bid.1))
						.unwrap_or(auction_item.amount);

					let deduct_amount = auction_item.amount.checked_sub(&new_amount).unwrap_or_default();

					// ensure have sufficient collateral in auction module
					if Self::total_collateral_in_auction(auction_item.currency_id) >= deduct_amount {
						T::Currency::transfer(
							auction_item.currency_id,
							&module_account,
							&(auction_item.owner),
							deduct_amount,
						)
						.expect("never failed after balance check");
						<TotalCollateralInAuction<T>>::mutate(auction_item.currency_id, |balance| {
							*balance -= deduct_amount
						});

						auction_item.amount = new_amount;
					}

					<Auctions<T>>::insert(id, auction_item);
				}

				return OnNewBidResult {
					accept_bid: true,
					auction_end: Some(Some(now + auction_time_to_close)),
				};
			}
		}

		OnNewBidResult {
			accept_bid: false,
			auction_end: None,
		}
	}

	fn on_auction_ended(id: AuctionIdOf<T>, winner: Option<(T::AccountId, T::Balance)>) {
		if let (Some(auction_item), Some((bidder, _))) = (Self::auctions(id), winner) {
			// these's bidder for this auction, transfer collateral to bidder
			let amount = rstd::cmp::min(
				auction_item.amount,
				Self::total_collateral_in_auction(auction_item.currency_id),
			);
			T::Currency::transfer(auction_item.currency_id, &Self::account_id(), &bidder, amount)
				.expect("never failed because use");
			<TotalCollateralInAuction<T>>::mutate(auction_item.currency_id, |balance| *balance -= amount);
			<Auctions<T>>::remove(id);
		}
	}
}

impl<T: Trait> AuctionManager<T::AccountId> for Module<T> {
	type CurrencyId = T::CurrencyId;
	type Balance = T::Balance;

	fn new_collateral_auction(
		who: &T::AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
		target: Self::Balance,
		bad_debt: Self::Balance,
	) {
		if Self::total_collateral_in_auction(currency_id)
			.checked_add(&amount)
			.is_some() && T::Currency::balance(T::GetStableCurrencyId::get(), &Self::account_id())
			.checked_add(&amount)
			.is_some()
		{
			T::Currency::deposit(currency_id, &Self::account_id(), amount).expect("never failed after overflow check");
			<TotalCollateralInAuction<T>>::mutate(currency_id, |balance| *balance += amount);
			T::Treasury::on_debit(bad_debt);

			let maximum_auction_size = <Module<T>>::maximum_auction_size(currency_id);
			let mut unhandled_amount: Self::Balance = amount;
			let mut unhandled_target: Self::Balance = target;
			let block_number = <system::Module<T>>::block_number();

			while !unhandled_amount.is_zero() {
				let (lot_amount, lot_target) =
					if unhandled_amount > maximum_auction_size && !maximum_auction_size.is_zero() {
						target
							.checked_mul(&maximum_auction_size)
							.and_then(|n| n.checked_div(&amount))
							.and_then(|result| Some((maximum_auction_size, result)))
							.unwrap_or((unhandled_amount, unhandled_target))
					} else {
						(unhandled_amount, unhandled_target)
					};

				let auction_id: AuctionIdOf<T> = T::Auction::new_auction(block_number, None);
				let aution_item = AuctionItem {
					owner: who.clone(),
					currency_id: currency_id,
					amount: lot_amount,
					target: lot_target,
					start_time: block_number,
				};
				<Auctions<T>>::insert(auction_id, aution_item);
				<Module<T>>::deposit_event(RawEvent::CollateralAuction(
					auction_id,
					currency_id,
					lot_amount,
					lot_target,
				));

				// note: this will never fail, because of lot_* are always smaller or equal than unhandled_*
				unhandled_amount -= lot_amount;
				unhandled_target -= lot_target;
			}
		}
	}
}
