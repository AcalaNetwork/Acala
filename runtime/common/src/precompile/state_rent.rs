use frame_support::debug;
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use sp_core::U256;
use sp_std::{borrow::Cow, convert::TryFrom, marker::PhantomData, prelude::*, result};

use module_support::EVMStateRentTrait;

use super::input::{Input, InputT};
use primitives::{evm::AddressMapping as AddressMappingT, Balance};

/// The `EVM` impl precompile.
///
/// `input` data starts with `action`.
///
/// Actions:
/// - QueryNewContractExtraBytes.
/// - QueryStorageDepositPerByte.
/// - QueryMaintainer.
/// - QueryDeveloperDeposit.
/// - QueryDeploymentFee.
/// - TransferMaintainer. Rest `input` bytes: `from`, `contract`,
///   `new_maintainer`.
pub struct StateRentPrecompile<AccountId, AddressMapping, EVM>(PhantomData<(AccountId, AddressMapping, EVM)>);

enum Action {
	QueryNewContractExtraBytes,
	QueryStorageDepositPerByte,
	QueryMaintainer,
	QueryDeveloperDeposit,
	QueryDeploymentFee,
	TransferMaintainer,
}

impl TryFrom<u8> for Action {
	type Error = ();

	fn try_from(value: u8) -> Result<Self, Self::Error> {
		// reserve 0 - 127 for query, 128 - 255 for action
		match value {
			0 => Ok(Action::QueryNewContractExtraBytes),
			1 => Ok(Action::QueryStorageDepositPerByte),
			2 => Ok(Action::QueryMaintainer),
			3 => Ok(Action::QueryDeveloperDeposit),
			4 => Ok(Action::QueryDeploymentFee),
			128 => Ok(Action::TransferMaintainer),
			_ => Err(()),
		}
	}
}

impl<AccountId, AddressMapping, EVM> Precompile for StateRentPrecompile<AccountId, AddressMapping, EVM>
where
	AccountId: Clone,
	AddressMapping: AddressMappingT<AccountId>,
	EVM: EVMStateRentTrait<AccountId, Balance>,
{
	fn execute(
		input: &[u8],
		_target_gas: Option<u64>,
		_context: &Context,
	) -> result::Result<(ExitSucceed, Vec<u8>, u64), ExitError> {
		debug::debug!(target: "evm", "state_rent input: {:?}", input);
		let input = Input::<Action, AccountId, AddressMapping>::new(input);

		let action = input.action()?;

		match action {
			Action::QueryNewContractExtraBytes => {
				let bytes = vec_u8_from_u32(EVM::query_new_contract_extra_bytes());
				Ok((ExitSucceed::Returned, bytes, 0))
			}
			Action::QueryStorageDepositPerByte => {
				let deposit = vec_u8_from_balance(EVM::query_storage_deposit_per_byte());
				Ok((ExitSucceed::Returned, deposit, 0))
			}
			Action::QueryMaintainer => {
				let contract = input.evm_address_at(1)?;

				let maintainer =
					EVM::query_maintainer(contract).map_err(|e| ExitError::Other(Cow::Borrowed(e.into())))?;

				let mut address = [0u8; 32];
				address[12..].copy_from_slice(&maintainer.as_bytes().to_vec());

				Ok((ExitSucceed::Returned, address.to_vec(), 0))
			}
			Action::QueryDeveloperDeposit => {
				let deposit = vec_u8_from_balance(EVM::query_developer_deposit());
				Ok((ExitSucceed::Returned, deposit, 0))
			}
			Action::QueryDeploymentFee => {
				let fee = vec_u8_from_balance(EVM::query_deployment_fee());
				Ok((ExitSucceed::Returned, fee, 0))
			}
			Action::TransferMaintainer => {
				let from = input.account_id_at(1)?;
				let contract = input.evm_address_at(2)?;
				let new_maintainer = input.evm_address_at(3)?;

				EVM::transfer_maintainer(from, contract, new_maintainer)
					.map_err(|e| ExitError::Other(Cow::Borrowed(e.into())))?;

				Ok((ExitSucceed::Returned, vec![], 0))
			}
		}
	}
}

fn vec_u8_from_balance(b: Balance) -> Vec<u8> {
	let mut be_bytes = [0u8; 32];
	U256::from(b).to_big_endian(&mut be_bytes[..]);
	be_bytes.to_vec()
}

fn vec_u8_from_u32(b: u32) -> Vec<u8> {
	let mut be_bytes = [0u8; 32];
	U256::from(b).to_big_endian(&mut be_bytes[..]);
	be_bytes.to_vec()
}
