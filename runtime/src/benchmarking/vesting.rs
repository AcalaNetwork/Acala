// use super::utils::lookup_of_account;
// use crate::{AccountId, CurrencyId, Runtime, Tokens};
//
// use sp_runtime::traits::SaturatedConversion;
// use sp_std::prelude::*;
//
// use frame_benchmarking::account;
// use frame_system::RawOrigin;
//
// use orml_benchmarking::runtime_benchmarks;
// use orml_vesting::VestingScheduleOf;
//
// type Schedule = VestingScheduleOf<Runtime>;
//
// const SEED: u32 = 0;
// const MAX_USER_INDEX: u32 = 1000;
//
// runtime_benchmarks! {
// 	{ Runtime, orml_vesting }
//
// 	_ {
// 		let u in 1 .. MAX_USER_INDEX => ();
// 	}
//
// 	add_vesting_schedule {
// 		let from = account("from", u, SEED);
// 	}
// }
