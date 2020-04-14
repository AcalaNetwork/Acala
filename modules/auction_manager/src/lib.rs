#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, HasCompact};
use frame_support::{
	debug, decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get, IsSubType, IterableStorageMap,
};
use orml_traits::{Auction, AuctionHandler, MultiCurrency, OnNewBidResult};
use rstd::{
	cmp::{Eq, PartialEq},
	prelude::*,
};
use sp_runtime::{
	traits::{BlakeTwo256, CheckedAdd, CheckedSub, Hash, Saturating, Zero},
	transaction_validity::{InvalidTransaction, TransactionPriority, TransactionValidity, ValidTransaction},
	DispatchResult, RandomNumberGenerator, RuntimeDebug,
};
use support::{AuctionManager, CDPTreasury, OnEmergencyShutdown, PriceProvider, Rate};
use system::{ensure_none, offchain::SubmitUnsignedTransaction};
use utilities::{OffchainErr, OffchainLock};

mod mock;
mod tests;

const DB_PREFIX: &[u8] = b"acala/auction-manager-offchain-worker/";

#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct CollateralAuctionItem<AccountId, CurrencyId, Balance: HasCompact, BlockNumber> {
	owner: AccountId,
	currency_id: CurrencyId,
	#[codec(compact)]
	amount: Balance,
	#[codec(compact)]
	target: Balance,
	start_time: BlockNumber,
}

#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct DebitAuctionItem<Balance: HasCompact, BlockNumber> {
	#[codec(compact)]
	amount: Balance,
	#[codec(compact)]
	fix: Balance,
	start_time: BlockNumber,
}

#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct SurplusAuctionItem<Balance: HasCompact, BlockNumber> {
	#[codec(compact)]
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
	type CDPTreasury: CDPTreasury<Self::AccountId, Balance = BalanceOf<Self>, CurrencyId = CurrencyIdOf<Self>>;
	type PriceSource: PriceProvider<CurrencyIdOf<Self>>;

	/// A dispatchable call type.
	type Call: From<Call<Self>> + IsSubType<Module<Self>, Self>;

	/// A transaction submitter.
	type SubmitTransaction: SubmitUnsignedTransaction<Self, <Self as Trait>::Call>;
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
		AuctionDealed(AuctionId),
	}
);

