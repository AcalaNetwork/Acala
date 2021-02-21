use crate::evm::EvmAddress;
use bstringify::bstringify;
use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;
use sp_std::{
	convert::{Into, TryFrom, TryInto},
	prelude::*,
};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

macro_rules! create_currency_id {
    ($(#[$meta:meta])*
	$vis:vis enum TokenSymbol {
        $($(#[$vmeta:meta])* $vname:ident = $val:literal,)*
    }) => {
        $(#[$meta])*
        $vis enum TokenSymbol {
            $($(#[$vmeta])* $vname = $val,)*
        }

        impl TryFrom<u8> for TokenSymbol {
            type Error = ();

            fn try_from(v: u8) -> Result<Self, Self::Error> {
                match v {
                    $($val => Ok(TokenSymbol::$vname),)*
                    _ => Err(()),
                }
            }
        }

		impl TryFrom<Vec<u8>> for CurrencyId {
			type Error = ();
			fn try_from(v: Vec<u8>) -> Result<CurrencyId, ()> {
				match v.as_slice() {
					$(bstringify!(vname) => Ok(CurrencyId::Token(TokenSymbol::$vname)),)*
					_ => Err(()),
				}
			}
		}

		#[test]
		#[ignore]
		fn generate_token_resources() {
			#[allow(non_snake_case)]
			#[derive(Serialize, Deserialize)]
			struct Token {
				name: String,
				symbol: String,
				currencyId: String,
			}

			let tokens = vec![
				$(
					Token {
						name: stringify!($vname).to_string(),
						symbol: stringify!($vname).to_string(),
						currencyId: $val.to_string(),
					},
				)*
			];
			frame_support::assert_ok!(std::fs::write("../resources/tokens.json", serde_json::to_string_pretty(&tokens).unwrap()));
		}
    }
}

create_currency_id! {
	// Represent a Token symbol with 8 bit
	// Bit 8 : 0 for Pokladot Ecosystem, 1 for Kusama Ecosystem
	// Bit 7 : Reserved
	// Bit 6 - 1 : The token ID
	#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	#[repr(u8)]
	pub enum TokenSymbol {
		// Polkadot Ecosystem
		ACA = 0,
		AUSD = 1,
		DOT = 2,
		LDOT = 3,
		XBTC = 4,
		RENBTC = 5,
		POLKABTC = 6,
		PLM = 7,
		PHA = 8,

		// Kusama Ecosystem
		KAR = 128,
		KUSD = 129,
		KSM = 130,
		LKSM = 131,
		// Reserve for XBTC = 132
		// Reserve for RENBTC = 133
		// Reserve for POLKABTC = 134
		SDN = 135,
		// Reserve for PHA = 136
	}
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	Token(TokenSymbol),
	DEXShare(TokenSymbol, TokenSymbol),
	ERC20(EvmAddress),
}

impl CurrencyId {
	pub fn is_token_currency_id(&self) -> bool {
		matches!(self, CurrencyId::Token(_))
	}

	pub fn is_dex_share_currency_id(&self) -> bool {
		matches!(self, CurrencyId::DEXShare(_, _))
	}

	pub fn split_dex_share_currency_id(&self) -> Option<(Self, Self)> {
		match self {
			CurrencyId::DEXShare(token_symbol_0, token_symbol_1) => {
				Some((CurrencyId::Token(*token_symbol_0), CurrencyId::Token(*token_symbol_1)))
			}
			_ => None,
		}
	}

	pub fn join_dex_share_currency_id(currency_id_0: Self, currency_id_1: Self) -> Option<Self> {
		match (currency_id_0, currency_id_1) {
			(CurrencyId::Token(token_symbol_0), CurrencyId::Token(token_symbol_1)) => {
				Some(CurrencyId::DEXShare(token_symbol_0, token_symbol_1))
			}
			_ => None,
		}
	}
}

/// Note the pre-deployed ERC20 contracts depend on `CurrencyId` implementation,
/// and need to be updated if any change.
impl TryFrom<[u8; 32]> for CurrencyId {
	type Error = ();

	fn try_from(v: [u8; 32]) -> Result<Self, Self::Error> {
		if !v.starts_with(&[0u8; 29][..]) {
			return Err(());
		}

		// token
		if v[29] == 0 && v[31] == 0 {
			return v[30].try_into().map(CurrencyId::Token);
		}

		// DEX share
		if v[29] == 1 {
			let left = v[30].try_into()?;
			let right = v[31].try_into()?;
			return Ok(CurrencyId::DEXShare(left, right));
		}

		Err(())
	}
}

/// Note the pre-deployed ERC20 contracts depend on `CurrencyId` implementation,
/// and need to be updated if any change.
impl From<CurrencyId> for [u8; 32] {
	fn from(val: CurrencyId) -> Self {
		let mut bytes = [0u8; 32];
		match val {
			CurrencyId::Token(token) => {
				bytes[30] = token as u8;
			}
			CurrencyId::DEXShare(left, right) => {
				bytes[29] = 1;
				bytes[30] = left as u8;
				bytes[31] = right as u8;
			}
			_ => {}
		}
		bytes
	}
}
