#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use paint_support::{decl_error, decl_event, decl_module, decl_storage, traits::Get};
use sr_primitives::{Permill, RuntimeDebug};
use traits::{Auction, AuctionHandler, MultiCurrency, MultiCurrencyExtended, OnNewBidResult};

mod mock;
mod tests;

#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct AuctionItem<AccountId, CurrencyId, Balance, BlockNumber> {
	owner: AccountId,
	currency_id: CurrencyId,
	amount: Balance,
	target: Balance,
	start_time: BlockNumber,
}

pub type BalanceOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
pub type CurrencyIdOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;
pub type AuctionIdOf<T> =
	<<T as Trait>::Auction as Auction<<T as system::Trait>::AccountId, <T as system::Trait>::BlockNumber>>::AuctionId;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type Currency: MultiCurrencyExtended<Self::AccountId>;
	type Auction: Auction<Self::AccountId, Self::BlockNumber>;
	//type Handler: AuctionManagerHandler<CurrencyIdOf<Self>, BalanceOf<Self>>;
	type MinimumIncrementSize: Get<Permill>;
	type AuctionTimeToClose: Get<Self::BlockNumber>;
	type AuctionDurationSoftCap: Get<Self::BlockNumber>;
	type GetNativeCurrencyId: Get<CurrencyIdOf<Self>>;
}

