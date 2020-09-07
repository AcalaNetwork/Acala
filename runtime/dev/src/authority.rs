//! An orml_authority trait implementation.

use crate::{
	AcalaTreasuryModuleId, AccountId, AccountIdConversion, AuthoritysOriginId, BadOrigin, BlockNumber, DSWFModuleId,
	DispatchResult, EnsureRootOrHalfGeneralCouncil, EnsureRootOrHalfHomaCouncil, EnsureRootOrHalfHonzonCouncil,
	EnsureRootOrOneThirdsTechnicalCommittee, EnsureRootOrThreeFourthsGeneralCouncil,
	EnsureRootOrTwoThirdsTechnicalCommittee, HomaTreasuryModuleId, HonzonTreasuryModuleId, OneDay, Origin,
	OriginCaller, Runtime, SevenDays, System, HOURS,
};
pub use frame_support::traits::{schedule::Priority, EnsureOrigin, OriginTrait};
use frame_system::ensure_root;
use orml_authority::{DelayedOrigin, EnsureDelayed};
use sp_std::boxed::Box;

//impl Into<Result<DelayedOrigin<BlockNumber, Origin>, Origin>> for Origin {
//	fn into(self) -> Result<DelayedOrigin<BlockNumber, Origin>, Origin> {
//		Ok(DelayedOrigin {
//			delay: System::block_number(),
//			origin: Box::new(self),
//		})
//	}
//}
//
//impl From<DelayedOrigin<BlockNumber, Origin>> for Origin {
//	fn from(delayed_origin: DelayedOrigin<BlockNumber, Origin>) -> Origin {
//		*delayed_origin.origin
//	}
//}

pub struct AuthorityConfigImpl;
impl orml_authority::AuthorityConfig<Origin, OriginCaller, BlockNumber> for AuthorityConfigImpl {
	fn check_schedule_dispatch(origin: Origin, _priority: Priority) -> DispatchResult {
		let origin: Result<frame_system::RawOrigin<AccountId>, _> = origin.into();
		match origin {
			Ok(frame_system::RawOrigin::Root) => Ok(()),
			Ok(frame_system::RawOrigin::Signed(caller)) => {
				if caller == AcalaTreasuryModuleId::get().into_account()
					|| caller == HonzonTreasuryModuleId::get().into_account()
					|| caller == HomaTreasuryModuleId::get().into_account()
					|| caller == DSWFModuleId::get().into_account()
				{
					Ok(())
				} else {
					Err(BadOrigin.into())
				}
			}
			_ => Err(BadOrigin.into()),
		}
	}

	fn check_fast_track_schedule(
		origin: Origin,
		_initial_origin: &OriginCaller,
		new_delay: BlockNumber,
	) -> DispatchResult {
		ensure_root(origin.clone()).or_else(|_| {
			if new_delay / HOURS < 12 {
				EnsureRootOrTwoThirdsTechnicalCommittee::ensure_origin(origin)
					.map_or_else(|e| Err(e.into()), |_| Ok(()))
			} else {
				EnsureRootOrOneThirdsTechnicalCommittee::ensure_origin(origin)
					.map_or_else(|e| Err(e.into()), |_| Ok(()))
			}
		})
	}

	fn check_delay_schedule(origin: Origin, _initial_origin: &OriginCaller) -> DispatchResult {
		ensure_root(origin.clone()).or_else(|_| {
			EnsureRootOrOneThirdsTechnicalCommittee::ensure_origin(origin).map_or_else(|e| Err(e.into()), |_| Ok(()))
		})
	}

	fn check_cancel_schedule(origin: Origin, initial_origin: &OriginCaller) -> DispatchResult {
		ensure_root(origin.clone()).or_else(|_| {
			if origin.caller() == initial_origin
				|| EnsureRootOrThreeFourthsGeneralCouncil::ensure_origin(origin).is_ok()
			{
				Ok(())
			} else {
				Err(BadOrigin.into())
			}
		})
	}
}

impl orml_authority::AsOriginId<Origin, OriginCaller> for AuthoritysOriginId {
	fn into_origin(self) -> OriginCaller {
		match self {
			AuthoritysOriginId::Root => Origin::root().caller().clone(),
			AuthoritysOriginId::AcalaTreasury => Origin::signed(AcalaTreasuryModuleId::get().into_account())
				.caller()
				.clone(),
			AuthoritysOriginId::HonzonTreasury => Origin::signed(HonzonTreasuryModuleId::get().into_account())
				.caller()
				.clone(),
			AuthoritysOriginId::HomaTreasury => Origin::signed(HomaTreasuryModuleId::get().into_account())
				.caller()
				.clone(),
			AuthoritysOriginId::DSWF => Origin::signed(DSWFModuleId::get().into_account()).caller().clone(),
		}
	}

	fn check_dispatch_from(&self, origin: Origin) -> DispatchResult {
		ensure_root(origin.clone()).or_else(|_| {
			match self {
				_ => Ok(()),
				AuthoritysOriginId::Root => {
					<EnsureDelayed::<SevenDays, EnsureRootOrThreeFourthsGeneralCouncil, BlockNumber, OriginCaller> as EnsureOrigin
					<Origin>>::ensure_origin(
						origin
					)
					.map_or_else(|_| Err(BadOrigin.into()), |_| Ok(()))
				}
				//AuthoritysOriginId::AcalaTreasury => {
				//	EnsureDelayed::<OneDay, EnsureRootOrHalfGeneralCouncil, BlockNumber>::ensure_origin(origin)
				//		.map_or_else(|_| Err(BadOrigin.into()), |_| Ok(()))
				//}
				//AuthoritysOriginId::HonzonTreasury => {
				//	EnsureDelayed::<OneDay, EnsureRootOrHalfHonzonCouncil, BlockNumber>::ensure_origin(origin)
				//		.map_or_else(|_| Err(BadOrigin.into()), |_| Ok(()))
				//}
				//AuthoritysOriginId::HomaTreasury => {
				//	EnsureDelayed::<OneDay, EnsureRootOrHalfHomaCouncil, BlockNumber>::ensure_origin(origin)
				//		.map_or_else(|_| Err(BadOrigin.into()), |_| Ok(()))
				//}
				//AuthoritysOriginId::DSWF => ensure_root(origin).map_or_else(|_| Err(BadOrigin.into()), |_| Ok(())),
			}
		})
	}
}