decl_error! {
	/// Error for auction manager module.
	pub enum Error for Module<T: Trait> {
		AuctionNotExsits,
		InReservedStage,
		BalanceNotEnough,
		InvalidFeedPrice,
		MustAfterShutdown,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as AuctionManager {
		pub CollateralAuctions get(fn collateral_auctions): map hasher(twox_64_concat) AuctionIdOf<T> =>
			Option<CollateralAuctionItem<T::AccountId, CurrencyIdOf<T>, BalanceOf<T>, T::BlockNumber>>;
		pub DebitAuctions get(fn debit_auctions): map hasher(twox_64_concat) AuctionIdOf<T> =>
			Option<DebitAuctionItem<BalanceOf<T>, T::BlockNumber>>;
		pub SurplusAuctions get(fn surplus_auctions): map hasher(twox_64_concat) AuctionIdOf<T> =>
			Option<SurplusAuctionItem<BalanceOf<T>, T::BlockNumber>>;
		pub TotalCollateralInAuction get(fn total_collateral_in_auction): map hasher(twox_64_concat) CurrencyIdOf<T> => BalanceOf<T>;
		pub TotalTargetInAuction get(fn total_target_in_auction): BalanceOf<T>;
		pub TotalDebitInAuction get(fn total_debit_in_auction): BalanceOf<T>;
		pub TotalSurplusInAuction get(fn total_surplus_in_auction): BalanceOf<T>;
		pub IsShutdown get(fn is_shutdown): bool;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		const MinimumIncrementSize: Rate = T::MinimumIncrementSize::get();
		const AuctionTimeToClose: T::BlockNumber = T::AuctionTimeToClose::get();
		const AuctionDurationSoftCap: T::BlockNumber = T::AuctionDurationSoftCap::get();
		const GetStableCurrencyId: CurrencyIdOf<T> = T::GetStableCurrencyId::get();
		const GetNativeCurrencyId: CurrencyIdOf<T> = T::GetNativeCurrencyId::get();
		const GetAmountAdjustment: Rate = T::GetAmountAdjustment::get();

		pub fn cancel(origin, id: AuctionIdOf<T>) {
			ensure_none(origin)?;
			ensure!(Self::is_shutdown(), Error::<T>::MustAfterShutdown);
			<Module<T> as AuctionManager<T::AccountId>>::cancel_auction(id)?;
		}

		// Runs after every block.
		fn offchain_worker(now: T::BlockNumber) {
			if Self::is_shutdown() && runtime_io::offchain::is_validator() {
				if let Err(e) = Self::_offchain_worker(now) {
					debug::info!(
						target: "auction-manager offchain worker",
						"cannot run offchain worker at {:?}: {:?}",
						now,
						e,
					);
				}
			}
		}
	}
}

impl<T: Trait> Module<T> {
	fn submit_cancel_auction_tx(auction_id: AuctionIdOf<T>) {
		let call = Call::<T>::cancel(auction_id);
		if !T::SubmitTransaction::submit_unsigned(call).is_ok() {
			debug::warn!(
				target: "auction-manager offchain worker",
				"submit unsigned auction cancel tx for \nAuctionId {:?} failed : {:?}",
				auction_id, OffchainErr::SubmitTransaction,
			);
		} else {
			debug::debug!(
				target: "auction-manager offchain worker",
				"successfully submit unsigned auction cancel tx for \nAuctionId {:?}",
				auction_id,
			);
		}
	}

	fn _offchain_worker(now: T::BlockNumber) -> Result<(), OffchainErr> {
		// Acquire offchain worker lock.
		// If succeeded, update the lock, otherwise return error
		let offchain_lock = OffchainLock::new(DB_PREFIX.to_vec());
		offchain_lock.acquire_offchain_lock(|_: Option<()>| ())?;

		let random_seed = runtime_io::offchain::random_seed();
		let mut rng = RandomNumberGenerator::<BlakeTwo256>::new(BlakeTwo256::hash(&random_seed[..]));
		match rng.pick_u32(2) {
			0 => {
				for (auction_id, _) in <DebitAuctions<T>>::iter() {
					Self::submit_cancel_auction_tx(auction_id);
					offchain_lock.extend_offchain_lock_if_needed::<()>();
				}
			}
			1 => {
				for (auction_id, _) in <SurplusAuctions<T>>::iter() {
					Self::submit_cancel_auction_tx(auction_id);
					offchain_lock.extend_offchain_lock_if_needed::<()>();
				}
			}
			_ => {
				for (auction_id, _) in <CollateralAuctions<T>>::iter() {
					if !Self::collateral_auction_in_reverse_stage(auction_id) {
						Self::submit_cancel_auction_tx(auction_id);
					}
					offchain_lock.extend_offchain_lock_if_needed::<()>();
				}
			}
		}

		// finally, reset the expire timestamp to now in order to release lock in advance.
		offchain_lock.release_offchain_lock(|_: ()| true);
		debug::debug!(
			target: "auction-manager offchain worker",
			"offchain worker start at block: {:?} already done!",
			now,
		);

		Ok(())
	}

	pub fn emergency_shutdown() {
		<IsShutdown>::put(true);
	}

	pub fn cancel_surplus_auction(id: AuctionIdOf<T>) -> DispatchResult {
		let surplus_auction = <SurplusAuctions<T>>::take(id).ok_or(Error::<T>::AuctionNotExsits)?;
		if let Some(auction_info) = T::Auction::auction_info(id) {
			// if these's bid, refund native token to the bidder
			if let Some((bidder, bid_price)) = auction_info.bid {
				let native_currency_id = T::GetNativeCurrencyId::get();
				if T::Currency::free_balance(native_currency_id, &bidder)
					.checked_add(&bid_price)
					.is_some()
				{
					// TODO: transfer from RESERVED TREASURY instead of mint
					T::Currency::deposit(native_currency_id, &bidder, bid_price)
						.expect("never failed after overflow check");
				}

				// decrease account ref of bidder
				system::Module::<T>::dec_ref(&bidder);
			}
		}

		// decrease total surplus in auction
		<TotalSurplusInAuction<T>>::mutate(|balance| *balance = balance.saturating_sub(surplus_auction.amount));

		// remove the auction info in auction module
		T::Auction::remove_auction(id);

		<Module<T>>::deposit_event(RawEvent::CancelAuction(id));
		Ok(())
	}

	pub fn cancel_debit_auction(id: AuctionIdOf<T>) -> DispatchResult {
		let debit_auction = <DebitAuctions<T>>::take(id).ok_or(Error::<T>::AuctionNotExsits)?;
		if let Some(auction_info) = T::Auction::auction_info(id) {
			// if these's bid, refund stable token to the bidder
			if let Some((bidder, _)) = auction_info.bid {
				if T::CDPTreasury::on_system_debit(debit_auction.fix).is_ok() {
					// return stablecoin to bidder, ignore Err
					let _ = T::CDPTreasury::deposit_backed_debit(&bidder, debit_auction.fix);
				}

				// decrease account ref of bidder
				system::Module::<T>::dec_ref(&bidder);
			}
		}

		// derease total debit in auction
		<TotalDebitInAuction<T>>::mutate(|balance| *balance = balance.saturating_sub(debit_auction.fix));

		// remove the auction info in auction module
		T::Auction::remove_auction(id);

		<Module<T>>::deposit_event(RawEvent::CancelAuction(id));
		Ok(())
	}

	pub fn cancel_collateral_auction(id: AuctionIdOf<T>) -> DispatchResult {
		let collateral_auction = Self::collateral_auctions(id).ok_or(Error::<T>::AuctionNotExsits)?;
		// must not in reverse bid stage
		ensure!(
			!Self::collateral_auction_in_reverse_stage(id),
			Error::<T>::InReservedStage
		);

		// calculate which amount of collateral to offset target
		// in settle price
		let stable_currency_id = T::GetStableCurrencyId::get();
		let settle_price = T::PriceSource::get_relative_price(stable_currency_id, collateral_auction.currency_id)
			.ok_or(Error::<T>::InvalidFeedPrice)?;
		let confiscate_collateral_amount = rstd::cmp::min(
			settle_price.saturating_mul_int(&collateral_auction.target),
			collateral_auction.amount,
		);
		let refund_collateral_amount = collateral_auction.amount.saturating_sub(confiscate_collateral_amount);

		// refund remain collateral from cdp treasury to auction owner
		if !refund_collateral_amount.is_zero() {
			T::CDPTreasury::transfer_system_collateral(
				collateral_auction.currency_id,
				&collateral_auction.owner,
				refund_collateral_amount,
			)?;
		}

		if let Some(auction_info) = T::Auction::auction_info(id) {
			// if these's bid, refund stable token to the bidder
			if let Some((bidder, bid_price)) = auction_info.bid {
				if T::CDPTreasury::on_system_debit(bid_price).is_ok() {
					// return stablecoin to bidder, ignore Err
					let _ = T::CDPTreasury::deposit_backed_debit(&bidder, bid_price);
				}

				// decrease account ref of bidder
				system::Module::<T>::dec_ref(&bidder);
			}
		}

		// decrease account ref of owner
		system::Module::<T>::dec_ref(&collateral_auction.owner);

		// decrease total collateral and target in auction
		<TotalCollateralInAuction<T>>::mutate(collateral_auction.currency_id, |balance| {
			*balance = balance.saturating_sub(collateral_auction.amount)
		});
		<TotalTargetInAuction<T>>::mutate(|balance| *balance = balance.saturating_sub(collateral_auction.target));

		// remove collateral auction
		<CollateralAuctions<T>>::remove(id);
		T::Auction::remove_auction(id);

		<Module<T>>::deposit_event(RawEvent::CancelAuction(id));
		Ok(())
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
			let mut payment = rstd::cmp::min(collateral_auction.target, new_bid.1);

			// check new price is larger than minimum increment and new bidder has enough stable coin
			if Self::check_minimum_increment(
				new_bid.1,
				last_price,
				collateral_auction.target,
				Self::get_minimum_increment_size(now, collateral_auction.start_time),
			) && T::Currency::ensure_can_withdraw(stable_currency_id, &new_bid.0, payment).is_ok()
			{
				// increase account ref of new bidder
				system::Module::<T>::inc_ref(&new_bid.0);

				// if these's bid before, return stablecoin from new bidder to last bidder
				if let Some((last_bidder, last_price)) = last_bid {
					let refund = rstd::cmp::min(last_price, collateral_auction.target);
					T::Currency::transfer(stable_currency_id, &new_bid.0, &last_bidder, refund)
						.expect("never failed after balance check");

					// decrease account ref of last bidder
					system::Module::<T>::dec_ref(&last_bidder);

					payment -= refund;
				}

				if !payment.is_zero() {
					// transfer stablecoin from new bidder to cdp treasury
					T::CDPTreasury::transfer_surplus_from(&new_bid.0, payment)
						.expect("never failed after balance check");
				}

				// if bid_price > target, the auction is in reverse, refund collateral to it's origin from auction cdp treasury
				if new_bid.1 > collateral_auction.target {
					let new_collateral_amount =
						Rate::from_rational(rstd::cmp::max(last_price, collateral_auction.target), new_bid.1)
							.checked_mul_int(&collateral_auction.amount)
							.unwrap_or(collateral_auction.amount);
					let deduct_collateral_amount = collateral_auction.amount.saturating_sub(new_collateral_amount);

					if T::CDPTreasury::transfer_system_collateral(
						collateral_auction.currency_id,
						&(collateral_auction.owner),
						deduct_collateral_amount,
					)
					.is_ok()
					{
						// update collateral auction when refund collateral to owner success
						<TotalCollateralInAuction<T>>::mutate(collateral_auction.currency_id, |balance| {
							*balance = balance.saturating_sub(deduct_collateral_amount)
						});
						collateral_auction.amount = new_collateral_amount;
						<CollateralAuctions<T>>::insert(id, collateral_auction.clone());
					}
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

					// decrease account ref of last bidder
					system::Module::<T>::dec_ref(&last_bidder);
				} else {
					// these's no bid before, transfer stablecoin to cdp treasury
					T::CDPTreasury::transfer_surplus_from(&new_bid.0, debit_auction.fix)
						.expect("never failed after balance check");
				}

				// increase account ref of bidder
				system::Module::<T>::inc_ref(&new_bid.0);

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
			) && T::Currency::ensure_can_withdraw(native_currency_id, &new_bid.0, new_bid.1).is_ok()
				&& !new_bid.1.is_zero()
			{
				let mut burn_native_currency_amount = new_bid.1;

				// if these's bid before, transfer the stablecoin from auction manager module to last bidder
				if let Some((last_bidder, last_price)) = last_bid {
					burn_native_currency_amount = burn_native_currency_amount.saturating_sub(last_price);
					T::Currency::transfer(native_currency_id, &new_bid.0, &last_bidder, last_price)
						.expect("never failed after balance check");

					// decrease account ref of last bidder
					system::Module::<T>::dec_ref(&last_bidder);
				}

				// burn remain native token from new bidder
				T::Currency::withdraw(native_currency_id, &new_bid.0, burn_native_currency_amount)
					.expect("never failed after balance check");

				// increase account ref of bidder
				system::Module::<T>::inc_ref(&new_bid.0);

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

			T::CDPTreasury::transfer_system_collateral(collateral_auction.currency_id, &bidder, amount)
				.expect("never failed after overflow check");

			// decrease account ref of winner
			system::Module::<T>::dec_ref(&bidder);

			// decrease account ref of collateral auction owner
			system::Module::<T>::dec_ref(&collateral_auction.owner);

			<TotalCollateralInAuction<T>>::mutate(collateral_auction.currency_id, |balance| {
				*balance = balance.saturating_sub(amount)
			});
			<TotalTargetInAuction<T>>::mutate(|balance| *balance = balance.saturating_sub(collateral_auction.target));
			<CollateralAuctions<T>>::remove(id);

			<Module<T>>::deposit_event(RawEvent::AuctionDealed(id));
		}
	}

	pub fn debit_auction_end_handler(id: AuctionIdOf<T>, winner: Option<(T::AccountId, BalanceOf<T>)>) {
		if let Some(debit_auction) = Self::debit_auctions(id) {
			if let Some((bidder, _)) = winner {
				// issue the amount of native token to winner
				if T::Currency::free_balance(T::GetNativeCurrencyId::get(), &bidder)
					.checked_add(&debit_auction.amount)
					.is_some()
				{
					// TODO: transfer from RESERVED TREASURY instead of mint
					T::Currency::deposit(T::GetNativeCurrencyId::get(), &bidder, debit_auction.amount)
						.expect("never failed after overflow check");
				}

				// decrease account ref of winner
				system::Module::<T>::dec_ref(&bidder);

				// decrease debit in auction and delete auction
				<TotalDebitInAuction<T>>::mutate(|balance| *balance = balance.saturating_sub(debit_auction.fix));
				<DebitAuctions<T>>::remove(id);

				<Module<T>>::deposit_event(RawEvent::AuctionDealed(id));
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

				<Module<T>>::deposit_event(RawEvent::CancelAuction(id));
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
			// transfer stable token from cdp treasury to winner, ignore Err
			let _ = T::CDPTreasury::transfer_system_surplus(&bidder, surplus_auction.amount);

			// decrease account ref of winner
			system::Module::<T>::dec_ref(&bidder);

			// decrease surplus in auction
			<TotalSurplusInAuction<T>>::mutate(|balance| *balance = balance.saturating_sub(surplus_auction.amount));
			<SurplusAuctions<T>>::remove(id);

			<Module<T>>::deposit_event(RawEvent::AuctionDealed(id));
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
		if <CollateralAuctions<T>>::contains_key(id) {
			Self::collateral_auction_bid_handler(now, id, new_bid, last_bid)
		} else if <DebitAuctions<T>>::contains_key(id) {
			Self::debit_auction_bid_handler(now, id, new_bid, last_bid)
		} else if <SurplusAuctions<T>>::contains_key(id) {
			Self::surplus_auction_bid_handler(now, id, new_bid, last_bid)
		} else {
			OnNewBidResult {
				accept_bid: false,
				auction_end: None,
			}
		}
	}

	fn on_auction_ended(id: AuctionIdOf<T>, winner: Option<(T::AccountId, BalanceOf<T>)>) {
		if <CollateralAuctions<T>>::contains_key(id) {
			Self::collateral_auction_end_handler(id, winner)
		} else if <DebitAuctions<T>>::contains_key(id) {
			Self::debit_auction_end_handler(id, winner)
		} else if <SurplusAuctions<T>>::contains_key(id) {
			Self::surplus_auction_end_handler(id, winner)
		}
	}
}

impl<T: Trait> AuctionManager<T::AccountId> for Module<T> {
	type CurrencyId = CurrencyIdOf<T>;
	type Balance = BalanceOf<T>;
	type AuctionId = AuctionIdOf<T>;

	fn new_collateral_auction(
		who: &T::AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
		target: Self::Balance,
	) {
		if Self::total_collateral_in_auction(currency_id)
			.checked_add(&amount)
			.is_some() && Self::total_target_in_auction().checked_add(&target).is_some()
		{
			<TotalCollateralInAuction<T>>::mutate(currency_id, |balance| *balance += amount);
			<TotalTargetInAuction<T>>::mutate(|balance| *balance += target);

			let block_number = <system::Module<T>>::block_number();
			let auction_id: AuctionIdOf<T> = T::Auction::new_auction(block_number, None); // do not set endtime for collateral auction
			let collateral_aution = CollateralAuctionItem {
				owner: who.clone(),
				currency_id: currency_id,
				amount: amount,
				target: target,
				start_time: block_number,
			};

			// decrease account ref of owner(remain receiver)
			system::Module::<T>::inc_ref(&who);

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
			let auction_id: AuctionIdOf<T> = T::Auction::new_auction(start_block, Some(end_block)); // set endtime for debit auction!
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
		if Self::total_surplus_in_auction().checked_add(&amount).is_some() {
			<TotalSurplusInAuction<T>>::mutate(|balance| *balance += amount);
			let auction_id: AuctionIdOf<T> = T::Auction::new_auction(<system::Module<T>>::block_number(), None); // do not set endtime for surplus auction
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

	fn get_total_collateral_in_auction(id: Self::CurrencyId) -> Self::Balance {
		Self::total_collateral_in_auction(id)
	}

	fn get_total_surplus_in_auction() -> Self::Balance {
		Self::total_surplus_in_auction()
	}

	fn cancel_auction(id: Self::AuctionId) -> DispatchResult {
		if <CollateralAuctions<T>>::contains_key(id) {
			Self::cancel_collateral_auction(id)
		} else if <DebitAuctions<T>>::contains_key(id) {
			Self::cancel_debit_auction(id)
		} else if <SurplusAuctions<T>>::contains_key(id) {
			Self::cancel_surplus_auction(id)
		} else {
			Err(Error::<T>::AuctionNotExsits.into())
		}
	}
}

impl<T: Trait> OnEmergencyShutdown for Module<T> {
	fn on_emergency_shutdown() {
		Self::emergency_shutdown();
	}
}

#[allow(deprecated)]
impl<T: Trait> frame_support::unsigned::ValidateUnsigned for Module<T> {
	type Call = Call<T>;

	fn validate_unsigned(call: &Self::Call) -> TransactionValidity {
		if let Call::cancel(auction_id) = call {
			if !Self::is_shutdown() {
				return InvalidTransaction::Stale.into();
			} else if <CollateralAuctions<T>>::contains_key(auction_id) {
				if !Self::collateral_auction_in_reverse_stage(*auction_id) {
					return InvalidTransaction::Stale.into();
				}
			} else if !<SurplusAuctions<T>>::contains_key(auction_id) && !<DebitAuctions<T>>::contains_key(auction_id) {
				return InvalidTransaction::Stale.into();
			}

			Ok(ValidTransaction {
				priority: TransactionPriority::max_value(),
				requires: vec![],
				provides: vec![("AuctionManagerOffchain", auction_id).encode()],
				longevity: 64_u64,
				propagate: true,
			})
		} else {
			InvalidTransaction::Call.into()
		}
	}
}