decl_event!(
	pub enum Event<T>
	where
		AuctionId = AuctionIdOf<T>,
		CurrencyId = CurrencyIdOf<T>,
		Balance = BalanceOf<T>,
	{
		CollateralAuction(AuctionId, CurrencyId, Balance, Balance),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as AuctionManager {
		pub MaximumAuctionSize get(fn maximum_auction_size): map CurrencyIdOf<T> => BalanceOf<T>;
		pub Auctions get(fn auctions): map AuctionIdOf<T> =>
			Option<AuctionItem<T::AccountId, CurrencyIdOf<T>, BalanceOf<T>, T::BlockNumber>>;
		pub TotalCollateralInAuction get(fn total_collateral_in_auction): map CurrencyIdOf<T> => BalanceOf<T>;
		pub BadDebtPool get(fn bad_debt_pool): BalanceOf<T>;
		pub SurplusPool get(fn surplus_pool): BalanceOf<T>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		fn on_finalize(_now: T::BlockNumber) {
			let amount = std::cmp::max(Self::bad_debt_pool(), Self::surplus_pool());
			if amount > 0.into() {
				<BadDebtPool<T>>::mutate(|debt| *debt -= amount);
				<SurplusPool<T>>::mutate(|surplus| *surplus -= amount);
			}
		}
	}
}

decl_error! {
	/// Error for auction-manager module.
	pub enum Error {
		BalanceTooLow,
	}
}

impl<T: Trait> Module<T> {
	pub fn set_maximum_auction_size(currency_id: CurrencyIdOf<T>, size: BalanceOf<T>) {
		<MaximumAuctionSize<T>>::insert(currency_id, size);
	}

	pub fn new_collateral_auction(
		who: T::AccountId,
		currency_id: CurrencyIdOf<T>,
		amount: BalanceOf<T>,
		target: BalanceOf<T>,
	) {
		let maximum_auction_size = Self::maximum_auction_size(currency_id);
		let mut unhandled_amount: BalanceOf<T> = amount;
		let mut unhandled_target: BalanceOf<T> = target;

		while unhandled_amount > 0.into() {
			let (lot_amount, lot_target) =
				if unhandled_amount > maximum_auction_size && maximum_auction_size != 0.into() {
					(maximum_auction_size, target * maximum_auction_size / amount)
				} else {
					(unhandled_amount, unhandled_target)
				};

			let auction_id: AuctionIdOf<T> = T::Auction::new_auction(
				<system::Module<T>>::block_number(),
				Some(<system::Module<T>>::block_number() + T::AuctionTimeToClose::get()),
			);
			let aution_item = AuctionItem {
				owner: who.clone(),
				currency_id: currency_id,
				amount: lot_amount,
				target: lot_target,
				start_time: <system::Module<T>>::block_number(),
			};
			<Auctions<T>>::insert(auction_id, aution_item);
			Self::deposit_event(RawEvent::CollateralAuction(
				auction_id,
				currency_id,
				lot_amount,
				lot_target,
			));

			unhandled_amount -= lot_amount;
			unhandled_target -= lot_target;
		}

		<TotalCollateralInAuction<T>>::mutate(currency_id, |balance| *balance += amount);
		<BadDebtPool<T>>::mutate(|debt| *debt += target);
	}
}

impl<T: Trait> AuctionHandler<T::AccountId, BalanceOf<T>, T::BlockNumber, AuctionIdOf<T>> for Module<T> {
	fn on_new_bid(
		now: T::BlockNumber,
		id: AuctionIdOf<T>,
		new_bid: (T::AccountId, BalanceOf<T>),
		last_bid: Option<(T::AccountId, BalanceOf<T>)>,
	) -> OnNewBidResult<T::BlockNumber> {
		if let Some(mut auction_item) = Self::auctions(id) {
			let (minimum_increment_size, auction_time_to_close) =
				if now >= auction_item.start_time + T::AuctionDurationSoftCap::get() {
					(
						Permill::from_parts(T::MinimumIncrementSize::get().deconstruct() * 2_u32),
						T::AuctionTimeToClose::get() / 2.into(),
					)
				} else {
					(T::MinimumIncrementSize::get(), T::AuctionTimeToClose::get())
				};

			let current_price: BalanceOf<T> = match last_bid {
				None => 0.into(),
				Some((_, price)) => price,
			};

			// 判断竞价是否有效
			if new_bid.1 - current_price >= minimum_increment_size * std::cmp::max(auction_item.target, current_price)
				&& T::Currency::balance(T::GetNativeCurrencyId::get(), &(new_bid.0))
					> std::cmp::min(auction_item.target, new_bid.1)
			{
				// 扣除bidder的投标款
				let payment = std::cmp::min(new_bid.1, auction_item.target);
				let _ = T::Currency::withdraw(T::GetNativeCurrencyId::get(), &(new_bid.0), payment);
				<SurplusPool<T>>::mutate(|surplus| *surplus += payment);

				// 向上一个winner退款
				if let Some((previous_bidder, previous_price)) = last_bid {
					let refund = std::cmp::min(previous_price, auction_item.target);
					let _ = T::Currency::deposit(T::GetNativeCurrencyId::get(), &(previous_bidder), refund);
					<SurplusPool<T>>::mutate(|surplus| *surplus -= refund);
				}

				// 逆向拍卖处理
				if new_bid.1 > auction_item.target {
					let new_amount =
						auction_item.amount * std::cmp::max(current_price, auction_item.target) / new_bid.1;
					let deduct_amount = auction_item.amount - new_amount;
					auction_item.amount = new_amount;

					let _ = T::Currency::deposit(auction_item.currency_id, &(auction_item.owner), deduct_amount);
					<TotalCollateralInAuction<T>>::mutate(auction_item.currency_id, |balance| {
						*balance -= deduct_amount
					});
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

	fn on_auction_ended(id: AuctionIdOf<T>, winner: Option<(T::AccountId, BalanceOf<T>)>) {
		if let Some(mut auction_item) = Self::auctions(id) {
			if let Some((bidder, _)) = winner {
				let _ = T::Currency::deposit(auction_item.currency_id, &bidder, auction_item.amount);
				<TotalCollateralInAuction<T>>::mutate(auction_item.currency_id, |balance| {
					*balance -= auction_item.amount
				});
				<Auctions<T>>::remove(id);
			} else {
				// 流拍的处理
				<Auctions<T>>::remove(id);

				let new_id: AuctionIdOf<T> = T::Auction::new_auction(
					<system::Module<T>>::block_number(),
					Some(<system::Module<T>>::block_number() + T::AuctionTimeToClose::get()),
				);
				auction_item.start_time = <system::Module<T>>::block_number();

				<Auctions<T>>::insert(new_id, auction_item.clone());
				Self::deposit_event(RawEvent::CollateralAuction(
					new_id,
					auction_item.currency_id,
					auction_item.amount,
					auction_item.target,
				));
			}
		}
	}
}
