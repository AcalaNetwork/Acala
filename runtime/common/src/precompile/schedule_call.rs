use frame_support::debug;
use frame_support::{
	dispatch::Dispatchable,
	traits::{
		schedule::{DispatchTime, Named as ScheduleNamed, Priority},
		Currency, Get, IsType, OriginTrait, ReservableCurrency,
	},
};
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use primitives::{evm::AddressMapping as AddressMappingT, BlockNumber};
use sp_core::U256;
use sp_std::{borrow::Cow, fmt::Debug, marker::PhantomData, prelude::*, result};

use super::input::{Input, InputT, PER_PARAM_BYTES};
use codec::{Codec, Encode};
use frame_system::RawOrigin;
use primitives::{Balance, CurrencyId};
use sp_runtime::traits::Saturating;

/// The `ScheduleCall` impl precompile.
///
///
/// `input` data starts with `action`.
///
/// Actions:
/// - ScheduleCall. Rest `input` bytes: `from`, `target`, `value`, `gas_limit`,
///   `storage_limit`, `min_delay`, `input_len`, `input_data`.
pub struct ScheduleCallPrecompile<AccountId, AddressMapping, Scheduler, Call, Origin, PalletsOrigin, Runtime>(
	PhantomData<(
		AccountId,
		AddressMapping,
		Scheduler,
		Call,
		Origin,
		PalletsOrigin,
		Runtime,
	)>,
);

enum Action {
	ScheduleCall,
	Unknown,
}

impl From<u8> for Action {
	fn from(a: u8) -> Self {
		match a {
			0 => Action::ScheduleCall,
			_ => Action::Unknown,
		}
	}
}

impl<AccountId, AddressMapping, Scheduler, Call, Origin, PalletsOrigin, Runtime> Precompile
	for ScheduleCallPrecompile<AccountId, AddressMapping, Scheduler, Call, Origin, PalletsOrigin, Runtime>
where
	AccountId: Debug + Clone,
	AddressMapping: AddressMappingT<AccountId>,
	Scheduler: ScheduleNamed<BlockNumber, Call, PalletsOrigin>,
	Call: Dispatchable + Debug + From<module_evm::Call<Runtime>>,
	Origin: IsType<<Runtime as frame_system::Config>::Origin> + OriginTrait<PalletsOrigin = PalletsOrigin>,
	PalletsOrigin: Into<<Runtime as frame_system::Config>::Origin> + From<frame_system::RawOrigin<AccountId>> + Clone,
	Runtime: module_evm::Config + frame_system::Config<AccountId = AccountId>,
	<<Runtime as module_evm::Config>::Currency as Currency<<Runtime as frame_system::Config>::AccountId>>::Balance:
		IsType<Balance>,
{
	fn execute(
		input: &[u8],
		_target_gas: Option<usize>,
		_context: &Context,
	) -> result::Result<(ExitSucceed, Vec<u8>, usize), ExitError> {
		debug::debug!(target: "evm", "schedule call: input: {:?}", input);

		let input = Input::<Action, AccountId, AddressMapping>::new(input);

		let action = input.action()?;

		match action {
			Action::ScheduleCall => {
				let from = input.evm_address_at(1)?;
				let target = input.evm_address_at(2)?;

				let value = input.balance_at(3)?;
				let gas_limit = input.u32_at(4)?;
				let storage_limit = input.u32_at(5)?;
				let min_delay = input.u32_at(6)?;
				let input_len = input.u32_at(7)?;
				let input_data = input.bytes_at(8 * PER_PARAM_BYTES, input_len as usize)?;

				debug::debug!(
					target: "evm",
					"schedule call: from: {:?}, target: {:?}, value: {:?}, gas_limit: {:?}, storage_limit: {:?}, min_delay: {:?}, input_len: {:?}, input_data: {:?}",
					from,
					target,
					value,
					gas_limit,
					storage_limit,
					min_delay,
					input_len,
					input_data,
				);

				let call = module_evm::Call::<Runtime>::scheduled_call(
					from,
					target,
					input_data.clone(),
					value.into(),
					gas_limit,
					storage_limit,
				)
				.into();

				let delay = DispatchTime::After(min_delay);
				let origin = Origin::root().caller().clone();

				let from_account = AddressMapping::get_account_id(&from);

				// reserve the deposit for gas_limit and storage_limit
				let total_fee = Runtime::StorageDepositPerByte::get()
					.saturating_mul(storage_limit.into())
					.saturating_add(gas_limit.into());
				Runtime::Currency::reserve(&from_account, total_fee).map_err(|e| {
					let err_msg: &str = e.into();
					ExitError::Other(err_msg.into())
				})?;

				Scheduler::schedule_named(
					Encode::encode(&(
						&"ScheduleCall",
						from,
						input_data,
						<frame_system::Module<Runtime>>::block_number(),
					)),
					delay,
					None,
					0,
					origin,
					call,
				)
				.map_err(|_| ExitError::Other("Scheduler failed".into()))?;

				Ok((ExitSucceed::Returned, vec![], 0))
			}
			Action::Unknown => Err(ExitError::Other("unknown action".into())),
		}
	}
}
