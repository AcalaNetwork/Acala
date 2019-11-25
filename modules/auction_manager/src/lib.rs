#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, FullCodec, HasCompact};
use orml_traits::{
	arithmetic::{self, Signed},
	Auction, AuctionHandler, MultiCurrency, MultiCurrencyExtended, OnNewBidResult,
};
use palette_support::{decl_event, decl_module, decl_storage, traits::Get, Parameter};
use rstd::{
	convert::{TryFrom, TryInto},
	fmt::Debug,
};
use sr_primitives::{
	traits::{AccountIdConversion, MaybeSerializeDeserialize, Member, SaturatedConversion, SimpleArithmetic},
	ModuleId, Permill, RuntimeDebug,
};
use support::AuctionManager;

mod mock;
mod tests;

const U128_MILLION: u128 = 1_000_000;
const MODULE_ID: ModuleId = ModuleId(*b"py/trsry");

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
	type CurrencyId: FullCodec + HasCompact + Eq + PartialEq + Copy + MaybeSerializeDeserialize + Debug;
	type Balance: Parameter + Member + SimpleArithmetic + Default + Copy + MaybeSerializeDeserialize;
	type Amount: Signed
		+ TryInto<Self::Balance>
		+ TryFrom<Self::Balance>
		+ Parameter
		+ Member
		+ arithmetic::SimpleArithmetic
		+ Default
		+ Copy
		+ MaybeSerializeDeserialize;
	type Currency: MultiCurrencyExtended<
		Self::AccountId,
		CurrencyId = Self::CurrencyId,
		Balance = Self::Balance,
		Amount = Self::Amount,
	>;
	type Auction: Auction<Self::AccountId, Self::BlockNumber>;
	type MinimumIncrementSize: Get<Permill>;
	type AuctionTimeToClose: Get<Self::BlockNumber>;
	type AuctionDurationSoftCap: Get<Self::BlockNumber>;
	type GetStableCurrencyId: Get<Self::CurrencyId>;
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
		BadDebtPool get(fn bad_debt_pool): T::Balance;
		SurplusPool get(fn surplus_pool): T::Balance;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		fn on_finalize(_now: T::BlockNumber) {
			let amount = std::cmp::min(Self::bad_debt_pool(), Self::surplus_pool());
			if amount > 0.into() {
				if T::Currency::withdraw(T::GetStableCurrencyId::get(), &<Module<T>>::account_id(), amount).is_ok() {
					<BadDebtPool<T>>::mutate(|debt| *debt -= amount);
					<SurplusPool<T>>::mutate(|surplus| *surplus -= amount);
				}
			}
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn set_maximum_auction_size(currency_id: T::CurrencyId, size: T::Balance) {
		<MaximumAuctionSize<T>>::insert(currency_id, size);
	}

	pub fn account_id() -> T::AccountId {
		MODULE_ID.into_account()
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
			let (minimum_increment_size, auction_time_to_close) =
				if now >= auction_item.start_time + T::AuctionDurationSoftCap::get() {
					(
						Permill::from_parts(T::MinimumIncrementSize::get().deconstruct() * 2_u32),
						T::AuctionTimeToClose::get() / 2.into(),
					)
				} else {
					(T::MinimumIncrementSize::get(), T::AuctionTimeToClose::get())
				};

			let current_price: T::Balance = match last_bid {
				None => 0.into(),
				Some((_, price)) => price,
			};

			let stable_currency_id = T::GetStableCurrencyId::get();
			let payment = std::cmp::min(auction_item.target, new_bid.1);

			// 判断竞价是否有效
			if (new_bid.1 - current_price).saturated_into::<u128>()
				>= u128::from(minimum_increment_size.deconstruct())
					* std::cmp::max(auction_item.target, current_price).saturated_into::<u128>()
					/ U128_MILLION && T::Currency::balance(stable_currency_id, &(new_bid.0)) > payment
			{
				let module_account = Self::account_id();

				// 扣除bidder的投标款
				let _ = T::Currency::transfer(stable_currency_id, &(new_bid.0), &module_account, payment);
				<SurplusPool<T>>::mutate(|surplus| *surplus += payment);

				// 向上一个winner退款
				if let Some((previous_bidder, previous_price)) = last_bid {
					let refund = std::cmp::min(previous_price, auction_item.target);
					let _ = T::Currency::transfer(stable_currency_id, &module_account, &previous_bidder, refund);
					<SurplusPool<T>>::mutate(|surplus| *surplus -= refund);
				}

				// 逆向拍卖处理
				if new_bid.1 > auction_item.target {
					let new_amount =
						auction_item.amount * std::cmp::max(current_price, auction_item.target) / new_bid.1;
					let deduct_amount = auction_item.amount - new_amount;
					auction_item.amount = new_amount;

					let _ = T::Currency::transfer(
						auction_item.currency_id,
						&module_account,
						&(auction_item.owner),
						deduct_amount,
					);
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

	fn on_auction_ended(id: AuctionIdOf<T>, winner: Option<(T::AccountId, T::Balance)>) {
		if let Some(mut auction_item) = Self::auctions(id) {
			if let Some((bidder, _)) = winner {
				let _ = T::Currency::transfer(
					auction_item.currency_id,
					&Self::account_id(),
					&bidder,
					auction_item.amount,
				);
				<TotalCollateralInAuction<T>>::mutate(auction_item.currency_id, |balance| {
					*balance -= auction_item.amount
				});
				<Auctions<T>>::remove(id);
			} else {
				// 流拍的处理
				<Auctions<T>>::remove(id);
				let block_number = <system::Module<T>>::block_number();

				let new_id: AuctionIdOf<T> =
					T::Auction::new_auction(block_number, Some(block_number + T::AuctionTimeToClose::get()));
				auction_item.start_time = block_number;

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

impl<T: Trait> AuctionManager<T::AccountId> for Module<T> {
	type CurrencyId = T::CurrencyId;
	type Balance = T::Balance;
	type Amount = T::Amount;

	fn increase_surplus(increment: Self::Balance) {
		let _ = T::Currency::deposit(T::GetStableCurrencyId::get(), &Self::account_id(), increment);
		<SurplusPool<T>>::mutate(|surplus| *surplus += increment);
	}

	fn new_collateral_auction(
		who: T::AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
		target: Self::Balance,
		bad_debt: Self::Balance,
	) {
		let maximum_auction_size = <Module<T>>::maximum_auction_size(currency_id);
		let mut unhandled_amount: Self::Balance = amount;
		let mut unhandled_target: Self::Balance = target;
		let block_number = <system::Module<T>>::block_number();

		while unhandled_amount > 0.into() {
			let (lot_amount, lot_target) =
				if unhandled_amount > maximum_auction_size && maximum_auction_size != 0.into() {
					(maximum_auction_size, target * maximum_auction_size / amount)
				} else {
					(unhandled_amount, unhandled_target)
				};

			let auction_id: AuctionIdOf<T> =
				T::Auction::new_auction(block_number, Some(block_number + T::AuctionTimeToClose::get()));
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

			unhandled_amount -= lot_amount;
			unhandled_target -= lot_target;
		}

		let _ = T::Currency::deposit(currency_id, &Self::account_id(), amount);
		<TotalCollateralInAuction<T>>::mutate(currency_id, |balance| *balance += amount);
		<BadDebtPool<T>>::mutate(|debt| *debt += bad_debt);
	}
}
