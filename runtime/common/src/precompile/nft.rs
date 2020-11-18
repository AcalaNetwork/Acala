use module_evm::{AddressMapping, Context, ExitError, ExitSucceed, Precompile};
use sp_core::{H160, U256};
use sp_std::{borrow::Cow, marker::PhantomData, prelude::*, result};

use orml_traits::NFT;

use super::account_id_from_slice;
use module_support::AccountMapping;
use primitives::NFTBalance;

/// The `NFT` impl precompile.
///
/// All `input` data start with `action_byte`;
///
/// The first byte of `input` indicates action.
/// -. `0`: Query balance. Rest: `account_id`.
/// -. `1`: Query owner. Rest `class_id ++ token_id`.
/// -. `2`: Transfer. Rest: `from ++ to ++ class_id ++ token_id`.
pub struct NFTPrecompile<AccountId, AccountIdConverter, AccountMappingImpl, NFTImpl>(
	PhantomData<(AccountId, AccountIdConverter, AccountMappingImpl, NFTImpl)>,
);

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

impl<AccountId, AccountIdConverter, AccountMappingImpl, NFTImpl> Precompile
	for NFTPrecompile<AccountId, AccountIdConverter, AccountMappingImpl, NFTImpl>
where
	AccountId: Clone,
	AccountIdConverter: AddressMapping<AccountId>,
	AccountMappingImpl: AccountMapping<AccountId>,
	NFTImpl: NFT<AccountId, Balance = NFTBalance, ClassId = u64, TokenId = u64>,
{
	fn execute(
		input: &[u8],
		_target_gas: Option<usize>,
		_context: &Context,
	) -> result::Result<(ExitSucceed, Vec<u8>, usize), ExitError> {
		if input.len() < 2 {
			return Err(ExitError::Other("invalid input".into()));
		}
		let action: Action = input[0].into();

		match action {
			Action::QueryBalance => {
				// 32 * 2
				if input.len() < 64 {
					return Err(ExitError::Other("invalid input".into()));
				}

				let who = account_id_from_slice::<_, AccountIdConverter>(&input[32..52]);
				let balance = vec_u8_from_balance(NFTImpl::balance(&who));

				Ok((ExitSucceed::Returned, balance, 0))
			}
			Action::QueryOwner => {
				// 32 * 3
				if input.len() < 96 {
					return Err(ExitError::Other("invalid input".into()));
				}

				let class_id = u64_from_slice(&input[32..40]);
				let token_id = u64_from_slice(&input[64..72]);

				let owner: H160 = if let Some(o) = NFTImpl::owner((class_id, token_id)) {
					AccountMappingImpl::into_h160(o)
				} else {
					Default::default()
				};

				Ok((ExitSucceed::Returned, owner.as_bytes().to_vec(), 0))
			}
			Action::Transfer => {
				// 32 * 5
				if input.len() < 160 {
					return Err(ExitError::Other("invalid input".into()));
				}

				let from = account_id_from_slice::<_, AccountIdConverter>(&input[32..52]);
				let to = account_id_from_slice::<_, AccountIdConverter>(&input[64..84]);
				let class_id = u64_from_slice(&input[96..104]);
				let token_id = u64_from_slice(&input[128..136]);

				NFTImpl::transfer(&from, &to, (class_id, token_id))
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

/// Note that slice length for `u64` must not exceed 8.
fn u64_from_slice(src: &[u8]) -> u64 {
	let mut int = [0u8; 8];
	int[..].copy_from_slice(src);
	u64::from_be_bytes(int)
}
