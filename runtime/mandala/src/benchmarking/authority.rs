use crate::{Authority, AuthoritysOriginId, BlockNumber, Call, Origin, Runtime, System};

use sp_runtime::Perbill;
use sp_std::prelude::*;

use frame_support::traits::{schedule::DispatchTime, OriginTrait};
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;

runtime_benchmarks! {
	{ Runtime, orml_authority }

	_ {}

	// dispatch a dispatchable as other origin
	dispatch_as {
		let ensure_root_call = Call::System(frame_system::Call::fill_block(Perbill::from_percent(1)));
	}: _(RawOrigin::Root, AuthoritysOriginId::Root, Box::new(ensure_root_call.clone()))

	// schdule a dispatchable to be dispatched at later block.
	schedule_dispatch_without_delay {
		let ensure_root_call = Call::System(frame_system::Call::fill_block(Perbill::from_percent(1)));
		let call = Call::Authority(orml_authority::Call::dispatch_as(
			AuthoritysOriginId::Root,
			Box::new(ensure_root_call.clone()),
		));
	}: schedule_dispatch(RawOrigin::Root, DispatchTime::At(2), 0, false, Box::new(call.clone()))

	// schdule a dispatchable to be dispatched at later block.
	// ensure that the delay is reached when scheduling
	schedule_dispatch_with_delay {
		let ensure_root_call = Call::System(frame_system::Call::fill_block(Perbill::from_percent(1)));
		let call = Call::Authority(orml_authority::Call::dispatch_as(
			AuthoritysOriginId::Root,
			Box::new(ensure_root_call.clone()),
		));
	}: schedule_dispatch(RawOrigin::Root, DispatchTime::At(2), 0, true, Box::new(call.clone()))

	// fast track a scheduled dispatchable.
	fast_track_scheduled_dispatch {
		let ensure_root_call = Call::System(frame_system::Call::fill_block(Perbill::from_percent(1)));
		let call = Call::Authority(orml_authority::Call::dispatch_as(
			AuthoritysOriginId::Root,
			Box::new(ensure_root_call.clone()),
		));
		System::set_block_number(1u32);
		Authority::schedule_dispatch(
			Origin::root(),
			DispatchTime::At(2),
			0,
			true,
			Box::new(call.clone())
		)?;
		let schedule_origin = {
			let origin: <Runtime as frame_system::Config>::Origin = From::from(Origin::root());
			let origin: <Runtime as frame_system::Config>::Origin =
				From::from(orml_authority::DelayedOrigin::<BlockNumber, <Runtime as orml_authority::Config>::PalletsOrigin> {
					delay: 1,
					origin: Box::new(origin.caller().clone()),
				});
			origin
		};

		let pallets_origin = schedule_origin.caller().clone();
	}: fast_track_scheduled_dispatch(RawOrigin::Root, pallets_origin, 0, DispatchTime::At(4))

	// delay a scheduled dispatchable.
	delay_scheduled_dispatch {
		let ensure_root_call = Call::System(frame_system::Call::fill_block(Perbill::from_percent(1)));
		let call = Call::Authority(orml_authority::Call::dispatch_as(
			AuthoritysOriginId::Root,
			Box::new(ensure_root_call.clone()),
		));
		System::set_block_number(1u32);
		Authority::schedule_dispatch(
			Origin::root(),
			DispatchTime::At(2),
			0,
			true,
			Box::new(call.clone())
		)?;
		let schedule_origin = {
			let origin: <Runtime as frame_system::Config>::Origin = From::from(Origin::root());
			let origin: <Runtime as frame_system::Config>::Origin =
				From::from(orml_authority::DelayedOrigin::<BlockNumber, <Runtime as orml_authority::Config>::PalletsOrigin> {
					delay: 1,
					origin: Box::new(origin.caller().clone()),
				});
			origin
		};

		let pallets_origin = schedule_origin.caller().clone();
	}: _(RawOrigin::Root, pallets_origin, 0, 5)

	// cancel a scheduled dispatchable
	cancel_scheduled_dispatch {
		let ensure_root_call = Call::System(frame_system::Call::fill_block(Perbill::from_percent(1)));
		let call = Call::Authority(orml_authority::Call::dispatch_as(
			AuthoritysOriginId::Root,
			Box::new(ensure_root_call.clone()),
		));
		System::set_block_number(1u32);
		Authority::schedule_dispatch(
			Origin::root(),
			DispatchTime::At(2),
			0,
			true,
			Box::new(call.clone())
		)?;
		let schedule_origin = {
			let origin: <Runtime as frame_system::Config>::Origin = From::from(Origin::root());
			let origin: <Runtime as frame_system::Config>::Origin =
				From::from(orml_authority::DelayedOrigin::<BlockNumber, <Runtime as orml_authority::Config>::PalletsOrigin> {
					delay: 1,
					origin: Box::new(origin.caller().clone()),
				});
			origin
		};

		let pallets_origin = schedule_origin.caller().clone();
	}: _(RawOrigin::Root, pallets_origin, 0)
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::assert_ok;

	fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap()
			.into()
	}

	#[test]
	fn test_dispatch_as() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_dispatch_as());
		});
	}

	#[test]
	fn test_scheduled_dispatch_without_delay() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_schedule_dispatch_without_delay());
		});
	}

	#[test]
	fn test_scheduled_dispatch_with_delay() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_schedule_dispatch_with_delay());
		});
	}

	#[test]
	fn test_cancel_scheduled_dispatch() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_cancel_scheduled_dispatch());
		});
	}
}
