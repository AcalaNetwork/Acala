use frame_support::debug;
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use sp_core::U256;
use sp_std::{borrow::Cow, marker::PhantomData, prelude::*, result};

use module_support::EVMStateRentTrait;

use super::input::{Input, InputT};
use primitives::{evm::AddressMapping as AddressMappingT, Balance};

/// The `EVM` impl precompile.
///
/// `input` data starts with `action`.
///
/// Actions:
/// - QueryContractExistentialDeposit.
/// - QueryTransferMaintainerDeposit.
/// - QueryStorageDepositPerByte.
/// - QueryMaintainer.
/// - QueryDeveloperDeposit.
/// - QueryDeploymentFee.
/// - RequestTransferMaintainer. Rest `input` bytes: `from`, `contract`.
/// - CancelTransferMaintainer. Rest `input` bytes: `from`, `contract`.
/// - ConfirmTransferMaintainer. Rest `input` bytes: `from`, `contract`,
///   `new_maintainer`.
/// - RejectTransferMaintainer. Rest `input` bytes: `from`, `contract`,
///   `invalid_maintainer`.
pub struct StateRentPrecompile<AccountId, AddressMapping, EVM>(PhantomData<(AccountId, AddressMapping, EVM)>);

enum Action {
	QueryContractExistentialDeposit,
	QueryTransferMaintainerDeposit,
	QueryStorageDepositPerByte,
	QueryMaintainer,
	QueryDeveloperDeposit,
	QueryDeploymentFee,
	RequestTransferMaintainer,
	CancelTransferMaintainer,
	ConfirmTransferMaintainer,
	RejectTransferMaintainer,
	Unknown,
}

impl From<u8> for Action {
	fn from(a: u8) -> Self {
		// reserve 0 - 127 for query, 128 - 255 for action
		match a {
			0 => Action::QueryContractExistentialDeposit,
			1 => Action::QueryTransferMaintainerDeposit,
			2 => Action::QueryStorageDepositPerByte,
			3 => Action::QueryMaintainer,
			4 => Action::QueryDeveloperDeposit,
			5 => Action::QueryDeploymentFee,
			128 => Action::RequestTransferMaintainer,
			129 => Action::CancelTransferMaintainer,
			130 => Action::ConfirmTransferMaintainer,
			131 => Action::RejectTransferMaintainer,
			_ => Action::Unknown,
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
		_target_gas: Option<usize>,
		_context: &Context,
	) -> result::Result<(ExitSucceed, Vec<u8>, usize), ExitError> {
		debug::debug!(target: "evm", "state_rent input: {:?}", input);
		let input = Input::<Action, AccountId, AddressMapping>::new(input);

		let action = input.action()?;

		match action {
			Action::QueryContractExistentialDeposit => {
				let deposit = vec_u8_from_balance(EVM::query_contract_existential_deposit());
				Ok((ExitSucceed::Returned, deposit, 0))
			}
			Action::QueryTransferMaintainerDeposit => {
				let deposit = vec_u8_from_balance(EVM::query_transfer_maintainer_deposit());
				Ok((ExitSucceed::Returned, deposit, 0))
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
			Action::RequestTransferMaintainer => {
				let from = input.account_id_at(1)?;
				let contract = input.evm_address_at(2)?;

				EVM::request_transfer_maintainer(from, contract)
					.map_err(|e| ExitError::Other(Cow::Borrowed(e.into())))?;

				Ok((ExitSucceed::Returned, vec![], 0))
			}
			Action::CancelTransferMaintainer => {
				let from = input.account_id_at(1)?;
				let contract = input.evm_address_at(2)?;

				EVM::cancel_transfer_maintainer(from, contract)
					.map_err(|e| ExitError::Other(Cow::Borrowed(e.into())))?;

				Ok((ExitSucceed::Returned, vec![], 0))
			}
			Action::ConfirmTransferMaintainer => {
				let from = input.account_id_at(1)?;
				let contract = input.evm_address_at(2)?;
				let new_maintainer = input.evm_address_at(3)?;

				EVM::confirm_transfer_maintainer(from, contract, new_maintainer)
					.map_err(|e| ExitError::Other(Cow::Borrowed(e.into())))?;

				Ok((ExitSucceed::Returned, vec![], 0))
			}
			Action::RejectTransferMaintainer => {
				let from = input.account_id_at(1)?;
				let contract = input.evm_address_at(2)?;
				let new_maintainer = input.evm_address_at(3)?;

				EVM::reject_transfer_maintainer(from, contract, new_maintainer)
					.map_err(|e| ExitError::Other(Cow::Borrowed(e.into())))?;

				Ok((ExitSucceed::Returned, vec![], 0))
			}
			Action::Unknown => Err(ExitError::Other("unknown action".into())),
		}
	}
}

fn vec_u8_from_balance(b: Balance) -> Vec<u8> {
	let mut be_bytes = [0u8; 32];
	U256::from(b).to_big_endian(&mut be_bytes[..]);
	be_bytes.to_vec()
}
