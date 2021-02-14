// Disable the following lints
#![allow(clippy::type_complexity)]

use frame_support::{
	debug,
	dispatch::Dispatchable,
	ensure, parameter_types,
	traits::{
		schedule::{DispatchTime, Named as ScheduleNamed},
		Currency, IsType, OriginTrait,
	},
};
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use module_support::TransactionPayment;
use primitives::{evm::AddressMapping as AddressMappingT, Balance, BlockNumber};
use sp_core::{H160, U256};
use sp_std::{fmt::Debug, marker::PhantomData, prelude::*, result};

use super::input::{Input, InputT, BALANCE_BYTES, PER_PARAM_BYTES};
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
	CancelCall,
	RescheduleCall,
	Unknown,
}

impl From<u8> for Action {
	fn from(a: u8) -> Self {
		match a {
			0 => Action::ScheduleCall,
			1 => Action::CancelCall,
			2 => Action::RescheduleCall,
			_ => Action::Unknown,
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
	ChargeTransactionPayment: TransactionPayment<AccountId, PalletBalanceOf<Runtime>, NegativeImbalanceOf<Runtime>>,
	Call: Dispatchable<Origin = Origin> + Debug + From<module_evm::Call<Runtime>>,
	Origin: IsType<<Runtime as frame_system::Config>::Origin>
		+ OriginTrait<AccountId = AccountId, PalletsOrigin = PalletsOrigin>,
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

				let from_account = AddressMapping::get_account_id(&from);
				let mut _fee: PalletBalanceOf<Runtime> = Default::default();
				#[cfg(not(feature = "with-ethereum-compatibility"))]
				{
					//// reserve the transaction fee for gas_limit
					use sp_runtime::traits::Convert;
					let weight = <Runtime as module_evm::Config>::GasToWeight::convert(gas_limit);
					_fee = ChargeTransactionPayment::reserve_fee(&from_account, weight).map_err(|e| {
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

				let current_id = EvmSchedulerNextID::get();
				let next_id = current_id
					.checked_add(1)
					.ok_or_else(|| ExitError::Other("Scheduler next id overflow".into()))?;
				EvmSchedulerNextID::set(&next_id);

				// task_id = id + sender + reserve_fee
				let task_id = join_task_id(Encode::encode(&(&"ScheduleCall", current_id)), from, _fee.into());
				Scheduler::schedule_named(
					task_id.clone(),
					DispatchTime::After(min_delay),
					None,
					0,
					Origin::root().caller().clone(),
					call,
				)
				.map_err(|_| ExitError::Other("Schedule failed".into()))?;

				Ok((ExitSucceed::Returned, task_id, 0))
			}
			Action::CancelCall => {
				let from = input.evm_address_at(1)?;
				let task_id = input.bytes_at(2 * PER_PARAM_BYTES, 96)?;

				debug::debug!(
					target: "evm",
					"cancel call: from: {:?}, task_id: {:?}",
					from,
					task_id,
				);

				// task_id = id + sender + reserve_fee
				let (_, sender, fee) = split_task_id(&task_id);
				ensure!(sender == from, ExitError::Other("NoPermission".into()));

				#[cfg(not(feature = "with-ethereum-compatibility"))]
				{
					// unreserve the transaction fee for gas_limit
					let from_account = AddressMapping::get_account_id(&from);
					ChargeTransactionPayment::unreserve_fee(&from_account, fee.into());
				}

				Scheduler::cancel_named(task_id).map_err(|_| ExitError::Other("Cancel schedule failed".into()))?;

				Ok((ExitSucceed::Returned, vec![], 0))
			}
			Action::RescheduleCall => {
				let from = input.evm_address_at(1)?;
				let task_id = input.bytes_at(2 * PER_PARAM_BYTES, 96)?;
				let min_delay = input.u32_at(3)?;

				debug::debug!(
					target: "evm",
					"reschedule call: from: {:?}, task_id: {:?}, min_delay: {:?}",
					from,
					task_id,
					min_delay,
				);

				// task_id = id + sender + reserve_fee
				let (_, sender, _) = split_task_id(&task_id);
				ensure!(sender == from, ExitError::Other("NoPermission".into()));

				let delay = DispatchTime::After(min_delay);
				Scheduler::reschedule_named(task_id, delay).map_err(|e| {
					let err_msg: &str = e.into();
					ExitError::Other(err_msg.into())
				})?;

				Ok((ExitSucceed::Returned, vec![], 0))
			}
			Action::Unknown => Err(ExitError::Other("unknown action".into())),
		}
	}
}

fn join_task_id(id: Vec<u8>, sender: H160, fee: Balance) -> Vec<u8> {
	let mut be_bytes = [0u8; 96];
	U256::from(&id[..]).to_big_endian(&mut be_bytes[..32]);
	U256::from(sender.as_bytes()).to_big_endian(&mut be_bytes[32..64]);
	U256::from(fee).to_big_endian(&mut be_bytes[64..96]);
	be_bytes.to_vec()
}

fn split_task_id(task_id: &[u8]) -> (Vec<u8>, H160, Balance) {
	let mut id = [0u8; 32];
	id[..].copy_from_slice(&task_id[0..32]);

	let sender = H160::from_slice(&task_id[32 + 12..64]);

	let mut balance = [0u8; BALANCE_BYTES];
	balance[..].copy_from_slice(&task_id[64 + PER_PARAM_BYTES - BALANCE_BYTES..96]);
	let fee = Balance::from_be_bytes(balance);
	(id.to_vec(), sender, fee)
}
