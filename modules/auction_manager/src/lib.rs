#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, FullCodec, HasCompact};
use frame_support::{decl_event, decl_module, decl_storage, traits::Get, Parameter};
use orml_traits::{
	arithmetic::{self, Signed},
	Auction, AuctionHandler, MultiCurrency, MultiCurrencyExtended, OnNewBidResult,
};
use rstd::{
	convert::{TryFrom, TryInto},
	fmt::Debug,
};
use sr_primitives::{
	traits::{
		AccountIdConversion, CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, MaybeSerializeDeserialize, Member,
		SimpleArithmetic,
	},
	ModuleId, RuntimeDebug,
};

use support::{AuctionManager, Rate};

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
	type MinimumIncrementSize: Get<Rate>;
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
				if T::Currency::withdraw(T::GetStableCurrencyId::get(), &Self::account_id(), amount).is_ok() {
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

	/// Check `new_price` is larger than minimum increment
	/// Formula: bid_price - last_price >= max(last_price, target) * minimum_increment_size
	pub fn check_minimum_increment(
		new_price: &T::Balance,
		last_price: &T::Balance,
		target_price: &T::Balance,
		minimum_increment: &Rate,
	) -> bool {
		if let (Some(target), Some(result)) = (
			minimum_increment.checked_mul_int(std::cmp::max(target_price, last_price)),
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
			let payment = std::cmp::min(auction_item.target, new_bid.1);

			// check new price is larger than minimum increment
			// check new bidder has enough stable coin
			if Self::check_minimum_increment(&new_bid.1, &last_price, &auction_item.target, &minimum_increment_size)
				&& T::Currency::balance(stable_currency_id, &(new_bid.0)) >= payment
				&& Self::surplus_pool().checked_add(&payment).is_some()
			{
				let module_account = Self::account_id();

				// first: deduct amount of stablecoin from new bidder, add this to auction manager module
				T::Currency::transfer(stable_currency_id, &(new_bid.0), &module_account, payment)
					.expect("never failed after balance check");
				<SurplusPool<T>>::mutate(|surplus| *surplus += payment);

				// second: if these's bid before, return stablecoin from auction manager module to last bidder
				if let Some((last_bidder, last_price)) = last_bid {
					let refund = std::cmp::min(last_price, auction_item.target);

					T::Currency::transfer(stable_currency_id, &module_account, &last_bidder, refund)
						.expect("never failed because payment >= refund");
					<SurplusPool<T>>::mutate(|surplus| *surplus -= refund);
				}

				// third: if bid_price > target, the auction is in reverse, refund collateral to it's origin from auction manager module
				if new_bid.1 > auction_item.target {
					let new_amount = auction_item
						.amount
						.checked_mul(std::cmp::max(&last_price, &auction_item.target))
						.and_then(|n| n.checked_div(&new_bid.1))
						.unwrap_or(auction_item.amount);

					let deduct_amount = auction_item.amount.checked_sub(&new_amount).unwrap_or(0.into());

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
		if let Some(auction_item) = Self::auctions(id) {
			if let Some((bidder, _)) = winner {
				// these's bidder for this auction, transfer collateral to bidder
				let amount = std::cmp::min(
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
}

impl<T: Trait> AuctionManager<T::AccountId> for Module<T> {
	type CurrencyId = T::CurrencyId;
	type Balance = T::Balance;
	type Amount = T::Amount;

	fn increase_surplus(increment: Self::Balance) {
		if Self::surplus_pool().checked_add(&increment).is_some()
			&& T::Currency::balance(T::GetStableCurrencyId::get(), &Self::account_id())
				.checked_add(&increment)
				.is_some()
		{
			T::Currency::deposit(T::GetStableCurrencyId::get(), &Self::account_id(), increment)
				.expect("never failed after overflow check");
			<SurplusPool<T>>::mutate(|surplus| *surplus += increment);
		}
	}

	fn new_collateral_auction(
		who: T::AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
		target: Self::Balance,
		bad_debt: Self::Balance,
	) {
		if Self::total_collateral_in_auction(currency_id)
			.checked_add(&amount)
			.is_some() && T::Currency::balance(T::GetStableCurrencyId::get(), &Self::account_id())
			.checked_add(&amount)
			.is_some() && Self::bad_debt_pool().checked_add(&bad_debt).is_some()
		{
			T::Currency::deposit(currency_id, &Self::account_id(), amount).expect("never failed after overflow check");
			<TotalCollateralInAuction<T>>::mutate(currency_id, |balance| *balance += amount);
			<BadDebtPool<T>>::mutate(|debt| *debt += bad_debt);

			let maximum_auction_size = <Module<T>>::maximum_auction_size(currency_id);
			let mut unhandled_amount: Self::Balance = amount;
			let mut unhandled_target: Self::Balance = target;
			let block_number = <system::Module<T>>::block_number();

			while unhandled_amount > 0.into() {
				let (lot_amount, lot_target) =
					if unhandled_amount > maximum_auction_size && maximum_auction_size != 0.into() {
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
