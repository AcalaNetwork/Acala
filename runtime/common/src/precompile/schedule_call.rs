// Disable the following lints
#![allow(clippy::type_complexity)]

use frame_support::{
	debug,
	dispatch::Dispatchable,
	parameter_types,
	traits::{
		schedule::{DispatchTime, Named as ScheduleNamed},
		Currency, IsType, OriginTrait,
	},
};
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use module_support::TransactionPayment;
use primitives::{evm::AddressMapping as AddressMappingT, Balance, BlockNumber};
use sp_core::U256;
use sp_std::{convert::TryFrom, fmt::Debug, marker::PhantomData, prelude::*, result};

use super::input::{Input, InputT, PER_PARAM_BYTES};
use codec::Encode;
use pallet_scheduler::TaskAddress;

parameter_types! {
	pub storage EvmSchedulerNextID: u32 = 0u32;
}

/// The `ScheduleCall` impl precompile.
///
///
/// `input` data starts with `action`.
///
/// Actions:
/// - ScheduleCall. Rest `input` bytes: `from`, `target`, `value`, `gas_limit`,
///   `storage_limit`, `min_delay`, `input_len`, `input_data`.
pub struct ScheduleCallPrecompile<
	AccountId,
	AddressMapping,
	Scheduler,
	ChargeTransactionPayment,
	Call,
	Origin,
	PalletsOrigin,
	Runtime,
>(
	PhantomData<(
		AccountId,
		AddressMapping,
		Scheduler,
		ChargeTransactionPayment,
		Call,
		Origin,
		PalletsOrigin,
		Runtime,
	)>,
);

enum Action {
	ScheduleCall,
}

impl TryFrom<u8> for Action {
	type Error = ();

	fn try_from(value: u8) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(Action::ScheduleCall),
			_ => Err(()),
		}
	}
}

type PalletBalanceOf<T> =
	<<T as module_evm::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
	<<T as module_evm::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

impl<AccountId, AddressMapping, Scheduler, ChargeTransactionPayment, Call, Origin, PalletsOrigin, Runtime> Precompile
	for ScheduleCallPrecompile<
		AccountId,
		AddressMapping,
		Scheduler,
		ChargeTransactionPayment,
		Call,
		Origin,
		PalletsOrigin,
		Runtime,
	> where
	AccountId: Debug + Clone,
	AddressMapping: AddressMappingT<AccountId>,
	Scheduler: ScheduleNamed<BlockNumber, Call, PalletsOrigin, Address = TaskAddress<BlockNumber>>,
	ChargeTransactionPayment: TransactionPayment<AccountId, Balance, NegativeImbalanceOf<Runtime>>,
	Call: Dispatchable + Debug + From<module_evm::Call<Runtime>>,
	Origin: IsType<<Runtime as frame_system::Config>::Origin> + OriginTrait<PalletsOrigin = PalletsOrigin>,
	PalletsOrigin: Into<<Runtime as frame_system::Config>::Origin> + From<frame_system::RawOrigin<AccountId>> + Clone,
	Runtime: module_evm::Config + frame_system::Config<AccountId = AccountId>,
	PalletBalanceOf<Runtime>: IsType<Balance>,
{
	fn execute(
		input: &[u8],
		_target_gas: Option<u64>,
		_context: &Context,
	) -> result::Result<(ExitSucceed, Vec<u8>, u64), ExitError> {
		debug::debug!(target: "evm", "schedule call: input: {:?}", input);

		let input = Input::<Action, AccountId, AddressMapping>::new(input);

		let action = input.action()?;

		match action {
			Action::ScheduleCall => {
				let from = input.evm_address_at(1)?;
				let target = input.evm_address_at(2)?;

				let value = input.balance_at(3)?;
				let gas_limit = input.u64_at(4)?;
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

				#[cfg(not(feature = "with-ethereum-compatibility"))]
				{
					//// reserve the transaction fee for gas_limit
					use sp_runtime::traits::Convert;
					let from_account = AddressMapping::get_account_id(&from);
					let weight = <Runtime as module_evm::Config>::GasToWeight::convert(gas_limit);
					ChargeTransactionPayment::reserve_fee(&from_account, weight).map_err(|e| {
						let err_msg: &str = e.into();
						ExitError::Other(err_msg.into())
					})?;
				}

				let call = module_evm::Call::<Runtime>::scheduled_call(
					from,
					target,
					input_data,
					value.into(),
					gas_limit,
					storage_limit,
				)
				.into();

				let delay = DispatchTime::After(min_delay);
				let origin = Origin::root().caller().clone();

				let current_id = EvmSchedulerNextID::get();
				let next_id = current_id
					.checked_add(1)
					.ok_or_else(|| ExitError::Other("Scheduler next id overflow".into()))?;
				EvmSchedulerNextID::set(&next_id);

				let task_address = Scheduler::schedule_named(
					Encode::encode(&(&"ScheduleCall", current_id)),
					delay,
					None,
					0,
					origin,
					call,
				)
				.map_err(|_| ExitError::Other("Scheduler failed".into()))?;

				Ok((ExitSucceed::Returned, vec_u8_from_tuple(task_address), 0))
			}
		}
	}
}

fn vec_u8_from_tuple(task_address: TaskAddress<BlockNumber>) -> Vec<u8> {
	let mut be_bytes_0 = [0u8; 32];
	U256::from(task_address.0).to_big_endian(&mut be_bytes_0[..]);

	let mut be_bytes_1 = [0u8; 32];
	U256::from(task_address.1).to_big_endian(&mut be_bytes_1[..]);

	vec![be_bytes_0.to_vec(), be_bytes_1.to_vec()].concat()
}
