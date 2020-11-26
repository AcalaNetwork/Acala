use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use sp_core::{H160, U256};
use sp_std::{borrow::Cow, marker::PhantomData, prelude::*, result};

use orml_traits::NFT as NFTT;

use super::input::{Input, InputT};
use primitives::{evm::AddressMapping as AddressMappingT, NFTBalance};

/// The `NFT` impl precompile.
///
/// `input` data starts with `action`.
///
/// Actions:
/// - Query balance. Rest `input` bytes: `account_id`.
/// - Query owner. Rest `input` bytes: `class_id`, `token_id`.
/// - Transfer. Rest `input`bytes: `from`, `to`, `class_id`, `token_id`.
pub struct NFTPrecompile<AccountId, AddressMapping, NFT>(PhantomData<(AccountId, AddressMapping, NFT)>);

enum Action {
	QueryBalance,
	QueryOwner,
	Transfer,
	Unknown,
}

impl From<u8> for Action {
	fn from(a: u8) -> Self {
		match a {
			0 => Action::QueryBalance,
			1 => Action::QueryOwner,
			2 => Action::Transfer,
			_ => Action::Unknown,
		}
	}
}

impl<AccountId, AddressMapping, NFT> Precompile for NFTPrecompile<AccountId, AddressMapping, NFT>
where
	AccountId: Clone,
	AddressMapping: AddressMappingT<AccountId>,
	NFT: NFTT<AccountId, Balance = NFTBalance, ClassId = u64, TokenId = u64>,
{
	fn execute(
		input: &[u8],
		_target_gas: Option<usize>,
		_context: &Context,
	) -> result::Result<(ExitSucceed, Vec<u8>, usize), ExitError> {
		let input = Input::<Action, AccountId, AddressMapping>::new(input);

		let action = input.action()?;

		match action {
			Action::QueryBalance => {
				let who = input.account_id_at(1)?;
				let balance = vec_u8_from_balance(NFT::balance(&who));

				Ok((ExitSucceed::Returned, balance, 0))
			}
			Action::QueryOwner => {
				let class_id = input.u64_at(1)?;
				let token_id = input.u64_at(2)?;

				let owner: H160 = if let Some(o) = NFT::owner((class_id, token_id)) {
					AddressMapping::to_evm_address(&o).unwrap_or_default()
				} else {
					Default::default()
				};

				Ok((ExitSucceed::Returned, owner.as_bytes().to_vec(), 0))
			}
			Action::Transfer => {
				let from = input.account_id_at(1)?;
				let to = input.account_id_at(2)?;

				let class_id = input.u64_at(3)?;
				let token_id = input.u64_at(4)?;

				NFT::transfer(&from, &to, (class_id, token_id))
					.map_err(|e| ExitError::Other(Cow::Borrowed(e.into())))?;

				Ok((ExitSucceed::Returned, vec![], 0))
			}
			Action::Unknown => Err(ExitError::Other("unknown action".into())),
		}
	}
}

fn vec_u8_from_balance(b: NFTBalance) -> Vec<u8> {
	let mut be_bytes = [0u8; 32];
	U256::from(b).to_big_endian(&mut be_bytes[..]);
	be_bytes.to_vec()
}
