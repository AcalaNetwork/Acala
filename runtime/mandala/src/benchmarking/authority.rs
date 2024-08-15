// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::{AccountId, Authority, AuthoritysOriginId, BlockNumber, Runtime, RuntimeCall, RuntimeOrigin, System};

use parity_scale_codec::Encode;
use sp_runtime::traits::Hash;
use sp_std::prelude::*;

use frame_support::{
	dispatch::GetDispatchInfo,
	traits::{schedule::DispatchTime, Bounded, OriginTrait},
};
use frame_system::RawOrigin;
use orml_benchmarking::{runtime_benchmarks, whitelisted_caller};

fn runtime_call() -> Box<RuntimeCall> {
	let call = RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
	Box::new(call)
}

fn bounded_call(call: RuntimeCall) -> Box<Bounded<RuntimeCall, <Runtime as frame_system::Config>::Hashing>> {
	let encoded_call = call.encode();
	Box::new(Bounded::Inline(encoded_call.try_into().unwrap()))
}

runtime_benchmarks! {
	{ Runtime, orml_authority }

	// dispatch a dispatchable as other origin
	dispatch_as {
	}: _(RawOrigin::Root, AuthoritysOriginId::Root, runtime_call())

	// schdule a dispatchable to be dispatched at later block.
	schedule_dispatch_without_delay {
		let call = RuntimeCall::Authority(orml_authority::Call::dispatch_as {
			as_origin: AuthoritysOriginId::Root,
			call: runtime_call(),
		});
	}: schedule_dispatch(RawOrigin::Root, DispatchTime::At(2), 0, false, bounded_call(call))

	// schdule a dispatchable to be dispatched at later block.
	// ensure that the delay is reached when scheduling
	schedule_dispatch_with_delay {
		let call = RuntimeCall::Authority(orml_authority::Call::dispatch_as {
			as_origin: AuthoritysOriginId::Root,
			call: runtime_call(),
		});
	}: schedule_dispatch(RawOrigin::Root, DispatchTime::At(2), 0, true, bounded_call(call))

	// fast track a scheduled dispatchable.
	fast_track_scheduled_dispatch {
		let call = RuntimeCall::Authority(orml_authority::Call::dispatch_as {
			as_origin: AuthoritysOriginId::Root,
			call: runtime_call(),
		});
		System::set_block_number(1u32);
		Authority::schedule_dispatch(
			RuntimeOrigin::root(),
			DispatchTime::At(2),
			0,
			true,
			bounded_call(call)
		)?;
		let schedule_origin = {
			let origin: <Runtime as frame_system::Config>::RuntimeOrigin = From::from(RuntimeOrigin::root());
			let origin: <Runtime as frame_system::Config>::RuntimeOrigin =
				From::from(orml_authority::DelayedOrigin::<BlockNumber, <Runtime as orml_authority::Config>::PalletsOrigin>::new(
					1,
					Box::new(origin.caller().clone()),
				));
			origin
		};

		let pallets_origin = schedule_origin.caller().clone();
	}: fast_track_scheduled_dispatch(RawOrigin::Root, Box::new(pallets_origin), 0, DispatchTime::At(4))

	// delay a scheduled dispatchable.
	delay_scheduled_dispatch {
		let call = RuntimeCall::Authority(orml_authority::Call::dispatch_as {
			as_origin: AuthoritysOriginId::Root,
			call: runtime_call(),
		});
		System::set_block_number(1u32);
		Authority::schedule_dispatch(
			RuntimeOrigin::root(),
			DispatchTime::At(2),
			0,
			true,
			bounded_call(call)
		)?;
		let schedule_origin = {
			let origin: <Runtime as frame_system::Config>::RuntimeOrigin = From::from(RuntimeOrigin::root());
			let origin: <Runtime as frame_system::Config>::RuntimeOrigin =
				From::from(orml_authority::DelayedOrigin::<BlockNumber, <Runtime as orml_authority::Config>::PalletsOrigin>::new(
					1,
					Box::new(origin.caller().clone()),
				));
			origin
		};

		let pallets_origin = schedule_origin.caller().clone();
	}: _(RawOrigin::Root, Box::new(pallets_origin), 0, 5)

	// cancel a scheduled dispatchable
	cancel_scheduled_dispatch {
		let call = RuntimeCall::Authority(orml_authority::Call::dispatch_as {
			as_origin: AuthoritysOriginId::Root,
			call: runtime_call(),
		});
		System::set_block_number(1u32);
		Authority::schedule_dispatch(
			RuntimeOrigin::root(),
			DispatchTime::At(2),
			0,
			true,
			bounded_call(call)
		)?;
		let schedule_origin = {
			let origin: <Runtime as frame_system::Config>::RuntimeOrigin = From::from(RuntimeOrigin::root());
			let origin: <Runtime as frame_system::Config>::RuntimeOrigin =
				From::from(orml_authority::DelayedOrigin::<BlockNumber, <Runtime as orml_authority::Config>::PalletsOrigin>::new(
					1,
					Box::new(origin.caller().clone()),
				));
			origin
		};

		let pallets_origin = schedule_origin.caller().clone();
	}: _(RawOrigin::Root, Box::new(pallets_origin), 0)

	// authorize a call that can be triggered later
	authorize_call {
		let caller: AccountId = whitelisted_caller();
		let call = RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
		let hash = <Runtime as frame_system::Config>::Hashing::hash_of(&call);
		System::set_block_number(1u32);
	}: _(RawOrigin::Root, Box::new(call.clone()), Some(caller.clone()))
	verify {
		assert_eq!(Authority::saved_calls(&hash), Some((call, Some(caller))));
	}

	remove_authorized_call {
		let caller: AccountId = whitelisted_caller();
		let call = RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
		let hash = <Runtime as frame_system::Config>::Hashing::hash_of(&call);
		System::set_block_number(1u32);
		Authority::authorize_call(RuntimeOrigin::root(), Box::new(call.clone()), Some(caller.clone()))?;
	}: _(RawOrigin::Signed(caller), hash)
	verify {
		assert_eq!(Authority::saved_calls(&hash), None);
	}

	trigger_call {
		let caller: AccountId = whitelisted_caller();
		let call = RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
		let hash = <Runtime as frame_system::Config>::Hashing::hash_of(&call);
		let call_weight_bound = call.get_dispatch_info().weight;
		System::set_block_number(1u32);
		Authority::authorize_call(RuntimeOrigin::root(), Box::new(call.clone()), Some(caller.clone()))?;
	}: _(RawOrigin::Signed(caller), hash, call_weight_bound)
	verify {
		assert_eq!(Authority::saved_calls(&hash), None);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
