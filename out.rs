#![feature(prelude_import)]
#![no_std]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]
#[prelude_import]
use core::prelude::rust_2021::*;
#[macro_use]
extern crate core;
#[macro_use]
extern crate compiler_builtins;
pub use crate::runner::{
    stack::SubstrateStackState,
    state::{PrecompileSet, StackExecutor, StackSubstateMetadata},
    storage_meter::StorageMeter, Runner,
};
use codec::{Decode, Encode, FullCodec, MaxEncodedLen};
use frame_support::{
    dispatch::{
        DispatchError, DispatchErrorWithPostInfo, DispatchResult,
        DispatchResultWithPostInfo, Pays, PostDispatchInfo, Weight,
    },
    ensure, error::BadOrigin, log, pallet_prelude::*, parameter_types,
    traits::{
        BalanceStatus, Currency, EitherOfDiverse, EnsureOrigin, ExistenceRequirement,
        FindAuthor, Get, NamedReservableCurrency, OnKilledAccount,
    },
    transactional, BoundedVec, RuntimeDebug,
};
use frame_system::{
    ensure_root, ensure_signed, pallet_prelude::*, EnsureRoot, EnsureSigned,
};
use hex_literal::hex;
pub use module_evm_utility::{
    ethereum::{AccessListItem, Log, TransactionAction},
    evm::{
        self, Config as EvmConfig, Context, ExitError, ExitFatal, ExitReason, ExitRevert,
        ExitSucceed,
    },
    Account,
};
pub use module_support::{
    AddressMapping, DispatchableTask, EVMManager, ExecutionMode, IdleScheduler,
    InvokeContext, TransactionPayment, EVM as EVMTrait,
};
pub use orml_traits::{currency::TransferAll, MultiCurrency};
pub use primitives::{
    evm::{
        convert_decimals_from_evm, convert_decimals_to_evm, decode_gas_limit, CallInfo,
        CreateInfo, EvmAddress, ExecutionInfo, Vicinity, MIRRORED_NFT_ADDRESS_START,
        MIRRORED_TOKENS_ADDRESS_START,
    },
    task::TaskResult, Balance, CurrencyId, Nonce, ReserveIdentifier,
};
use scale_info::TypeInfo;
use sha3::{Digest, Keccak256};
use sp_core::{H160, H256, U256};
use sp_runtime::{
    traits::{
        Convert, DispatchInfoOf, One, PostDispatchInfoOf, SignedExtension,
        UniqueSaturatedInto, Zero,
    },
    transaction_validity::TransactionValidityError, Either, SaturatedConversion,
    Saturating, TransactionOutcome,
};
use sp_std::{
    cmp, collections::btree_map::BTreeMap, fmt::Debug, marker::PhantomData, prelude::*,
};
pub mod precompiles {
    //! Builtin precompiles.
    use crate::runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult};
    use module_evm_utility::evm::{Context, ExitError, ExitSucceed};
    use sp_std::vec::Vec;
    mod blake2 {
        use super::Precompile;
        use crate::runner::state::{
            PrecompileFailure, PrecompileOutput, PrecompileResult,
        };
        use module_evm_utility::evm::{Context, ExitError, ExitSucceed};
        mod eip_152 {
            /// The precomputed values for BLAKE2b [from the spec](https://tools.ietf.org/html/rfc7693#section-2.7)
            /// There are 10 16-byte arrays - one for each round
            /// the entries are calculated from the sigma constants.
            const SIGMA: [[usize; 16]; 10] = [
                [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
                [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
                [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
                [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
                [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
                [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
                [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
                [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
                [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
                [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
            ];
            /// IV is the initialization vector for BLAKE2b. See https://tools.ietf.org/html/rfc7693#section-2.6
            /// for details.
            const IV: [u64; 8] = [
                0x6a09e667f3bcc908,
                0xbb67ae8584caa73b,
                0x3c6ef372fe94f82b,
                0xa54ff53a5f1d36f1,
                0x510e527fade682d1,
                0x9b05688c2b3e6c1f,
                0x1f83d9abfb41bd6b,
                0x5be0cd19137e2179,
            ];
            #[inline(always)]
            /// The G mixing function. See https://tools.ietf.org/html/rfc7693#section-3.1
            fn g(v: &mut [u64], a: usize, b: usize, c: usize, d: usize, x: u64, y: u64) {
                v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
                v[d] = (v[d] ^ v[a]).rotate_right(32);
                v[c] = v[c].wrapping_add(v[d]);
                v[b] = (v[b] ^ v[c]).rotate_right(24);
                v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
                v[d] = (v[d] ^ v[a]).rotate_right(16);
                v[c] = v[c].wrapping_add(v[d]);
                v[b] = (v[b] ^ v[c]).rotate_right(63);
            }
            /// The Blake2 compression function F. See https://tools.ietf.org/html/rfc7693#section-3.2
            /// Takes as an argument the state vector `h`, message block vector `m`, offset counter `t`, final
            /// block indicator flag `f`, and number of rounds `rounds`. The state vector provided as the first
            /// parameter is modified by the function.
            pub fn compress(
                h: &mut [u64; 8],
                m: [u64; 16],
                t: [u64; 2],
                f: bool,
                rounds: usize,
            ) {
                let mut v = [0u64; 16];
                v[..h.len()].copy_from_slice(h);
                v[h.len()..].copy_from_slice(&IV);
                v[12] ^= t[0];
                v[13] ^= t[1];
                if f {
                    v[14] = !v[14];
                }
                for i in 0..rounds {
                    let s = &SIGMA[i % 10];
                    g(&mut v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
                    g(&mut v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
                    g(&mut v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
                    g(&mut v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
                    g(&mut v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
                    g(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
                    g(&mut v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
                    g(&mut v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
                }
                for i in 0..8 {
                    h[i] ^= v[i] ^ v[i + 8];
                }
            }
        }
        pub struct Blake2F;
        impl Blake2F {
            const GAS_COST_PER_ROUND: u64 = 1;
        }
        impl Precompile for Blake2F {
            /// Format of `input`:
            /// [4 bytes for rounds][64 bytes for h][128 bytes for m][8 bytes for t_0][8 bytes for t_1][1
            /// byte for f]
            fn execute(
                input: &[u8],
                target_gas: Option<u64>,
                _context: &Context,
                _is_static: bool,
            ) -> PrecompileResult {
                const BLAKE2_F_ARG_LEN: usize = 213;
                if input.len() != BLAKE2_F_ARG_LEN {
                    return Err(PrecompileFailure::Error {
                        exit_status: ExitError::Other(
                            "input length for Blake2 F precompile should be exactly 213 bytes"
                                .into(),
                        ),
                    });
                }
                let mut rounds_buf: [u8; 4] = [0; 4];
                rounds_buf.copy_from_slice(&input[0..4]);
                let rounds: u32 = u32::from_be_bytes(rounds_buf);
                let gas_cost: u64 = (rounds as u64) * Blake2F::GAS_COST_PER_ROUND;
                if let Some(gas_left) = target_gas {
                    if gas_left < gas_cost {
                        return Err(PrecompileFailure::Error {
                            exit_status: ExitError::OutOfGas,
                        });
                    }
                }
                let mut h_buf: [u8; 64] = [0; 64];
                h_buf.copy_from_slice(&input[4..68]);
                let mut h = [0u64; 8];
                let mut ctr = 0;
                for state_word in &mut h {
                    let mut temp: [u8; 8] = Default::default();
                    temp.copy_from_slice(&h_buf[(ctr * 8)..(ctr + 1) * 8]);
                    *state_word = u64::from_le_bytes(temp);
                    ctr += 1;
                }
                let mut m_buf: [u8; 128] = [0; 128];
                m_buf.copy_from_slice(&input[68..196]);
                let mut m = [0u64; 16];
                ctr = 0;
                for msg_word in &mut m {
                    let mut temp: [u8; 8] = Default::default();
                    temp.copy_from_slice(&m_buf[(ctr * 8)..(ctr + 1) * 8]);
                    *msg_word = u64::from_le_bytes(temp);
                    ctr += 1;
                }
                let mut t_0_buf: [u8; 8] = [0; 8];
                t_0_buf.copy_from_slice(&input[196..204]);
                let t_0 = u64::from_le_bytes(t_0_buf);
                let mut t_1_buf: [u8; 8] = [0; 8];
                t_1_buf.copy_from_slice(&input[204..212]);
                let t_1 = u64::from_le_bytes(t_1_buf);
                let f = if input[212] == 1 {
                    true
                } else if input[212] == 0 {
                    false
                } else {
                    return Err(PrecompileFailure::Error {
                        exit_status: ExitError::Other(
                            "incorrect final block indicator flag".into(),
                        ),
                    });
                };
                eip_152::compress(&mut h, m, [t_0, t_1], f, rounds as usize);
                let mut output_buf = [0u8; u64::BITS as usize];
                for (i, state_word) in h.iter().enumerate() {
                    output_buf[i * 8..(i + 1) * 8]
                        .copy_from_slice(&state_word.to_le_bytes());
                }
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    cost: gas_cost,
                    output: output_buf.to_vec(),
                    logs: Default::default(),
                })
            }
        }
    }
    mod bn128 {
        use super::Precompile;
        use crate::runner::state::{
            PrecompileFailure, PrecompileOutput, PrecompileResult,
        };
        use module_evm_utility::evm::{Context, ExitError, ExitSucceed};
        use sp_core::U256;
        use sp_std::vec::Vec;
        fn read_fr(input: &[u8], start_inx: usize) -> Result<bn::Fr, PrecompileFailure> {
            let mut padded_input = Vec::from(input);
            if padded_input.len() < start_inx + 32 {
                padded_input.resize_with(start_inx + 32, Default::default);
            }
            bn::Fr::from_slice(&padded_input[start_inx..(start_inx + 32)])
                .map_err(|_| PrecompileFailure::Error {
                    exit_status: ExitError::Other("Invalid field element".into()),
                })
        }
        fn read_point(
            input: &[u8],
            start_inx: usize,
        ) -> Result<bn::G1, PrecompileFailure> {
            use bn::{AffineG1, Fq, Group, G1};
            let mut padded_input = Vec::from(input);
            if padded_input.len() < start_inx + 64 {
                padded_input.resize_with(start_inx + 64, Default::default);
            }
            let px = Fq::from_slice(&padded_input[start_inx..(start_inx + 32)])
                .map_err(|_| PrecompileFailure::Error {
                    exit_status: ExitError::Other("Invalid point x coordinate".into()),
                })?;
            let py = Fq::from_slice(&padded_input[(start_inx + 32)..(start_inx + 64)])
                .map_err(|_| PrecompileFailure::Error {
                    exit_status: ExitError::Other("Invalid point y coordinate".into()),
                })?;
            Ok(
                if px == Fq::zero() && py == Fq::zero() {
                    G1::zero()
                } else {
                    AffineG1::new(px, py)
                        .map_err(|_| PrecompileFailure::Error {
                            exit_status: ExitError::Other("Invalid curve point".into()),
                        })?
                        .into()
                },
            )
        }
        /// The Bn128Add builtin
        pub struct Bn128Add;
        impl Bn128Add {
            const GAS_COST: u64 = 150;
        }
        impl Precompile for Bn128Add {
            fn execute(
                input: &[u8],
                _target_gas: Option<u64>,
                _context: &Context,
                _is_static: bool,
            ) -> PrecompileResult {
                use bn::AffineG1;
                let p1 = read_point(input, 0)?;
                let p2 = read_point(input, 64)?;
                let mut buf = [0u8; 64];
                if let Some(sum) = AffineG1::from_jacobian(p1 + p2) {
                    sum.x()
                        .to_big_endian(&mut buf[0..32])
                        .map_err(|_| PrecompileFailure::Error {
                            exit_status: ExitError::Other(
                                "Cannot fail since 0..32 is 32-byte length".into(),
                            ),
                        })?;
                    sum.y()
                        .to_big_endian(&mut buf[32..64])
                        .map_err(|_| PrecompileFailure::Error {
                            exit_status: ExitError::Other(
                                "Cannot fail since 32..64 is 32-byte length".into(),
                            ),
                        })?;
                }
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    cost: Bn128Add::GAS_COST,
                    output: buf.to_vec(),
                    logs: Default::default(),
                })
            }
        }
        /// The Bn128Mul builtin
        pub struct Bn128Mul;
        impl Bn128Mul {
            const GAS_COST: u64 = 6_000;
        }
        impl Precompile for Bn128Mul {
            fn execute(
                input: &[u8],
                _target_gas: Option<u64>,
                _context: &Context,
                _is_static: bool,
            ) -> PrecompileResult {
                use bn::AffineG1;
                let p = read_point(input, 0)?;
                let fr = read_fr(input, 64)?;
                let mut buf = [0u8; 64];
                if let Some(sum) = AffineG1::from_jacobian(p * fr) {
                    sum.x()
                        .to_big_endian(&mut buf[0..32])
                        .map_err(|_| PrecompileFailure::Error {
                            exit_status: ExitError::Other(
                                "Cannot fail since 0..32 is 32-byte length".into(),
                            ),
                        })?;
                    sum.y()
                        .to_big_endian(&mut buf[32..64])
                        .map_err(|_| PrecompileFailure::Error {
                            exit_status: ExitError::Other(
                                "Cannot fail since 32..64 is 32-byte length".into(),
                            ),
                        })?;
                }
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    cost: Bn128Mul::GAS_COST,
                    output: buf.to_vec(),
                    logs: Default::default(),
                })
            }
        }
        /// The Bn128Pairing builtin
        pub struct Bn128Pairing;
        impl Bn128Pairing {
            const BASE_GAS_COST: u64 = 45_000;
            const GAS_COST_PER_PAIRING: u64 = 34_000;
        }
        impl Precompile for Bn128Pairing {
            fn execute(
                input: &[u8],
                target_gas: Option<u64>,
                _context: &Context,
                _is_static: bool,
            ) -> PrecompileResult {
                use bn::{pairing_batch, AffineG1, AffineG2, Fq, Fq2, Group, Gt, G1, G2};
                if let Some(gas_left) = target_gas {
                    if gas_left < Bn128Pairing::BASE_GAS_COST {
                        return Err(PrecompileFailure::Error {
                            exit_status: ExitError::OutOfGas,
                        });
                    }
                }
                if input.len() % 192 != 0 {
                    return Err(PrecompileFailure::Error {
                        exit_status: ExitError::Other(
                            "Invalid input length, must be multiple of 192 (3 * (32*2))"
                                .into(),
                        ),
                    });
                }
                let (ret_val, gas_cost) = if input.is_empty() {
                    (U256::one(), Bn128Pairing::BASE_GAS_COST)
                } else {
                    let elements = input.len() / 192;
                    let gas_cost: u64 = Bn128Pairing::BASE_GAS_COST
                        + (elements as u64 * Bn128Pairing::GAS_COST_PER_PAIRING);
                    if let Some(gas_left) = target_gas {
                        if gas_left < gas_cost {
                            return Err(PrecompileFailure::Error {
                                exit_status: ExitError::OutOfGas,
                            });
                        }
                    }
                    let mut vals = Vec::new();
                    for idx in 0..elements {
                        let a_x = Fq::from_slice(&input[idx * 192..idx * 192 + 32])
                            .map_err(|_| PrecompileFailure::Error {
                                exit_status: ExitError::Other(
                                    "Invalid a argument x coordinate".into(),
                                ),
                            })?;
                        let a_y = Fq::from_slice(&input[idx * 192 + 32..idx * 192 + 64])
                            .map_err(|_| PrecompileFailure::Error {
                                exit_status: ExitError::Other(
                                    "Invalid a argument y coordinate".into(),
                                ),
                            })?;
                        let b_a_y = Fq::from_slice(
                                &input[idx * 192 + 64..idx * 192 + 96],
                            )
                            .map_err(|_| PrecompileFailure::Error {
                                exit_status: ExitError::Other(
                                    "Invalid b argument imaginary coeff x coordinate".into(),
                                ),
                            })?;
                        let b_a_x = Fq::from_slice(
                                &input[idx * 192 + 96..idx * 192 + 128],
                            )
                            .map_err(|_| PrecompileFailure::Error {
                                exit_status: ExitError::Other(
                                    "Invalid b argument imaginary coeff y coordinate".into(),
                                ),
                            })?;
                        let b_b_y = Fq::from_slice(
                                &input[idx * 192 + 128..idx * 192 + 160],
                            )
                            .map_err(|_| PrecompileFailure::Error {
                                exit_status: ExitError::Other(
                                    "Invalid b argument real coeff x coordinate".into(),
                                ),
                            })?;
                        let b_b_x = Fq::from_slice(
                                &input[idx * 192 + 160..idx * 192 + 192],
                            )
                            .map_err(|_| PrecompileFailure::Error {
                                exit_status: ExitError::Other(
                                    "Invalid b argument real coeff y coordinate".into(),
                                ),
                            })?;
                        let b_a = Fq2::new(b_a_x, b_a_y);
                        let b_b = Fq2::new(b_b_x, b_b_y);
                        let b = if b_a.is_zero() && b_b.is_zero() {
                            G2::zero()
                        } else {
                            G2::from(
                                AffineG2::new(b_a, b_b)
                                    .map_err(|_| PrecompileFailure::Error {
                                        exit_status: ExitError::Other(
                                            "Invalid b argument - not on curve".into(),
                                        ),
                                    })?,
                            )
                        };
                        let a = if a_x.is_zero() && a_y.is_zero() {
                            G1::zero()
                        } else {
                            G1::from(
                                AffineG1::new(a_x, a_y)
                                    .map_err(|_| PrecompileFailure::Error {
                                        exit_status: ExitError::Other(
                                            "Invalid a argument - not on curve".into(),
                                        ),
                                    })?,
                            )
                        };
                        vals.push((a, b));
                    }
                    let mul = pairing_batch(&vals);
                    if mul == Gt::one() {
                        (U256::one(), gas_cost)
                    } else {
                        (U256::zero(), gas_cost)
                    }
                };
                let mut buf = [0u8; 32];
                ret_val.to_big_endian(&mut buf);
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    cost: gas_cost,
                    output: buf.to_vec(),
                    logs: Default::default(),
                })
            }
        }
    }
    mod ecrecover {
        use super::LinearCostPrecompile;
        use crate::runner::state::PrecompileFailure;
        use module_evm_utility::evm::ExitSucceed;
        use sp_std::{cmp::min, vec::Vec};
        /// The ecrecover precompile.
        pub struct ECRecover;
        impl LinearCostPrecompile for ECRecover {
            const BASE: u64 = 3000;
            const WORD: u64 = 0;
            fn execute(
                i: &[u8],
                _: u64,
            ) -> core::result::Result<(ExitSucceed, Vec<u8>), PrecompileFailure> {
                let mut input = [0u8; 128];
                input[..min(i.len(), 128)].copy_from_slice(&i[..min(i.len(), 128)]);
                let mut msg = [0u8; 32];
                let mut sig = [0u8; 65];
                msg[0..32].copy_from_slice(&input[0..32]);
                sig[0..32].copy_from_slice(&input[64..96]);
                sig[32..64].copy_from_slice(&input[96..128]);
                sig[64] = match input[63] {
                    v if v > 26 && input[32..63] == [0; 31] => v - 27,
                    _ => {
                        return Ok((ExitSucceed::Returned, [0u8; 0].to_vec()));
                    }
                };
                let result = match sp_io::crypto::secp256k1_ecdsa_recover(&sig, &msg) {
                    Ok(pubkey) => {
                        let mut address = sp_io::hashing::keccak_256(&pubkey);
                        address[0..12].copy_from_slice(&[0u8; 12]);
                        address.to_vec()
                    }
                    Err(_) => [0u8; 0].to_vec(),
                };
                Ok((ExitSucceed::Returned, result))
            }
        }
    }
    mod ecrecover_publickey {
        use super::LinearCostPrecompile;
        use crate::runner::state::PrecompileFailure;
        use module_evm_utility::evm::{ExitError, ExitSucceed};
        use sp_std::{cmp::min, vec::Vec};
        /// The ecrecover precompile.
        pub struct ECRecoverPublicKey;
        impl LinearCostPrecompile for ECRecoverPublicKey {
            const BASE: u64 = 3000;
            const WORD: u64 = 0;
            fn execute(
                i: &[u8],
                _: u64,
            ) -> core::result::Result<(ExitSucceed, Vec<u8>), PrecompileFailure> {
                let mut input = [0u8; 128];
                input[..min(i.len(), 128)].copy_from_slice(&i[..min(i.len(), 128)]);
                let mut msg = [0u8; 32];
                let mut sig = [0u8; 65];
                msg[0..32].copy_from_slice(&input[0..32]);
                sig[0..32].copy_from_slice(&input[64..96]);
                sig[32..64].copy_from_slice(&input[96..128]);
                sig[64] = input[63];
                let pubkey = sp_io::crypto::secp256k1_ecdsa_recover(&sig, &msg)
                    .map_err(|_| PrecompileFailure::Error {
                        exit_status: ExitError::Other("Public key recover failed".into()),
                    })?;
                Ok((ExitSucceed::Returned, pubkey.to_vec()))
            }
        }
    }
    mod identity {
        use super::LinearCostPrecompile;
        use crate::runner::state::PrecompileFailure;
        use module_evm_utility::evm::ExitSucceed;
        use sp_std::vec::Vec;
        /// The identity precompile.
        pub struct Identity;
        impl LinearCostPrecompile for Identity {
            const BASE: u64 = 15;
            const WORD: u64 = 3;
            fn execute(
                input: &[u8],
                _: u64,
            ) -> core::result::Result<(ExitSucceed, Vec<u8>), PrecompileFailure> {
                Ok((ExitSucceed::Returned, input.to_vec()))
            }
        }
    }
    mod modexp {
        use super::Precompile;
        use crate::runner::state::{
            PrecompileFailure, PrecompileOutput, PrecompileResult,
        };
        use module_evm_utility::evm::{Context, ExitError, ExitSucceed};
        use num::{BigUint, One, Zero};
        use sp_core::U256;
        use sp_runtime::traits::UniqueSaturatedInto;
        use sp_std::{
            cmp::{max, min},
            vec::Vec,
        };
        const MAX_LENGTH: u64 = 1024;
        const MIN_GAS_COST: u64 = 200;
        struct ModexpPricer;
        impl ModexpPricer {
            fn adjusted_exp_len(len: u64, exp_low: U256) -> u64 {
                let bit_index = if exp_low.is_zero() {
                    0
                } else {
                    (255 - exp_low.leading_zeros()) as u64
                };
                if len <= 32 { bit_index } else { 8 * (len - 32) + bit_index }
            }
            fn mult_complexity(x: u64) -> u64 {
                match x {
                    x if x <= 64 => x * x,
                    x if x <= 1024 => (x * x) / 4 + 96 * x - 3072,
                    x => (x * x) / 16 + 480 * x - 199_680,
                }
            }
            fn read_lengths(input: &[u8]) -> (U256, U256, U256) {
                let mut input = Vec::from(input);
                if input.len() < 96 {
                    input.resize_with(96, Default::default);
                }
                let base_len = U256::from_big_endian(&input[..32]);
                let exp_len = U256::from_big_endian(&input[32..64]);
                let mod_len = U256::from_big_endian(&input[64..96]);
                (base_len, exp_len, mod_len)
            }
            fn read_exp(input: &[u8], base_len: U256, exp_len: U256) -> U256 {
                let input_len = input.len();
                let base_len = if base_len > U256::from(u32::MAX) {
                    return U256::zero();
                } else {
                    UniqueSaturatedInto::<u64>::unique_saturated_into(base_len)
                };
                if base_len + 96 >= input_len as u64 {
                    U256::zero()
                } else {
                    let exp_start = 96 + base_len as usize;
                    let remaining_len = input_len - exp_start;
                    let mut reader = Vec::from(
                        &input[exp_start..exp_start + remaining_len],
                    );
                    let len = if exp_len < U256::from(32) {
                        UniqueSaturatedInto::<usize>::unique_saturated_into(exp_len)
                    } else {
                        32
                    };
                    if reader.len() < len {
                        reader.resize_with(len, Default::default);
                    }
                    let mut buf: Vec<u8> = Vec::new();
                    buf.resize_with(32 - len, Default::default);
                    buf.extend(&reader[..min(len, remaining_len)]);
                    buf.resize_with(32, Default::default);
                    U256::from_big_endian(&buf[..])
                }
            }
            fn cost(divisor: u64, input: &[u8]) -> U256 {
                let (base_len, exp_len, mod_len) = Self::read_lengths(input);
                if mod_len.is_zero() && base_len.is_zero() {
                    return U256::zero();
                }
                let max_len = U256::from(MAX_LENGTH - 96);
                if base_len > max_len || mod_len > max_len || exp_len > max_len {
                    return U256::max_value();
                }
                let exp_low = Self::read_exp(input, base_len, exp_len);
                let (base_len, exp_len, mod_len) = (
                    base_len.unique_saturated_into(),
                    exp_len.unique_saturated_into(),
                    mod_len.unique_saturated_into(),
                );
                let m = max(mod_len, base_len);
                let adjusted_exp_len = Self::adjusted_exp_len(exp_len, exp_low);
                let (gas, overflow) = Self::mult_complexity(m)
                    .overflowing_mul(max(adjusted_exp_len, 1));
                if overflow {
                    return U256::max_value();
                }
                (gas / divisor).into()
            }
            fn eip_2565_mul_complexity(base_length: U256, modulus_length: U256) -> U256 {
                let max_length = max(base_length, modulus_length);
                let words = {
                    let tmp = max_length / 8;
                    if (max_length % 8).is_zero() { tmp } else { tmp + 1 }
                };
                words.saturating_mul(words)
            }
            fn eip_2565_iter_count(exponent_length: U256, exponent: U256) -> U256 {
                let thirty_two = U256::from(32);
                let it = if exponent_length <= thirty_two && exponent.is_zero() {
                    U256::zero()
                } else if exponent_length <= thirty_two {
                    U256::from(exponent.bits()) - U256::from(1)
                } else {
                    U256::from(8)
                        .saturating_mul(exponent_length - thirty_two)
                        .saturating_add(
                            U256::from(exponent.bits()).saturating_sub(U256::from(1)),
                        )
                };
                max(it, U256::one())
            }
            fn eip_2565_cost(
                divisor: U256,
                base_length: U256,
                modulus_length: U256,
                exponent_length: U256,
                exponent: U256,
            ) -> U256 {
                let multiplication_complexity = Self::eip_2565_mul_complexity(
                    base_length,
                    modulus_length,
                );
                let iteration_count = Self::eip_2565_iter_count(
                    exponent_length,
                    exponent,
                );
                max(
                    U256::from(MIN_GAS_COST),
                    multiplication_complexity.saturating_mul(iteration_count) / divisor,
                )
            }
        }
        pub trait ModexpImpl {
            const DIVISOR: u64;
            const EIP_2565: bool;
            fn execute_modexp(input: &[u8]) -> Vec<u8> {
                let mut reader = Vec::from(input);
                if reader.len() < 96 {
                    reader.resize_with(96, Default::default);
                }
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&reader[24..32]);
                let base_len = u64::from_be_bytes(buf);
                buf.copy_from_slice(&reader[32 + 24..64]);
                let exp_len = u64::from_be_bytes(buf);
                buf.copy_from_slice(&reader[64 + 24..96]);
                let mod_len = u64::from_be_bytes(buf);
                let r = if base_len == 0 && mod_len == 0 {
                    BigUint::zero()
                } else {
                    let total_len = 96 + base_len + exp_len + mod_len;
                    if total_len > MAX_LENGTH {
                        return [0u8; 1].to_vec();
                    }
                    let mut reader = Vec::from(input);
                    if reader.len() < total_len as usize {
                        reader.resize_with(total_len as usize, Default::default);
                    }
                    let base_end = 96 + base_len as usize;
                    let base = BigUint::from_bytes_be(&reader[96..base_end]);
                    let exp_end = base_end + exp_len as usize;
                    let exponent = BigUint::from_bytes_be(&reader[base_end..exp_end]);
                    let mod_end = exp_end + mod_len as usize;
                    let modulus = BigUint::from_bytes_be(&reader[exp_end..mod_end]);
                    if modulus.is_zero() || modulus.is_one() {
                        BigUint::zero()
                    } else {
                        base.modpow(&exponent, &modulus)
                    }
                };
                let bytes = r.to_bytes_be();
                if bytes.len() as u64 <= mod_len {
                    let mut ret = Vec::with_capacity(mod_len as usize);
                    ret.extend(
                        core::iter::repeat(0).take(mod_len as usize - bytes.len()),
                    );
                    ret.extend_from_slice(&bytes[..]);
                    ret.to_vec()
                } else {
                    [0u8; 0].to_vec()
                }
            }
        }
        pub struct IstanbulModexp;
        pub struct Modexp;
        impl ModexpImpl for IstanbulModexp {
            const DIVISOR: u64 = 20;
            const EIP_2565: bool = false;
        }
        impl ModexpImpl for Modexp {
            const DIVISOR: u64 = 3;
            const EIP_2565: bool = true;
        }
        impl Precompile for IstanbulModexp {
            fn execute(
                input: &[u8],
                target_gas: Option<u64>,
                _context: &Context,
                _is_static: bool,
            ) -> PrecompileResult {
                if input.len() as u64 > MAX_LENGTH {
                    return Err(PrecompileFailure::Error {
                        exit_status: ExitError::OutOfGas,
                    });
                }
                let cost = ModexpPricer::cost(Self::DIVISOR, input);
                if let Some(target_gas) = target_gas {
                    if cost > U256::from(u64::MAX) || target_gas < cost.as_u64() {
                        return Err(PrecompileFailure::Error {
                            exit_status: ExitError::OutOfGas,
                        });
                    }
                }
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    cost: cost.as_u64(),
                    output: Self::execute_modexp(input),
                    logs: Default::default(),
                })
            }
        }
        impl Precompile for Modexp {
            fn execute(
                input: &[u8],
                target_gas: Option<u64>,
                _context: &Context,
                _is_static: bool,
            ) -> PrecompileResult {
                if input.len() as u64 > MAX_LENGTH {
                    return Err(PrecompileFailure::Error {
                        exit_status: ExitError::OutOfGas,
                    });
                }
                if let Some(target_gas) = target_gas {
                    if target_gas < MIN_GAS_COST {
                        return Err(PrecompileFailure::Error {
                            exit_status: ExitError::OutOfGas,
                        });
                    }
                }
                let (base_len, exp_len, mod_len) = ModexpPricer::read_lengths(input);
                let exp = ModexpPricer::read_exp(input, base_len, exp_len);
                let cost = ModexpPricer::eip_2565_cost(
                    U256::from(Self::DIVISOR),
                    base_len,
                    mod_len,
                    exp_len,
                    exp,
                );
                if let Some(target_gas) = target_gas {
                    if cost > U256::from(u64::MAX) || target_gas < cost.as_u64() {
                        return Err(PrecompileFailure::Error {
                            exit_status: ExitError::OutOfGas,
                        });
                    }
                }
                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    cost: cost.as_u64(),
                    output: Self::execute_modexp(input),
                    logs: Default::default(),
                })
            }
        }
    }
    mod ripemd {
        use super::LinearCostPrecompile;
        use crate::runner::state::PrecompileFailure;
        use module_evm_utility::evm::ExitSucceed;
        use sha3::Digest;
        use sp_std::vec::Vec;
        /// The ripemd precompile.
        pub struct Ripemd160;
        impl LinearCostPrecompile for Ripemd160 {
            const BASE: u64 = 600;
            const WORD: u64 = 120;
            fn execute(
                input: &[u8],
                _cost: u64,
            ) -> core::result::Result<(ExitSucceed, Vec<u8>), PrecompileFailure> {
                let mut ret = [0u8; 32];
                ret[12..32].copy_from_slice(&ripemd160::Ripemd160::digest(input));
                Ok((ExitSucceed::Returned, ret.to_vec()))
            }
        }
    }
    mod sha256 {
        use super::LinearCostPrecompile;
        use crate::runner::state::PrecompileFailure;
        use module_evm_utility::evm::ExitSucceed;
        use sp_std::vec::Vec;
        /// The sha256 precompile.
        pub struct Sha256;
        impl LinearCostPrecompile for Sha256 {
            const BASE: u64 = 60;
            const WORD: u64 = 12;
            fn execute(
                input: &[u8],
                _cost: u64,
            ) -> core::result::Result<(ExitSucceed, Vec<u8>), PrecompileFailure> {
                let ret = sp_io::hashing::sha2_256(input);
                Ok((ExitSucceed::Returned, ret.to_vec()))
            }
        }
    }
    mod sha3fips {
        use super::LinearCostPrecompile;
        use crate::runner::state::PrecompileFailure;
        use module_evm_utility::evm::ExitSucceed;
        use sp_std::vec::Vec;
        use tiny_keccak::Hasher;
        /// The Sha3FIPS256 precompile.
        pub struct Sha3FIPS256;
        impl LinearCostPrecompile for Sha3FIPS256 {
            const BASE: u64 = 60;
            const WORD: u64 = 12;
            fn execute(
                input: &[u8],
                _: u64,
            ) -> core::result::Result<(ExitSucceed, Vec<u8>), PrecompileFailure> {
                let mut output = [0; 32];
                let mut sha3 = tiny_keccak::Sha3::v256();
                sha3.update(input);
                sha3.finalize(&mut output);
                Ok((ExitSucceed::Returned, output.to_vec()))
            }
        }
        /// The Sha3FIPS512 precompile.
        pub struct Sha3FIPS512;
        impl LinearCostPrecompile for Sha3FIPS512 {
            const BASE: u64 = 60;
            const WORD: u64 = 12;
            fn execute(
                input: &[u8],
                _: u64,
            ) -> core::result::Result<(ExitSucceed, Vec<u8>), PrecompileFailure> {
                let mut output = [0; 64];
                let mut sha3 = tiny_keccak::Sha3::v512();
                sha3.update(input);
                sha3.finalize(&mut output);
                Ok((ExitSucceed::Returned, output.to_vec()))
            }
        }
    }
    pub use blake2::Blake2F;
    pub use bn128::{Bn128Add, Bn128Mul, Bn128Pairing};
    pub use ecrecover::ECRecover;
    pub use ecrecover_publickey::ECRecoverPublicKey;
    pub use identity::Identity;
    pub use modexp::{IstanbulModexp, Modexp};
    pub use ripemd::Ripemd160;
    pub use sha256::Sha256;
    pub use sha3fips::{Sha3FIPS256, Sha3FIPS512};
    /// One single precompile used by EVM engine.
    pub trait Precompile {
        /// Try to execute the precompile. Calculate the amount of gas needed with given `input` and
        /// `target_gas`. Return `Ok(status, output, gas_used)` if the execution is
        /// successful. Otherwise return `Err(_)`.
        fn execute(
            input: &[u8],
            target_gas: Option<u64>,
            context: &Context,
            is_static: bool,
        ) -> PrecompileResult;
    }
    pub trait LinearCostPrecompile {
        const BASE: u64;
        const WORD: u64;
        fn execute(
            input: &[u8],
            cost: u64,
        ) -> core::result::Result<(ExitSucceed, Vec<u8>), PrecompileFailure>;
    }
    impl<T: LinearCostPrecompile> Precompile for T {
        fn execute(
            input: &[u8],
            target_gas: Option<u64>,
            _: &Context,
            _: bool,
        ) -> PrecompileResult {
            let cost = ensure_linear_cost(
                target_gas,
                input.len() as u64,
                T::BASE,
                T::WORD,
            )?;
            let (exit_status, output) = T::execute(input, cost)?;
            Ok(PrecompileOutput {
                exit_status,
                cost,
                output,
                logs: Default::default(),
            })
        }
    }
    /// Linear gas cost
    fn ensure_linear_cost(
        target_gas: Option<u64>,
        len: u64,
        base: u64,
        word: u64,
    ) -> Result<u64, PrecompileFailure> {
        let cost = base
            .checked_add(
                word
                    .checked_mul(len.saturating_add(31) / 32)
                    .ok_or(PrecompileFailure::Error {
                        exit_status: ExitError::OutOfGas,
                    })?,
            )
            .ok_or(PrecompileFailure::Error {
                exit_status: ExitError::OutOfGas,
            })?;
        if let Some(target_gas) = target_gas {
            if cost > target_gas {
                return Err(PrecompileFailure::Error {
                    exit_status: ExitError::OutOfGas,
                });
            }
        }
        Ok(cost)
    }
}
pub mod runner {
    pub mod stack {
        //! EVM stack-based runner.
        use crate::{
            runner::{
                state::{
                    Accessed, CustomStackState, StackExecutor, StackState as StackStateT,
                    StackSubstateMetadata,
                },
                Runner as RunnerT, RunnerExtended,
            },
            AccountInfo, AccountStorages, Accounts, BalanceOf, CallInfo, Config,
            CreateInfo, Error, ExecutionInfo, One, Pallet, STORAGE_SIZE,
        };
        use frame_support::{
            dispatch::DispatchError, ensure, log,
            traits::{Currency, ExistenceRequirement, Get},
            transactional,
        };
        use module_evm_utility::{
            ethereum::Log,
            evm::{self, backend::Backend as BackendT, ExitError, ExitReason, Transfer},
        };
        use module_support::{AddressMapping, EVM};
        pub use primitives::{
            evm::{
                convert_decimals_from_evm, EvmAddress, Vicinity,
                MIRRORED_NFT_ADDRESS_START,
            },
            ReserveIdentifier,
        };
        use sp_core::{defer, H160, H256, U256};
        use sp_runtime::traits::{UniqueSaturatedInto, Zero};
        use sp_std::{
            boxed::Box, collections::{btree_map::BTreeMap, btree_set::BTreeSet},
            marker::PhantomData, mem, vec::Vec,
        };
        pub struct Runner<T: Config> {
            _marker: PhantomData<T>,
        }
        #[automatically_derived]
        impl<T: ::core::default::Default + Config> ::core::default::Default
        for Runner<T> {
            #[inline]
            fn default() -> Runner<T> {
                Runner {
                    _marker: ::core::default::Default::default(),
                }
            }
        }
        impl<T: Config> Runner<T> {
            /// Execute an EVM operation.
            pub fn execute<'config, 'precompiles, F, R>(
                source: H160,
                origin: H160,
                value: U256,
                gas_limit: u64,
                storage_limit: u32,
                config: &'config evm::Config,
                skip_storage_rent: bool,
                precompiles: &'precompiles T::PrecompilesType,
                f: F,
            ) -> Result<ExecutionInfo<R>, sp_runtime::DispatchError>
            where
                F: FnOnce(
                    &mut StackExecutor<
                        'config,
                        'precompiles,
                        SubstrateStackState<'_, 'config, T>,
                        T::PrecompilesType,
                    >,
                ) -> (ExitReason, R),
            {
                let gas_price = U256::one();
                let vicinity = Vicinity {
                    gas_price,
                    origin,
                    ..Default::default()
                };
                let metadata = StackSubstateMetadata::new(
                    gas_limit,
                    storage_limit,
                    config,
                );
                let state = SubstrateStackState::new(&vicinity, metadata);
                let mut executor = StackExecutor::new_with_precompiles(
                    state,
                    config,
                    precompiles,
                );
                {
                    if !convert_decimals_from_evm(
                            TryInto::<BalanceOf<T>>::try_into(value)
                                .map_err(|_| Error::<T>::InvalidDecimals)?,
                        )
                        .is_some()
                    {
                        { return Err(Error::<T>::InvalidDecimals.into()) };
                    }
                };
                if !skip_storage_rent {
                    Pallet::<T>::reserve_storage(&origin, storage_limit)
                        .map_err(|e| {
                            {
                                let lvl = ::log::Level::Debug;
                                if lvl <= ::log::STATIC_MAX_LEVEL
                                    && lvl <= ::log::max_level()
                                {
                                    ::log::__private_api_log(
                                        format_args!(
                                            "ReserveStorageFailed {0:?} [source: {1:?}, storage_limit: {2:?}]",
                                            e, origin, storage_limit
                                        ),
                                        lvl,
                                        &(
                                            "evm",
                                            "module_evm::runner::stack",
                                            "modules/evm/src/runner/stack.rs",
                                            99u32,
                                        ),
                                        ::log::__private_api::Option::None,
                                    );
                                }
                            };
                            Error::<T>::ReserveStorageFailed
                        })?;
                }
                let (reason, retv) = f(&mut executor);
                let used_gas = U256::from(executor.used_gas());
                {
                    let lvl = ::log::Level::Debug;
                    if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                        ::log::__private_api_log(
                            format_args!(
                                "Execution {0:?} [source: {1:?}, value: {2}, gas_limit: {3}, used_gas: {4}]",
                                reason, source, value, gas_limit, used_gas
                            ),
                            lvl,
                            &(
                                "evm",
                                "module_evm::runner::stack",
                                "modules/evm/src/runner/stack.rs",
                                114u32,
                            ),
                            ::log::__private_api::Option::None,
                        );
                    }
                };
                let state = executor.into_state();
                let actual_storage = state
                    .metadata()
                    .storage_meter()
                    .finish()
                    .ok_or(Error::<T>::OutOfStorage)?;
                let used_storage = state.metadata().storage_meter().total_used();
                let refunded_storage = state.metadata().storage_meter().total_refunded();
                {
                    let lvl = ::log::Level::Debug;
                    if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                        ::log::__private_api_log(
                            format_args!(
                                "Storage logs: {0:?}", state.substate.storage_logs
                            ),
                            lvl,
                            &(
                                "evm",
                                "module_evm::runner::stack",
                                "modules/evm/src/runner/stack.rs",
                                134u32,
                            ),
                            ::log::__private_api::Option::None,
                        );
                    }
                };
                let mut sum_storage: i32 = 0;
                for (target, storage) in &state
                    .substate
                    .storage_logs
                    .into_iter()
                    .fold(
                        BTreeMap::<H160, i32>::new(),
                        |mut bmap, (target, storage)| {
                            bmap.entry(target)
                                .and_modify(|x| *x = x.saturating_add(storage))
                                .or_insert(storage);
                            bmap
                        },
                    )
                {
                    if !skip_storage_rent {
                        Pallet::<T>::charge_storage(&origin, target, *storage)
                            .map_err(|e| {
                                {
                                    let lvl = ::log::Level::Debug;
                                    if lvl <= ::log::STATIC_MAX_LEVEL
                                        && lvl <= ::log::max_level()
                                    {
                                        ::log::__private_api_log(
                                            format_args!(
                                                "ChargeStorageFailed {0:?} [source: {1:?}, target: {2:?}, storage: {3:?}]",
                                                e, origin, target, storage
                                            ),
                                            lvl,
                                            &(
                                                "evm",
                                                "module_evm::runner::stack",
                                                "modules/evm/src/runner/stack.rs",
                                                151u32,
                                            ),
                                            ::log::__private_api::Option::None,
                                        );
                                    }
                                };
                                Error::<T>::ChargeStorageFailed
                            })?;
                    }
                    sum_storage = sum_storage.saturating_add(*storage);
                }
                if actual_storage != sum_storage {
                    {
                        let lvl = ::log::Level::Debug;
                        if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                            ::log::__private_api_log(
                                format_args!(
                                    "ChargeStorageFailed [actual_storage: {0:?}, sum_storage: {1:?}]",
                                    actual_storage, sum_storage
                                ),
                                lvl,
                                &(
                                    "evm",
                                    "module_evm::runner::stack",
                                    "modules/evm/src/runner/stack.rs",
                                    165u32,
                                ),
                                ::log::__private_api::Option::None,
                            );
                        }
                    };
                    return Err(Error::<T>::ChargeStorageFailed.into());
                }
                if !skip_storage_rent {
                    Pallet::<
                        T,
                    >::unreserve_storage(
                            &origin,
                            storage_limit,
                            used_storage,
                            refunded_storage,
                        )
                        .map_err(|e| {
                            {
                                let lvl = ::log::Level::Debug;
                                if lvl <= ::log::STATIC_MAX_LEVEL
                                    && lvl <= ::log::max_level()
                                {
                                    ::log::__private_api_log(
                                        format_args!(
                                            "UnreserveStorageFailed {0:?} [source: {1:?}, storage_limit: {2:?}, used_storage: {3:?}, refunded_storage: {4:?}]",
                                            e, origin, storage_limit, used_storage, refunded_storage
                                        ),
                                        lvl,
                                        &(
                                            "evm",
                                            "module_evm::runner::stack",
                                            "modules/evm/src/runner/stack.rs",
                                            175u32,
                                        ),
                                        ::log::__private_api::Option::None,
                                    );
                                }
                            };
                            Error::<T>::UnreserveStorageFailed
                        })?;
                }
                for address in state.substate.deletes {
                    {
                        let lvl = ::log::Level::Debug;
                        if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                            ::log::__private_api_log(
                                format_args!("Deleting account at {0:?}", address),
                                lvl,
                                &(
                                    "evm",
                                    "module_evm::runner::stack",
                                    "modules/evm/src/runner/stack.rs",
                                    189u32,
                                ),
                                ::log::__private_api::Option::None,
                            );
                        }
                    };
                    Pallet::<T>::remove_contract(&origin, &address)
                        .map_err(|e| {
                            {
                                let lvl = ::log::Level::Debug;
                                if lvl <= ::log::STATIC_MAX_LEVEL
                                    && lvl <= ::log::max_level()
                                {
                                    ::log::__private_api_log(
                                        format_args!(
                                            "CannotKillContract address {0:?}, reason: {1:?}", address,
                                            e
                                        ),
                                        lvl,
                                        &(
                                            "evm",
                                            "module_evm::runner::stack",
                                            "modules/evm/src/runner/stack.rs",
                                            195u32,
                                        ),
                                        ::log::__private_api::Option::None,
                                    );
                                }
                            };
                            Error::<T>::CannotKillContract
                        })?;
                }
                {
                    let lvl = ::log::Level::Debug;
                    if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                        ::log::__private_api_log(
                            format_args!("Execution logs {0:?}", state.substate.logs),
                            lvl,
                            &(
                                "evm",
                                "module_evm::runner::stack",
                                "modules/evm/src/runner/stack.rs",
                                205u32,
                            ),
                            ::log::__private_api::Option::None,
                        );
                    }
                };
                Ok(ExecutionInfo {
                    value: retv,
                    exit_reason: reason,
                    used_gas,
                    used_storage: actual_storage,
                    logs: state.substate.logs,
                })
            }
        }
        impl<T: Config> RunnerT<T> for Runner<T> {
            /// Require transactional here. Always need to send events.
            fn call(
                source: H160,
                origin: H160,
                target: H160,
                input: Vec<u8>,
                value: BalanceOf<T>,
                gas_limit: u64,
                storage_limit: u32,
                access_list: Vec<(H160, Vec<H256>)>,
                config: &evm::Config,
            ) -> Result<CallInfo, DispatchError> {
                use frame_support::storage::{with_transaction, TransactionOutcome};
                with_transaction(|| {
                    let r = (|| {
                        {
                            {
                                if !Pallet::<T>::can_call_contract(&target, &source) {
                                    { return Err(Error::<T>::NoPermission.into()) };
                                }
                            };
                            let precompiles = T::PrecompilesValue::get();
                            let value = U256::from(
                                UniqueSaturatedInto::<u128>::unique_saturated_into(value),
                            );
                            Self::execute(
                                source,
                                origin,
                                value,
                                gas_limit,
                                storage_limit,
                                config,
                                false,
                                &precompiles,
                                |executor| {
                                    executor
                                        .transact_call(
                                            source,
                                            target,
                                            value,
                                            input,
                                            gas_limit,
                                            access_list,
                                        )
                                },
                            )
                        }
                    })();
                    if r.is_ok() {
                        TransactionOutcome::Commit(r)
                    } else {
                        TransactionOutcome::Rollback(r)
                    }
                })
            }
            /// Require transactional here. Always need to send events.
            fn create(
                source: H160,
                init: Vec<u8>,
                value: BalanceOf<T>,
                gas_limit: u64,
                storage_limit: u32,
                access_list: Vec<(H160, Vec<H256>)>,
                config: &evm::Config,
            ) -> Result<CreateInfo, DispatchError> {
                use frame_support::storage::{with_transaction, TransactionOutcome};
                with_transaction(|| {
                    let r = (|| {
                        {
                            let precompiles = T::PrecompilesValue::get();
                            let value = U256::from(
                                UniqueSaturatedInto::<u128>::unique_saturated_into(value),
                            );
                            Self::execute(
                                source,
                                source,
                                value,
                                gas_limit,
                                storage_limit,
                                config,
                                false,
                                &precompiles,
                                |executor| {
                                    let address = executor
                                        .create_address(evm::CreateScheme::Legacy {
                                            caller: source,
                                        })
                                        .unwrap_or_default();
                                    let (reason, _) = executor
                                        .transact_create(
                                            source,
                                            value,
                                            init,
                                            gas_limit,
                                            access_list,
                                        );
                                    (reason, address)
                                },
                            )
                        }
                    })();
                    if r.is_ok() {
                        TransactionOutcome::Commit(r)
                    } else {
                        TransactionOutcome::Rollback(r)
                    }
                })
            }
            /// Require transactional here. Always need to send events.
            fn create2(
                source: H160,
                init: Vec<u8>,
                salt: H256,
                value: BalanceOf<T>,
                gas_limit: u64,
                storage_limit: u32,
                access_list: Vec<(H160, Vec<H256>)>,
                config: &evm::Config,
            ) -> Result<CreateInfo, DispatchError> {
                use frame_support::storage::{with_transaction, TransactionOutcome};
                with_transaction(|| {
                    let r = (|| {
                        {
                            let precompiles = T::PrecompilesValue::get();
                            let value = U256::from(
                                UniqueSaturatedInto::<u128>::unique_saturated_into(value),
                            );
                            let code_hash = H256::from(
                                sp_io::hashing::keccak_256(&init),
                            );
                            Self::execute(
                                source,
                                source,
                                value,
                                gas_limit,
                                storage_limit,
                                config,
                                false,
                                &precompiles,
                                |executor| {
                                    let address = executor
                                        .create_address(evm::CreateScheme::Create2 {
                                            caller: source,
                                            code_hash,
                                            salt,
                                        })
                                        .unwrap_or_default();
                                    let (reason, _) = executor
                                        .transact_create2(
                                            source,
                                            value,
                                            init,
                                            salt,
                                            gas_limit,
                                            access_list,
                                        );
                                    (reason, address)
                                },
                            )
                        }
                    })();
                    if r.is_ok() {
                        TransactionOutcome::Commit(r)
                    } else {
                        TransactionOutcome::Rollback(r)
                    }
                })
            }
            /// Require transactional here. Always need to send events.
            fn create_at_address(
                source: H160,
                address: H160,
                init: Vec<u8>,
                value: BalanceOf<T>,
                gas_limit: u64,
                storage_limit: u32,
                access_list: Vec<(H160, Vec<H256>)>,
                config: &evm::Config,
            ) -> Result<CreateInfo, DispatchError> {
                use frame_support::storage::{with_transaction, TransactionOutcome};
                with_transaction(|| {
                    let r = (|| {
                        {
                            let precompiles = T::PrecompilesValue::get();
                            let value = U256::from(
                                UniqueSaturatedInto::<u128>::unique_saturated_into(value),
                            );
                            Self::execute(
                                source,
                                source,
                                value,
                                gas_limit,
                                storage_limit,
                                config,
                                false,
                                &precompiles,
                                |executor| {
                                    let (reason, _) = executor
                                        .transact_create_at_address(
                                            source,
                                            address,
                                            value,
                                            init,
                                            gas_limit,
                                            access_list,
                                        );
                                    (reason, address)
                                },
                            )
                        }
                    })();
                    if r.is_ok() {
                        TransactionOutcome::Commit(r)
                    } else {
                        TransactionOutcome::Rollback(r)
                    }
                })
            }
        }
        impl<T: Config> RunnerExtended<T> for Runner<T> {
            /// Special method for rpc call which won't charge for storage rent
            /// Same as call but with skip_storage_rent: true
            fn rpc_call(
                source: H160,
                origin: H160,
                target: H160,
                input: Vec<u8>,
                value: BalanceOf<T>,
                gas_limit: u64,
                storage_limit: u32,
                access_list: Vec<(H160, Vec<H256>)>,
                config: &evm::Config,
            ) -> Result<CallInfo, DispatchError> {
                Pallet::<T>::set_origin(T::AddressMapping::get_account_id(&origin));
                let _guard = ::sp_core::defer::DeferGuard(
                    Some(|| { Pallet::<T>::kill_origin() }),
                );
                let precompiles = T::PrecompilesValue::get();
                let value = U256::from(
                    UniqueSaturatedInto::<u128>::unique_saturated_into(value),
                );
                Self::execute(
                    source,
                    origin,
                    value,
                    gas_limit,
                    storage_limit,
                    config,
                    true,
                    &precompiles,
                    |executor| {
                        executor
                            .transact_call(
                                source,
                                target,
                                value,
                                input,
                                gas_limit,
                                access_list,
                            )
                    },
                )
            }
            /// Special method for rpc create which won't charge for storage rent
            /// Same as create but with skip_storage_rent: true
            fn rpc_create(
                source: H160,
                init: Vec<u8>,
                value: BalanceOf<T>,
                gas_limit: u64,
                storage_limit: u32,
                access_list: Vec<(H160, Vec<H256>)>,
                config: &evm::Config,
            ) -> Result<CreateInfo, DispatchError> {
                let precompiles = T::PrecompilesValue::get();
                let value = U256::from(
                    UniqueSaturatedInto::<u128>::unique_saturated_into(value),
                );
                Self::execute(
                    source,
                    source,
                    value,
                    gas_limit,
                    storage_limit,
                    config,
                    true,
                    &precompiles,
                    |executor| {
                        let address = executor
                            .create_address(evm::CreateScheme::Legacy {
                                caller: source,
                            })
                            .unwrap_or_default();
                        let (reason, _) = executor
                            .transact_create(
                                source,
                                value,
                                init,
                                gas_limit,
                                access_list,
                            );
                        (reason, address)
                    },
                )
            }
        }
        struct SubstrateStackSubstate<'config> {
            metadata: StackSubstateMetadata<'config>,
            deletes: BTreeSet<H160>,
            logs: Vec<Log>,
            storage_logs: Vec<(H160, i32)>,
            parent: Option<Box<SubstrateStackSubstate<'config>>>,
            known_original_storage: BTreeMap<(H160, H256), H256>,
        }
        impl<'config> SubstrateStackSubstate<'config> {
            pub fn metadata(&self) -> &StackSubstateMetadata<'config> {
                &self.metadata
            }
            pub fn metadata_mut(&mut self) -> &mut StackSubstateMetadata<'config> {
                &mut self.metadata
            }
            pub fn enter(&mut self, gas_limit: u64, is_static: bool) {
                let mut entering = Self {
                    metadata: self.metadata.spit_child(gas_limit, is_static),
                    parent: None,
                    deletes: BTreeSet::new(),
                    logs: Vec::new(),
                    storage_logs: Vec::new(),
                    known_original_storage: BTreeMap::new(),
                };
                mem::swap(&mut entering, self);
                self.parent = Some(Box::new(entering));
                sp_io::storage::start_transaction();
            }
            pub fn exit_commit(&mut self) -> Result<(), ExitError> {
                let mut exited = *self
                    .parent
                    .take()
                    .expect("Cannot commit on root substate");
                mem::swap(&mut exited, self);
                let target = self.metadata().target().expect("Storage target is none");
                let storage = exited.metadata().storage_meter().used_storage();
                self.metadata
                    .swallow_commit(exited.metadata)
                    .map_err(|e| {
                        sp_io::storage::rollback_transaction();
                        e
                    })?;
                self.logs.append(&mut exited.logs);
                self.deletes.append(&mut exited.deletes);
                exited.storage_logs.push((target, storage));
                self.storage_logs.append(&mut exited.storage_logs);
                sp_io::storage::commit_transaction();
                Ok(())
            }
            pub fn exit_revert(&mut self) -> Result<(), ExitError> {
                let mut exited = *self
                    .parent
                    .take()
                    .expect("Cannot discard on root substate");
                mem::swap(&mut exited, self);
                self.metadata
                    .swallow_revert(exited.metadata)
                    .map_err(|e| {
                        sp_io::storage::rollback_transaction();
                        e
                    })?;
                sp_io::storage::rollback_transaction();
                Ok(())
            }
            pub fn exit_discard(&mut self) -> Result<(), ExitError> {
                let mut exited = *self
                    .parent
                    .take()
                    .expect("Cannot discard on root substate");
                mem::swap(&mut exited, self);
                self.metadata
                    .swallow_discard(exited.metadata)
                    .map_err(|e| {
                        sp_io::storage::rollback_transaction();
                        e
                    })?;
                sp_io::storage::rollback_transaction();
                Ok(())
            }
            pub fn deleted(&self, address: H160) -> bool {
                if self.deletes.contains(&address) {
                    return true;
                }
                if let Some(parent) = self.parent.as_ref() {
                    return parent.deleted(address);
                }
                false
            }
            pub fn set_deleted(&mut self, address: H160) {
                self.deletes.insert(address);
            }
            pub fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) {
                self.logs.push(Log { address, topics, data });
            }
            fn recursive_is_cold<F: Fn(&Accessed) -> bool>(&self, f: &F) -> bool {
                let local_is_accessed = self
                    .metadata
                    .accessed()
                    .as_ref()
                    .map(f)
                    .unwrap_or(false);
                if local_is_accessed {
                    false
                } else {
                    self.parent.as_ref().map(|p| p.recursive_is_cold(f)).unwrap_or(true)
                }
            }
            pub fn known_original_storage(
                &self,
                address: H160,
                index: H256,
            ) -> Option<H256> {
                if let Some(parent) = self.parent.as_ref() {
                    return parent.known_original_storage(address, index);
                }
                self.known_original_storage.get(&(address, index)).copied()
            }
            pub fn set_known_original_storage(
                &mut self,
                address: H160,
                index: H256,
                value: H256,
            ) {
                if let Some(ref mut parent) = self.parent {
                    return parent.set_known_original_storage(address, index, value);
                }
                self.known_original_storage.insert((address, index), value);
            }
        }
        /// Substrate backend for EVM.
        pub struct SubstrateStackState<'vicinity, 'config, T> {
            vicinity: &'vicinity Vicinity,
            substate: SubstrateStackSubstate<'config>,
            _marker: PhantomData<T>,
        }
        impl<'vicinity, 'config, T: Config> SubstrateStackState<'vicinity, 'config, T> {
            /// Create a new backend with given vicinity.
            pub fn new(
                vicinity: &'vicinity Vicinity,
                metadata: StackSubstateMetadata<'config>,
            ) -> Self {
                Self {
                    vicinity,
                    substate: SubstrateStackSubstate {
                        metadata,
                        deletes: BTreeSet::new(),
                        logs: Vec::new(),
                        storage_logs: Vec::new(),
                        parent: None,
                        known_original_storage: BTreeMap::new(),
                    },
                    _marker: PhantomData,
                }
            }
        }
        impl<'vicinity, 'config, T: Config> BackendT
        for SubstrateStackState<'vicinity, 'config, T> {
            fn gas_price(&self) -> U256 {
                self.vicinity.gas_price
            }
            fn origin(&self) -> H160 {
                self.vicinity.origin
            }
            fn block_hash(&self, number: U256) -> H256 {
                if number > U256::from(u32::MAX) {
                    H256::default()
                } else {
                    let number = T::BlockNumber::from(number.as_u32());
                    H256::from_slice(
                        frame_system::Pallet::<T>::block_hash(number).as_ref(),
                    )
                }
            }
            fn block_number(&self) -> U256 {
                let number: u128 = frame_system::Pallet::<T>::block_number()
                    .unique_saturated_into();
                U256::from(number)
            }
            fn block_coinbase(&self) -> H160 {
                self.vicinity.block_coinbase.unwrap_or(Pallet::<T>::find_author())
            }
            fn block_timestamp(&self) -> U256 {
                let now: u128 = pallet_timestamp::Pallet::<T>::get()
                    .unique_saturated_into();
                U256::from(now / 1000)
            }
            fn block_difficulty(&self) -> U256 {
                self.vicinity.block_difficulty.unwrap_or_default()
            }
            fn block_gas_limit(&self) -> U256 {
                self.vicinity.block_gas_limit.unwrap_or_default()
            }
            fn chain_id(&self) -> U256 {
                U256::from(Pallet::<T>::chain_id())
            }
            #[cfg(not(feature = "evm-tests"))]
            fn exists(&self, _address: H160) -> bool {
                true
            }
            fn basic(&self, address: H160) -> evm::backend::Basic {
                let account = Pallet::<T>::account_basic(&address);
                evm::backend::Basic {
                    balance: account.balance,
                    nonce: account.nonce,
                }
            }
            fn code(&self, address: H160) -> Vec<u8> {
                Pallet::<T>::code_at_address(&address).into_inner()
            }
            fn storage(&self, address: H160, index: H256) -> H256 {
                AccountStorages::<T>::get(address, index)
            }
            fn original_storage(&self, address: H160, index: H256) -> Option<H256> {
                if let Some(value) = self.substate.known_original_storage(address, index)
                {
                    Some(value)
                } else {
                    Some(self.storage(address, index))
                }
            }
            fn block_base_fee_per_gas(&self) -> sp_core::U256 {
                self.vicinity.block_base_fee_per_gas.unwrap_or(U256::one())
            }
        }
        impl<'vicinity, 'config, T: Config> StackStateT<'config>
        for SubstrateStackState<'vicinity, 'config, T> {
            fn metadata(&self) -> &StackSubstateMetadata<'config> {
                self.substate.metadata()
            }
            fn metadata_mut(&mut self) -> &mut StackSubstateMetadata<'config> {
                self.substate.metadata_mut()
            }
            fn enter(&mut self, gas_limit: u64, is_static: bool) {
                self.substate.enter(gas_limit, is_static)
            }
            fn exit_commit(&mut self) -> Result<(), ExitError> {
                self.substate.exit_commit()
            }
            fn exit_revert(&mut self) -> Result<(), ExitError> {
                self.substate.exit_revert()
            }
            fn exit_discard(&mut self) -> Result<(), ExitError> {
                self.substate.exit_discard()
            }
            fn is_empty(&self, address: H160) -> bool {
                Pallet::<T>::is_account_empty(&address)
            }
            fn deleted(&self, address: H160) -> bool {
                self.substate.deleted(address)
            }
            fn inc_nonce(&mut self, address: H160) {
                Accounts::<
                    T,
                >::mutate(
                    address,
                    |maybe_account| {
                        if let Some(account) = maybe_account.as_mut() {
                            account.nonce += One::one()
                        } else {
                            let mut account_info = <AccountInfo<
                                T::Index,
                            >>::new(Default::default(), None);
                            account_info.nonce += One::one();
                            *maybe_account = Some(account_info);
                        }
                    },
                );
            }
            fn set_storage(&mut self, address: H160, index: H256, value: H256) {
                let current = <AccountStorages<T>>::get(address, index);
                if self.substate.known_original_storage(address, index).is_none() {
                    self.substate.set_known_original_storage(address, index, current);
                }
                if value == H256::default() {
                    {
                        let lvl = ::log::Level::Debug;
                        if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                            ::log::__private_api_log(
                                format_args!(
                                    "Removing storage for {0:?} [index: {1:?}]", address, index
                                ),
                                lvl,
                                &(
                                    "evm",
                                    "module_evm::runner::stack",
                                    "modules/evm/src/runner/stack.rs",
                                    751u32,
                                ),
                                ::log::__private_api::Option::None,
                            );
                        }
                    };
                    <AccountStorages<T>>::remove(address, index);
                    if !current.is_zero() {
                        Pallet::<
                            T,
                        >::update_contract_storage_size(
                            &address,
                            -(STORAGE_SIZE as i32),
                        );
                        self.substate.metadata.storage_meter_mut().refund(STORAGE_SIZE);
                    }
                } else {
                    {
                        let lvl = ::log::Level::Debug;
                        if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                            ::log::__private_api_log(
                                format_args!(
                                    "Updating storage for {0:?} [index: {1:?}, value: {2:?}]",
                                    address, index, value
                                ),
                                lvl,
                                &(
                                    "evm",
                                    "module_evm::runner::stack",
                                    "modules/evm/src/runner/stack.rs",
                                    765u32,
                                ),
                                ::log::__private_api::Option::None,
                            );
                        }
                    };
                    <AccountStorages<T>>::insert(address, index, value);
                    if current.is_zero() {
                        Pallet::<
                            T,
                        >::update_contract_storage_size(&address, STORAGE_SIZE as i32);
                        self.substate.metadata.storage_meter_mut().charge(STORAGE_SIZE);
                    }
                }
            }
            fn reset_storage(&mut self, address: H160) {
                let _ = <AccountStorages<T>>::clear_prefix(address, u32::MAX, None);
            }
            fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) {
                self.substate.log(address, topics, data)
            }
            fn set_deleted(&mut self, address: H160) {
                self.substate.set_deleted(address)
            }
            fn set_code(&mut self, address: H160, code: Vec<u8>) {
                {
                    let lvl = ::log::Level::Debug;
                    if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                        ::log::__private_api_log(
                            format_args!(
                                "Inserting code ({0} bytes) at {1:?}", code.len(), address
                            ),
                            lvl,
                            &(
                                "evm",
                                "module_evm::runner::stack",
                                "modules/evm/src/runner/stack.rs",
                                795u32,
                            ),
                            ::log::__private_api::Option::None,
                        );
                    }
                };
                let parent = match self.substate.parent {
                    Some(ref parent) => parent,
                    None => {
                        {
                            let lvl = ::log::Level::Error;
                            if lvl <= ::log::STATIC_MAX_LEVEL
                                && lvl <= ::log::max_level()
                            {
                                ::log::__private_api_log(
                                    format_args!(
                                        "get parent\'s maintainer failed. address: {0:?}", address
                                    ),
                                    lvl,
                                    &(
                                        "evm",
                                        "module_evm::runner::stack",
                                        "modules/evm/src/runner/stack.rs",
                                        806u32,
                                    ),
                                    ::log::__private_api::Option::None,
                                );
                            }
                        };
                        if true {
                            if !false {
                                ::core::panicking::panic("assertion failed: false")
                            }
                        }
                        return;
                    }
                };
                let caller = match parent.metadata().caller() {
                    Some(ref caller) => caller,
                    None => {
                        {
                            let lvl = ::log::Level::Error;
                            if lvl <= ::log::STATIC_MAX_LEVEL
                                && lvl <= ::log::max_level()
                            {
                                ::log::__private_api_log(
                                    format_args!(
                                        "get parent\'s caller failed. address: {0:?}", address
                                    ),
                                    lvl,
                                    &(
                                        "evm",
                                        "module_evm::runner::stack",
                                        "modules/evm/src/runner/stack.rs",
                                        819u32,
                                    ),
                                    ::log::__private_api::Option::None,
                                );
                            }
                        };
                        if true {
                            if !false {
                                ::core::panicking::panic("assertion failed: false")
                            }
                        }
                        return;
                    }
                };
                let is_published = self
                    .substate
                    .metadata
                    .origin_code_address()
                    .map_or(
                        false,
                        |addr| {
                            Pallet::<T>::accounts(addr)
                                .map_or(
                                    false,
                                    |account| {
                                        account.contract_info.map_or(false, |v| v.published)
                                    },
                                )
                        },
                    );
                {
                    let lvl = ::log::Level::Debug;
                    if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                        ::log::__private_api_log(
                            format_args!(
                                "set_code: address: {0:?}, maintainer: {1:?}, publish: {2:?}",
                                address, caller, is_published
                            ),
                            lvl,
                            &(
                                "evm",
                                "module_evm::runner::stack",
                                "modules/evm/src/runner/stack.rs",
                                833u32,
                            ),
                            ::log::__private_api::Option::None,
                        );
                    }
                };
                let code_size = code.len() as u32;
                Pallet::<T>::create_contract(*caller, address, is_published, code);
                let used_storage = code_size
                    .saturating_add(T::NewContractExtraBytes::get());
                Pallet::<T>::update_contract_storage_size(&address, used_storage as i32);
                self.substate.metadata.storage_meter_mut().charge(used_storage);
            }
            fn transfer(&mut self, transfer: Transfer) -> Result<(), ExitError> {
                self.touch(transfer.target);
                if transfer.value.is_zero() {
                    return Ok(());
                }
                let source = T::AddressMapping::get_account_id(&transfer.source);
                let target = T::AddressMapping::get_account_id(&transfer.target);
                let amount = convert_decimals_from_evm(
                        TryInto::<BalanceOf<T>>::try_into(transfer.value)
                            .map_err(|_| ExitError::OutOfFund)?,
                    )
                    .ok_or(
                        ExitError::Other(
                            Into::<&str>::into(Error::<T>::InvalidDecimals).into(),
                        ),
                    )?;
                {
                    let lvl = ::log::Level::Debug;
                    if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                        ::log::__private_api_log(
                            format_args!(
                                "transfer [source: {0:?}, target: {1:?}, amount: {2:?}]",
                                source, target, amount
                            ),
                            lvl,
                            &(
                                "evm",
                                "module_evm::runner::stack",
                                "modules/evm/src/runner/stack.rs",
                                861u32,
                            ),
                            ::log::__private_api::Option::None,
                        );
                    }
                };
                if T::Currency::free_balance(&source) < amount {
                    return Err(ExitError::OutOfFund);
                }
                T::Currency::transfer(
                        &source,
                        &target,
                        amount,
                        ExistenceRequirement::AllowDeath,
                    )
                    .map_err(|e| ExitError::Other(Into::<&str>::into(e).into()))
            }
            fn reset_balance(&mut self, address: H160) {
                let source = T::AddressMapping::get_account_id(&address);
                let balance = T::Currency::free_balance(&source);
                if !balance.is_zero() {
                    if let Err(e)
                        = T::Currency::transfer(
                            &source,
                            &T::TreasuryAccount::get(),
                            balance,
                            ExistenceRequirement::AllowDeath,
                        ) {
                        if true {
                            if !false {
                                ::core::panicking::panic_fmt(
                                    format_args!(
                                        "Failed to transfer remaining balance to treasury with error: {0:?}",
                                        e
                                    ),
                                )
                            }
                        }
                    }
                }
            }
            fn touch(&mut self, _address: H160) {}
            fn is_cold(&self, address: H160) -> bool {
                self.substate
                    .recursive_is_cold(&(|a| a.accessed_addresses.contains(&address)))
            }
            fn is_storage_cold(&self, address: H160, key: H256) -> bool {
                self.substate
                    .recursive_is_cold(
                        &(|a: &Accessed| a.accessed_storage.contains(&(address, key))),
                    )
            }
        }
        impl<'vicinity, 'config, T: Config> CustomStackState
        for SubstrateStackState<'vicinity, 'config, T> {
            fn code_hash_at_address(&self, address: H160) -> H256 {
                Pallet::<T>::code_hash_at_address(&address)
            }
            fn code_size_at_address(&self, address: H160) -> U256 {
                Pallet::<T>::code_size_at_address(&address)
            }
        }
    }
    pub mod state {
        use crate::{encode_revert_message, StorageMeter};
        use core::{cmp::min, convert::Infallible};
        use frame_support::log;
        use module_evm_utility::{
            ethereum::Log,
            evm::{
                backend::Backend, Capture, Config, Context, CreateScheme, ExitError,
                ExitFatal, ExitReason, ExitRevert, ExitSucceed, Opcode, Runtime, Stack,
                Transfer,
            },
            evm_gasometer::{self as gasometer, Gasometer, StorageTarget},
            evm_runtime::Handler,
        };
        pub use primitives::{
            currency::CurrencyIdType,
            evm::{
                EvmAddress, Vicinity, H160_POSITION_CURRENCY_ID_TYPE,
                H160_POSITION_TOKEN_NFT, MIRRORED_NFT_ADDRESS_START,
                SYSTEM_CONTRACT_ADDRESS_PREFIX,
            },
            ReserveIdentifier,
        };
        use sha3::{Digest, Keccak256};
        use sp_core::{H160, H256, U256};
        use sp_runtime::traits::Zero;
        use sp_std::{
            collections::{btree_map::BTreeMap, btree_set::BTreeSet},
            rc::Rc, vec::Vec,
        };
        pub enum StackExitKind {
            Succeeded,
            Reverted,
            Failed,
        }
        pub struct Accessed {
            pub accessed_addresses: BTreeSet<H160>,
            pub accessed_storage: BTreeSet<(H160, H256)>,
        }
        #[automatically_derived]
        impl ::core::default::Default for Accessed {
            #[inline]
            fn default() -> Accessed {
                Accessed {
                    accessed_addresses: ::core::default::Default::default(),
                    accessed_storage: ::core::default::Default::default(),
                }
            }
        }
        #[automatically_derived]
        impl ::core::clone::Clone for Accessed {
            #[inline]
            fn clone(&self) -> Accessed {
                Accessed {
                    accessed_addresses: ::core::clone::Clone::clone(
                        &self.accessed_addresses,
                    ),
                    accessed_storage: ::core::clone::Clone::clone(&self.accessed_storage),
                }
            }
        }
        #[automatically_derived]
        impl ::core::fmt::Debug for Accessed {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::fmt::Formatter::debug_struct_field2_finish(
                    f,
                    "Accessed",
                    "accessed_addresses",
                    &self.accessed_addresses,
                    "accessed_storage",
                    &&self.accessed_storage,
                )
            }
        }
        impl Accessed {
            pub fn access_address(&mut self, address: H160) {
                self.accessed_addresses.insert(address);
            }
            pub fn access_addresses<I>(&mut self, addresses: I)
            where
                I: Iterator<Item = H160>,
            {
                for address in addresses {
                    self.accessed_addresses.insert(address);
                }
            }
            pub fn access_storages<I>(&mut self, storages: I)
            where
                I: Iterator<Item = (H160, H256)>,
            {
                for storage in storages {
                    self.accessed_storage.insert((storage.0, storage.1));
                }
            }
        }
        pub struct StackSubstateMetadata<'config> {
            gasometer: Gasometer<'config>,
            storage_meter: StorageMeter,
            is_static: bool,
            depth: Option<usize>,
            accessed: Option<Accessed>,
            caller: Option<H160>,
            target: Option<H160>,
            origin_code_address: Option<H160>,
        }
        #[automatically_derived]
        impl<'config> ::core::clone::Clone for StackSubstateMetadata<'config> {
            #[inline]
            fn clone(&self) -> StackSubstateMetadata<'config> {
                StackSubstateMetadata {
                    gasometer: ::core::clone::Clone::clone(&self.gasometer),
                    storage_meter: ::core::clone::Clone::clone(&self.storage_meter),
                    is_static: ::core::clone::Clone::clone(&self.is_static),
                    depth: ::core::clone::Clone::clone(&self.depth),
                    accessed: ::core::clone::Clone::clone(&self.accessed),
                    caller: ::core::clone::Clone::clone(&self.caller),
                    target: ::core::clone::Clone::clone(&self.target),
                    origin_code_address: ::core::clone::Clone::clone(
                        &self.origin_code_address,
                    ),
                }
            }
        }
        #[automatically_derived]
        impl<'config> ::core::fmt::Debug for StackSubstateMetadata<'config> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                let names: &'static _ = &[
                    "gasometer",
                    "storage_meter",
                    "is_static",
                    "depth",
                    "accessed",
                    "caller",
                    "target",
                    "origin_code_address",
                ];
                let values: &[&dyn ::core::fmt::Debug] = &[
                    &self.gasometer,
                    &self.storage_meter,
                    &self.is_static,
                    &self.depth,
                    &self.accessed,
                    &self.caller,
                    &self.target,
                    &&self.origin_code_address,
                ];
                ::core::fmt::Formatter::debug_struct_fields_finish(
                    f,
                    "StackSubstateMetadata",
                    names,
                    values,
                )
            }
        }
        impl<'config> StackSubstateMetadata<'config> {
            pub fn new(
                gas_limit: u64,
                storage_limit: u32,
                config: &'config Config,
            ) -> Self {
                let accessed = if config.increase_state_access_gas {
                    Some(Accessed::default())
                } else {
                    None
                };
                Self {
                    gasometer: Gasometer::new(gas_limit, config),
                    storage_meter: StorageMeter::new(storage_limit),
                    is_static: false,
                    depth: None,
                    accessed,
                    caller: None,
                    target: None,
                    origin_code_address: None,
                }
            }
            pub fn swallow_commit(&mut self, other: Self) -> Result<(), ExitError> {
                self.gasometer.record_stipend(other.gasometer.gas())?;
                self.gasometer.record_refund(other.gasometer.refunded_gas())?;
                if let (Some(mut other_accessed), Some(self_accessed))
                    = (other.accessed, self.accessed.as_mut()) {
                    self_accessed
                        .accessed_addresses
                        .append(&mut other_accessed.accessed_addresses);
                    self_accessed
                        .accessed_storage
                        .append(&mut other_accessed.accessed_storage);
                }
                self.storage_meter.merge(&other.storage_meter);
                Ok(())
            }
            pub fn swallow_revert(&mut self, other: Self) -> Result<(), ExitError> {
                self.gasometer.record_stipend(other.gasometer.gas())?;
                Ok(())
            }
            pub fn swallow_discard(&mut self, _other: Self) -> Result<(), ExitError> {
                Ok(())
            }
            pub fn spit_child(&self, gas_limit: u64, is_static: bool) -> Self {
                Self {
                    gasometer: Gasometer::new(gas_limit, self.gasometer.config()),
                    storage_meter: StorageMeter::new(
                        self.storage_meter.available_storage(),
                    ),
                    is_static: is_static || self.is_static,
                    depth: match self.depth {
                        None => Some(0),
                        Some(n) => Some(n + 1),
                    },
                    accessed: self.accessed.as_ref().map(|_| Accessed::default()),
                    caller: None,
                    target: None,
                    origin_code_address: self.origin_code_address,
                }
            }
            pub fn gasometer(&self) -> &Gasometer<'config> {
                &self.gasometer
            }
            pub fn gasometer_mut(&mut self) -> &mut Gasometer<'config> {
                &mut self.gasometer
            }
            pub fn storage_meter(&self) -> &StorageMeter {
                &self.storage_meter
            }
            pub fn storage_meter_mut(&mut self) -> &mut StorageMeter {
                &mut self.storage_meter
            }
            pub fn is_static(&self) -> bool {
                self.is_static
            }
            pub fn depth(&self) -> Option<usize> {
                self.depth
            }
            pub fn access_address(&mut self, address: H160) {
                if let Some(accessed) = &mut self.accessed {
                    accessed.access_address(address)
                }
            }
            pub fn access_addresses<I>(&mut self, addresses: I)
            where
                I: Iterator<Item = H160>,
            {
                if let Some(accessed) = &mut self.accessed {
                    accessed.access_addresses(addresses);
                }
            }
            pub fn access_storage(&mut self, address: H160, key: H256) {
                if let Some(accessed) = &mut self.accessed {
                    accessed.accessed_storage.insert((address, key));
                }
            }
            pub fn access_storages<I>(&mut self, storages: I)
            where
                I: Iterator<Item = (H160, H256)>,
            {
                if let Some(accessed) = &mut self.accessed {
                    accessed.access_storages(storages);
                }
            }
            pub fn accessed(&self) -> &Option<Accessed> {
                &self.accessed
            }
            pub fn caller(&self) -> &Option<H160> {
                &self.caller
            }
            pub fn caller_mut(&mut self) -> &mut Option<H160> {
                &mut self.caller
            }
            pub fn target(&self) -> &Option<H160> {
                &self.target
            }
            pub fn target_mut(&mut self) -> &mut Option<H160> {
                &mut self.target
            }
            pub fn origin_code_address(&mut self) -> &Option<H160> {
                &self.origin_code_address
            }
            pub fn origin_code_address_mut(&mut self) -> &mut Option<H160> {
                &mut self.origin_code_address
            }
        }
        pub trait CustomStackState {
            fn code_hash_at_address(&self, address: H160) -> H256;
            fn code_size_at_address(&self, address: H160) -> U256;
        }
        pub trait StackState<'config>: Backend + CustomStackState {
            fn metadata(&self) -> &StackSubstateMetadata<'config>;
            fn metadata_mut(&mut self) -> &mut StackSubstateMetadata<'config>;
            fn enter(&mut self, gas_limit: u64, is_static: bool);
            fn exit_commit(&mut self) -> Result<(), ExitError>;
            fn exit_revert(&mut self) -> Result<(), ExitError>;
            fn exit_discard(&mut self) -> Result<(), ExitError>;
            fn is_empty(&self, address: H160) -> bool;
            fn deleted(&self, address: H160) -> bool;
            fn is_cold(&self, address: H160) -> bool;
            fn is_storage_cold(&self, address: H160, key: H256) -> bool;
            fn inc_nonce(&mut self, address: H160);
            fn set_storage(&mut self, address: H160, key: H256, value: H256);
            fn reset_storage(&mut self, address: H160);
            fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>);
            fn set_deleted(&mut self, address: H160);
            fn set_code(&mut self, address: H160, code: Vec<u8>);
            fn transfer(&mut self, transfer: Transfer) -> Result<(), ExitError>;
            fn reset_balance(&mut self, address: H160);
            fn touch(&mut self, address: H160);
        }
        /// Data returned by a precompile on success.
        pub struct PrecompileOutput {
            pub exit_status: ExitSucceed,
            pub cost: u64,
            pub output: Vec<u8>,
            pub logs: Vec<Log>,
        }
        #[automatically_derived]
        impl ::core::fmt::Debug for PrecompileOutput {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::fmt::Formatter::debug_struct_field4_finish(
                    f,
                    "PrecompileOutput",
                    "exit_status",
                    &self.exit_status,
                    "cost",
                    &self.cost,
                    "output",
                    &self.output,
                    "logs",
                    &&self.logs,
                )
            }
        }
        #[automatically_derived]
        impl ::core::marker::StructuralEq for PrecompileOutput {}
        #[automatically_derived]
        impl ::core::cmp::Eq for PrecompileOutput {
            #[inline]
            #[doc(hidden)]
            #[no_coverage]
            fn assert_receiver_is_total_eq(&self) -> () {
                let _: ::core::cmp::AssertParamIsEq<ExitSucceed>;
                let _: ::core::cmp::AssertParamIsEq<u64>;
                let _: ::core::cmp::AssertParamIsEq<Vec<u8>>;
                let _: ::core::cmp::AssertParamIsEq<Vec<Log>>;
            }
        }
        #[automatically_derived]
        impl ::core::marker::StructuralPartialEq for PrecompileOutput {}
        #[automatically_derived]
        impl ::core::cmp::PartialEq for PrecompileOutput {
            #[inline]
            fn eq(&self, other: &PrecompileOutput) -> bool {
                self.exit_status == other.exit_status && self.cost == other.cost
                    && self.output == other.output && self.logs == other.logs
            }
        }
        #[automatically_derived]
        impl ::core::clone::Clone for PrecompileOutput {
            #[inline]
            fn clone(&self) -> PrecompileOutput {
                PrecompileOutput {
                    exit_status: ::core::clone::Clone::clone(&self.exit_status),
                    cost: ::core::clone::Clone::clone(&self.cost),
                    output: ::core::clone::Clone::clone(&self.output),
                    logs: ::core::clone::Clone::clone(&self.logs),
                }
            }
        }
        /// Data returned by a precompile in case of failure.
        pub enum PrecompileFailure {
            /// Reverts the state changes and consume all the gas.
            Error { exit_status: ExitError },
            /// Reverts the state changes and consume the provided `cost`.
            /// Returns the provided error message.
            Revert { exit_status: ExitRevert, output: Vec<u8>, cost: u64 },
            /// Mark this failure as fatal, and all EVM execution stacks must be exited.
            Fatal { exit_status: ExitFatal },
        }
        #[automatically_derived]
        impl ::core::fmt::Debug for PrecompileFailure {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                match self {
                    PrecompileFailure::Error { exit_status: __self_0 } => {
                        ::core::fmt::Formatter::debug_struct_field1_finish(
                            f,
                            "Error",
                            "exit_status",
                            &__self_0,
                        )
                    }
                    PrecompileFailure::Revert {
                        exit_status: __self_0,
                        output: __self_1,
                        cost: __self_2,
                    } => {
                        ::core::fmt::Formatter::debug_struct_field3_finish(
                            f,
                            "Revert",
                            "exit_status",
                            __self_0,
                            "output",
                            __self_1,
                            "cost",
                            &__self_2,
                        )
                    }
                    PrecompileFailure::Fatal { exit_status: __self_0 } => {
                        ::core::fmt::Formatter::debug_struct_field1_finish(
                            f,
                            "Fatal",
                            "exit_status",
                            &__self_0,
                        )
                    }
                }
            }
        }
        #[automatically_derived]
        impl ::core::marker::StructuralEq for PrecompileFailure {}
        #[automatically_derived]
        impl ::core::cmp::Eq for PrecompileFailure {
            #[inline]
            #[doc(hidden)]
            #[no_coverage]
            fn assert_receiver_is_total_eq(&self) -> () {
                let _: ::core::cmp::AssertParamIsEq<ExitError>;
                let _: ::core::cmp::AssertParamIsEq<ExitRevert>;
                let _: ::core::cmp::AssertParamIsEq<Vec<u8>>;
                let _: ::core::cmp::AssertParamIsEq<u64>;
                let _: ::core::cmp::AssertParamIsEq<ExitFatal>;
            }
        }
        #[automatically_derived]
        impl ::core::marker::StructuralPartialEq for PrecompileFailure {}
        #[automatically_derived]
        impl ::core::cmp::PartialEq for PrecompileFailure {
            #[inline]
            fn eq(&self, other: &PrecompileFailure) -> bool {
                let __self_tag = ::core::intrinsics::discriminant_value(self);
                let __arg1_tag = ::core::intrinsics::discriminant_value(other);
                __self_tag == __arg1_tag
                    && match (self, other) {
                        (
                            PrecompileFailure::Error { exit_status: __self_0 },
                            PrecompileFailure::Error { exit_status: __arg1_0 },
                        ) => *__self_0 == *__arg1_0,
                        (
                            PrecompileFailure::Revert {
                                exit_status: __self_0,
                                output: __self_1,
                                cost: __self_2,
                            },
                            PrecompileFailure::Revert {
                                exit_status: __arg1_0,
                                output: __arg1_1,
                                cost: __arg1_2,
                            },
                        ) => {
                            *__self_0 == *__arg1_0 && *__self_1 == *__arg1_1
                                && *__self_2 == *__arg1_2
                        }
                        (
                            PrecompileFailure::Fatal { exit_status: __self_0 },
                            PrecompileFailure::Fatal { exit_status: __arg1_0 },
                        ) => *__self_0 == *__arg1_0,
                        _ => unsafe { ::core::intrinsics::unreachable() }
                    }
            }
        }
        #[automatically_derived]
        impl ::core::clone::Clone for PrecompileFailure {
            #[inline]
            fn clone(&self) -> PrecompileFailure {
                match self {
                    PrecompileFailure::Error { exit_status: __self_0 } => {
                        PrecompileFailure::Error {
                            exit_status: ::core::clone::Clone::clone(__self_0),
                        }
                    }
                    PrecompileFailure::Revert {
                        exit_status: __self_0,
                        output: __self_1,
                        cost: __self_2,
                    } => {
                        PrecompileFailure::Revert {
                            exit_status: ::core::clone::Clone::clone(__self_0),
                            output: ::core::clone::Clone::clone(__self_1),
                            cost: ::core::clone::Clone::clone(__self_2),
                        }
                    }
                    PrecompileFailure::Fatal { exit_status: __self_0 } => {
                        PrecompileFailure::Fatal {
                            exit_status: ::core::clone::Clone::clone(__self_0),
                        }
                    }
                }
            }
        }
        /// A precompile result.
        pub type PrecompileResult = Result<PrecompileOutput, PrecompileFailure>;
        /// A set of precompiles.
        /// Checks of the provided address being in the precompile set should be
        /// as cheap as possible since it may be called often.
        pub trait PrecompileSet {
            /// Tries to execute a precompile in the precompile set.
            /// If the provided address is not a precompile, returns None.
            fn execute(
                &self,
                address: H160,
                input: &[u8],
                gas_limit: Option<u64>,
                context: &Context,
                is_static: bool,
            ) -> Option<PrecompileResult>;
            /// Check if the given address is a precompile. Should only be called to
            /// perform the check while not executing the precompile afterward, since
            /// `execute` already performs a check internally.
            fn is_precompile(&self, address: H160) -> bool;
        }
        impl PrecompileSet for () {
            fn execute(
                &self,
                _: H160,
                _: &[u8],
                _: Option<u64>,
                _: &Context,
                _: bool,
            ) -> Option<PrecompileResult> {
                None
            }
            fn is_precompile(&self, _: H160) -> bool {
                false
            }
        }
        /// Precompiles function signature. Expected input arguments are:
        ///  * Input
        ///  * Gas limit
        ///  * Context
        ///  * Is static
        pub type PrecompileFn = fn(
            &[u8],
            Option<u64>,
            &Context,
            bool,
        ) -> PrecompileResult;
        impl PrecompileSet for BTreeMap<H160, PrecompileFn> {
            fn execute(
                &self,
                address: H160,
                input: &[u8],
                gas_limit: Option<u64>,
                context: &Context,
                is_static: bool,
            ) -> Option<PrecompileResult> {
                self.get(&address)
                    .map(|precompile| (*precompile)(
                        input,
                        gas_limit,
                        context,
                        is_static,
                    ))
            }
            /// Check if the given address is a precompile. Should only be called to
            /// perform the check while not executing the precompile afterward, since
            /// `execute` already performs a check internally.
            fn is_precompile(&self, address: H160) -> bool {
                self.contains_key(&address)
            }
        }
        /// Stack-based executor.
        pub struct StackExecutor<'config, 'precompiles, S, P> {
            config: &'config Config,
            state: S,
            precompile_set: &'precompiles P,
        }
        impl<
            'config,
            'precompiles,
            S: StackState<'config>,
            P: PrecompileSet,
        > StackExecutor<'config, 'precompiles, S, P> {
            /// Return a reference of the Config.
            pub fn config(&self) -> &'config Config {
                self.config
            }
            /// Return a reference to the precompile set.
            pub fn precompiles(&self) -> &'precompiles P {
                self.precompile_set
            }
            /// Create a new stack-based executor with given precompiles.
            pub fn new_with_precompiles(
                state: S,
                config: &'config Config,
                precompile_set: &'precompiles P,
            ) -> Self {
                Self {
                    config,
                    state,
                    precompile_set,
                }
            }
            pub fn state(&self) -> &S {
                &self.state
            }
            pub fn state_mut(&mut self) -> &mut S {
                &mut self.state
            }
            pub fn into_state(self) -> S {
                self.state
            }
            /// Create a substate executor from the current executor.
            pub fn enter_substate(&mut self, gas_limit: u64, is_static: bool) {
                self.state.enter(gas_limit, is_static);
            }
            /// Exit a substate. Panic if it results an empty substate stack.
            pub fn exit_substate(
                &mut self,
                kind: StackExitKind,
            ) -> Result<(), ExitError> {
                match kind {
                    StackExitKind::Succeeded => self.state.exit_commit(),
                    StackExitKind::Reverted => self.state.exit_revert(),
                    StackExitKind::Failed => self.state.exit_discard(),
                }
            }
            /// Execute the runtime until it returns.
            pub fn execute(&mut self, runtime: &mut Runtime) -> ExitReason {
                match runtime.run(self) {
                    Capture::Exit(s) => s,
                    Capture::Trap(_) => {
                        ::core::panicking::panic_fmt(
                            format_args!(
                                "internal error: entered unreachable code: {0}",
                                format_args!("Trap is Infallible")
                            ),
                        )
                    }
                }
            }
            /// Get remaining gas.
            pub fn gas(&self) -> u64 {
                self.state.metadata().gasometer.gas()
            }
            fn record_create_transaction_cost(
                &mut self,
                init_code: &[u8],
                access_list: &[(H160, Vec<H256>)],
            ) -> Result<(), ExitError> {
                let transaction_cost = gasometer::create_transaction_cost(
                    init_code,
                    access_list,
                );
                let gasometer = &mut self.state.metadata_mut().gasometer;
                gasometer.record_transaction(transaction_cost)
            }
            /// Execute a `CREATE` transaction.
            pub fn transact_create(
                &mut self,
                caller: H160,
                value: U256,
                init_code: Vec<u8>,
                gas_limit: u64,
                access_list: Vec<(H160, Vec<H256>)>,
            ) -> (ExitReason, Vec<u8>) {
                if let Err(e)
                    = self.record_create_transaction_cost(&init_code, &access_list)
                {
                    return {
                        let reason = e.into();
                        let return_value = Vec::new();
                        (reason, return_value)
                    };
                }
                self.initialize_with_access_list(access_list);
                match self
                    .create_inner(
                        caller,
                        CreateScheme::Legacy { caller },
                        value,
                        init_code,
                        Some(gas_limit),
                        false,
                    )
                {
                    Capture::Exit((s, _, v)) => {
                        let reason = s;
                        let return_value = v;
                        (reason, return_value)
                    }
                    Capture::Trap(_) => {
                        ::core::panicking::panic(
                            "internal error: entered unreachable code",
                        )
                    }
                }
            }
            /// Execute a `CREATE2` transaction.
            pub fn transact_create2(
                &mut self,
                caller: H160,
                value: U256,
                init_code: Vec<u8>,
                salt: H256,
                gas_limit: u64,
                access_list: Vec<(H160, Vec<H256>)>,
            ) -> (ExitReason, Vec<u8>) {
                let code_hash = H256::from_slice(
                    Keccak256::digest(&init_code).as_slice(),
                );
                if let Err(e)
                    = self.record_create_transaction_cost(&init_code, &access_list)
                {
                    return {
                        let reason = e.into();
                        let return_value = Vec::new();
                        (reason, return_value)
                    };
                }
                self.initialize_with_access_list(access_list);
                match self
                    .create_inner(
                        caller,
                        CreateScheme::Create2 {
                            caller,
                            code_hash,
                            salt,
                        },
                        value,
                        init_code,
                        Some(gas_limit),
                        false,
                    )
                {
                    Capture::Exit((s, _, v)) => {
                        let reason = s;
                        let return_value = v;
                        (reason, return_value)
                    }
                    Capture::Trap(_) => {
                        ::core::panicking::panic(
                            "internal error: entered unreachable code",
                        )
                    }
                }
            }
            /// Execute a `CREATE` transaction with specific address.
            pub fn transact_create_at_address(
                &mut self,
                caller: H160,
                address: H160,
                value: U256,
                init_code: Vec<u8>,
                gas_limit: u64,
                access_list: Vec<(H160, Vec<H256>)>,
            ) -> (ExitReason, Vec<u8>) {
                if let Err(e)
                    = self.record_create_transaction_cost(&init_code, &access_list)
                {
                    return {
                        let reason = e.into();
                        let return_value = Vec::new();
                        (reason, return_value)
                    };
                }
                self.initialize_with_access_list(access_list);
                match self
                    .create_inner(
                        caller,
                        CreateScheme::Fixed(address),
                        value,
                        init_code,
                        Some(gas_limit),
                        false,
                    )
                {
                    Capture::Exit((s, _, v)) => {
                        let reason = s;
                        let return_value = v;
                        (reason, return_value)
                    }
                    Capture::Trap(_) => {
                        ::core::panicking::panic(
                            "internal error: entered unreachable code",
                        )
                    }
                }
            }
            /// Execute a `CALL` transaction with a given caller, address, value and
            /// gas limit and data.
            ///
            /// Takes in an additional `access_list` parameter for EIP-2930 which was
            /// introduced in the Ethereum Berlin hard fork. If you do not wish to use
            /// this functionality, just pass in an empty vector.
            pub fn transact_call(
                &mut self,
                caller: H160,
                address: H160,
                value: U256,
                data: Vec<u8>,
                gas_limit: u64,
                access_list: Vec<(H160, Vec<H256>)>,
            ) -> (ExitReason, Vec<u8>) {
                let transaction_cost = gasometer::call_transaction_cost(
                    &data,
                    &access_list,
                );
                let gasometer = &mut self.state.metadata_mut().gasometer;
                match gasometer.record_transaction(transaction_cost) {
                    Ok(()) => {}
                    Err(e) => {
                        return {
                            let reason = e.into();
                            let return_value = Vec::new();
                            (reason, return_value)
                        };
                    }
                }
                *self.state.metadata_mut().origin_code_address_mut() = Some(address);
                if self.config.increase_state_access_gas {
                    let addresses = core::iter::once(caller)
                        .chain(core::iter::once(address));
                    self.state.metadata_mut().access_addresses(addresses);
                    self.initialize_with_access_list(access_list);
                }
                self.state.inc_nonce(caller);
                let context = Context {
                    caller,
                    address,
                    apparent_value: value,
                };
                match self
                    .call_inner(
                        address,
                        Some(Transfer {
                            source: caller,
                            target: address,
                            value,
                        }),
                        data,
                        Some(gas_limit),
                        false,
                        false,
                        false,
                        context,
                    )
                {
                    Capture::Exit((s, v)) => {
                        let reason = s;
                        let return_value = v;
                        (reason, return_value)
                    }
                    Capture::Trap(_) => {
                        ::core::panicking::panic(
                            "internal error: entered unreachable code",
                        )
                    }
                }
            }
            /// Get used gas for the current executor, given the price.
            pub fn used_gas(&self) -> u64 {
                self.state.metadata().gasometer.total_used_gas()
                    - min(
                        self.state.metadata().gasometer.total_used_gas()
                            / self.config.max_refund_quotient,
                        self.state.metadata().gasometer.refunded_gas() as u64,
                    )
            }
            /// Get fee needed for the current executor, given the price.
            pub fn fee(&self, price: U256) -> U256 {
                let used_gas = self.used_gas();
                U256::from(used_gas) * price
            }
            /// Get account nonce.
            pub fn nonce(&self, address: H160) -> U256 {
                self.state.basic(address).nonce
            }
            /// Get the create address from given scheme.
            pub fn create_address(
                &self,
                scheme: CreateScheme,
            ) -> Result<H160, ExitError> {
                let address = match scheme {
                    CreateScheme::Create2 { caller, code_hash, salt } => {
                        let mut hasher = Keccak256::new();
                        hasher.update([0xff]);
                        hasher.update(&caller[..]);
                        hasher.update(&salt[..]);
                        hasher.update(&code_hash[..]);
                        H256::from_slice(hasher.finalize().as_slice()).into()
                    }
                    CreateScheme::Legacy { caller } => {
                        let nonce = self.nonce(caller);
                        let mut stream = rlp::RlpStream::new_list(2);
                        stream.append(&caller);
                        stream.append(&nonce);
                        H256::from_slice(Keccak256::digest(&stream.out()).as_slice())
                            .into()
                    }
                    CreateScheme::Fixed(naddress) => naddress,
                };
                match scheme {
                    CreateScheme::Create2 { .. } | CreateScheme::Legacy { .. } => {
                        if address
                            .as_bytes()
                            .starts_with(&SYSTEM_CONTRACT_ADDRESS_PREFIX)
                        {
                            Err(ExitError::CreateCollision)
                        } else {
                            Ok(address)
                        }
                    }
                    _ => Ok(address),
                }
            }
            pub fn initialize_with_access_list(
                &mut self,
                access_list: Vec<(H160, Vec<H256>)>,
            ) {
                let addresses = access_list.iter().map(|a| a.0);
                self.state.metadata_mut().access_addresses(addresses);
                let storage_keys = access_list
                    .into_iter()
                    .flat_map(|(address, keys)| {
                        keys.into_iter().map(move |key| (address, key))
                    });
                self.state.metadata_mut().access_storages(storage_keys);
            }
            fn create_inner(
                &mut self,
                caller: H160,
                scheme: CreateScheme,
                value: U256,
                init_code: Vec<u8>,
                target_gas: Option<u64>,
                take_l64: bool,
            ) -> Capture<(ExitReason, Option<H160>, Vec<u8>), Infallible> {
                fn check_first_byte(
                    config: &Config,
                    code: &[u8],
                ) -> Result<(), ExitError> {
                    if config.disallow_executable_format
                        && Some(&Opcode::EOFMAGIC.as_u8()) == code.first()
                    {
                        return Err(ExitError::InvalidCode(Opcode::EOFMAGIC));
                    }
                    Ok(())
                }
                fn l64(gas: u64) -> u64 {
                    gas - gas / 64
                }
                let address = match self.create_address(scheme) {
                    Err(e) => {
                        return Capture::Exit((ExitReason::Error(e), None, Vec::new()));
                    }
                    Ok(address) => address,
                };
                *self.state.metadata_mut().caller_mut() = Some(caller);
                *self.state.metadata_mut().target_mut() = Some(address);
                self.state.metadata_mut().access_address(caller);
                self.state.metadata_mut().access_address(address);
                if let Some(depth) = self.state.metadata().depth {
                    if depth >= self.config.call_stack_limit {
                        return Capture::Exit((
                            ExitError::CallTooDeep.into(),
                            None,
                            Vec::new(),
                        ));
                    }
                }
                if self.balance(caller) < value {
                    return Capture::Exit((
                        ExitError::OutOfFund.into(),
                        None,
                        Vec::new(),
                    ));
                }
                let after_gas = if take_l64 && self.config.call_l64_after_gas {
                    if self.config.estimate {
                        let initial_after_gas = self.state.metadata().gasometer.gas();
                        let diff = initial_after_gas - l64(initial_after_gas);
                        match self.state.metadata_mut().gasometer.record_cost(diff) {
                            Ok(v) => v,
                            Err(e) => return Capture::Exit((e.into(), None, Vec::new())),
                        };
                        self.state.metadata().gasometer.gas()
                    } else {
                        l64(self.state.metadata().gasometer.gas())
                    }
                } else {
                    self.state.metadata().gasometer.gas()
                };
                let target_gas = target_gas.unwrap_or(after_gas);
                let gas_limit = min(after_gas, target_gas);
                match self.state.metadata_mut().gasometer.record_cost(gas_limit) {
                    Ok(v) => v,
                    Err(e) => return Capture::Exit((e.into(), None, Vec::new())),
                };
                self.state.inc_nonce(caller);
                self.enter_substate(gas_limit, false);
                {
                    if self.code_size(address) != U256::zero() {
                        let _ = self.exit_substate(StackExitKind::Failed);
                        return Capture::Exit((
                            ExitError::CreateCollision.into(),
                            None,
                            Vec::new(),
                        ));
                    }
                    if self.nonce(address) > U256::zero() {
                        let _ = self.exit_substate(StackExitKind::Failed);
                        return Capture::Exit((
                            ExitError::CreateCollision.into(),
                            None,
                            Vec::new(),
                        ));
                    }
                    self.state.reset_storage(address);
                }
                let context = Context {
                    address,
                    caller,
                    apparent_value: value,
                };
                let transfer = Transfer {
                    source: caller,
                    target: address,
                    value,
                };
                match self.state.transfer(transfer) {
                    Ok(()) => {}
                    Err(e) => {
                        let _ = self.exit_substate(StackExitKind::Reverted);
                        return Capture::Exit((ExitReason::Error(e), None, Vec::new()));
                    }
                }
                if self.config.create_increase_nonce {
                    self.state.inc_nonce(address);
                }
                let mut runtime = Runtime::new(
                    Rc::new(init_code),
                    Rc::new(Vec::new()),
                    context,
                    self.config,
                );
                let reason = self.execute(&mut runtime);
                {
                    let lvl = ::log::Level::Debug;
                    if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                        ::log::__private_api_log(
                            format_args!(
                                "Create execution using address {0}: {1:?}", address, reason
                            ),
                            lvl,
                            &(
                                "evm",
                                "module_evm::runner::state",
                                "modules/evm/src/runner/state.rs",
                                881u32,
                            ),
                            ::log::__private_api::Option::None,
                        );
                    }
                };
                match reason {
                    ExitReason::Succeed(s) => {
                        let out = runtime.machine().return_value();
                        if let Err(e) = check_first_byte(self.config, &out) {
                            self.state.metadata_mut().gasometer.fail();
                            let _ = self.exit_substate(StackExitKind::Failed);
                            return Capture::Exit((e.into(), None, Vec::new()));
                        }
                        if let Some(limit) = self.config.create_contract_limit {
                            if out.len() > limit {
                                self.state.metadata_mut().gasometer.fail();
                                let _ = self.exit_substate(StackExitKind::Failed);
                                return Capture::Exit((
                                    ExitError::CreateContractLimit.into(),
                                    None,
                                    Vec::new(),
                                ));
                            }
                        }
                        match self
                            .state
                            .metadata_mut()
                            .gasometer
                            .record_deposit(out.len())
                        {
                            Ok(()) => {
                                self.state.set_code(address, out);
                                let e = self.exit_substate(StackExitKind::Succeeded);
                                match e {
                                    Ok(v) => v,
                                    Err(e) => return Capture::Exit((e.into(), None, Vec::new())),
                                };
                                Capture::Exit((
                                    ExitReason::Succeed(s),
                                    Some(address),
                                    Vec::new(),
                                ))
                            }
                            Err(e) => {
                                let _ = self.exit_substate(StackExitKind::Failed);
                                Capture::Exit((ExitReason::Error(e), None, Vec::new()))
                            }
                        }
                    }
                    ExitReason::Error(e) => {
                        self.state.metadata_mut().gasometer.fail();
                        let _ = self.exit_substate(StackExitKind::Failed);
                        Capture::Exit((ExitReason::Error(e), None, Vec::new()))
                    }
                    ExitReason::Revert(e) => {
                        let _ = self.exit_substate(StackExitKind::Reverted);
                        Capture::Exit((
                            ExitReason::Revert(e),
                            None,
                            runtime.machine().return_value(),
                        ))
                    }
                    ExitReason::Fatal(e) => {
                        self.state.metadata_mut().gasometer.fail();
                        let _ = self.exit_substate(StackExitKind::Failed);
                        Capture::Exit((ExitReason::Fatal(e), None, Vec::new()))
                    }
                }
            }
            #[allow(clippy::too_many_arguments)]
            fn call_inner(
                &mut self,
                code_address: H160,
                transfer: Option<Transfer>,
                input: Vec<u8>,
                target_gas: Option<u64>,
                is_static: bool,
                take_l64: bool,
                take_stipend: bool,
                context: Context,
            ) -> Capture<(ExitReason, Vec<u8>), Infallible> {
                fn l64(gas: u64) -> u64 {
                    gas - gas / 64
                }
                *self.state.metadata_mut().target_mut() = Some(code_address);
                let after_gas = if take_l64 && self.config.call_l64_after_gas {
                    if self.config.estimate {
                        let initial_after_gas = self.state.metadata().gasometer.gas();
                        let diff = initial_after_gas - l64(initial_after_gas);
                        match self.state.metadata_mut().gasometer.record_cost(diff) {
                            Ok(v) => v,
                            Err(e) => return Capture::Exit((e.into(), Vec::new())),
                        };
                        self.state.metadata().gasometer.gas()
                    } else {
                        l64(self.state.metadata().gasometer.gas())
                    }
                } else {
                    self.state.metadata().gasometer.gas()
                };
                let target_gas = target_gas.unwrap_or(after_gas);
                let mut gas_limit = min(target_gas, after_gas);
                match self.state.metadata_mut().gasometer.record_cost(gas_limit) {
                    Ok(v) => v,
                    Err(e) => return Capture::Exit((e.into(), Vec::new())),
                };
                if let Some(transfer) = transfer.as_ref() {
                    if take_stipend && transfer.value != U256::zero() {
                        gas_limit = gas_limit.saturating_add(self.config.call_stipend);
                    }
                }
                let code = self.code(code_address);
                self.enter_substate(gas_limit, is_static);
                self.state.touch(context.address);
                if let Some(depth) = self.state.metadata().depth {
                    if depth > self.config.call_stack_limit {
                        let _ = self.exit_substate(StackExitKind::Reverted);
                        return Capture::Exit((
                            ExitError::CallTooDeep.into(),
                            Vec::new(),
                        ));
                    }
                }
                if let Some(transfer) = transfer {
                    match self.state.transfer(transfer) {
                        Ok(()) => {}
                        Err(e) => {
                            let _ = self.exit_substate(StackExitKind::Reverted);
                            return Capture::Exit((ExitReason::Error(e), Vec::new()));
                        }
                    }
                }
                let precompile_is_static = self.state.metadata().is_static();
                if let Some(result)
                    = self
                        .precompile_set
                        .execute(
                            code_address,
                            &input,
                            Some(gas_limit),
                            &context,
                            precompile_is_static,
                        )
                {
                    return match result {
                        Ok(PrecompileOutput { exit_status, output, cost, logs }) => {
                            for Log { address, topics, data } in logs {
                                match self.log(address, topics, data) {
                                    Ok(_) => continue,
                                    Err(error) => {
                                        return Capture::Exit((ExitReason::Error(error), output));
                                    }
                                }
                            }
                            let _ = self
                                .state
                                .metadata_mut()
                                .gasometer
                                .record_cost(cost);
                            let e = self.exit_substate(StackExitKind::Succeeded);
                            match e {
                                Ok(v) => v,
                                Err(e) => return Capture::Exit((e.into(), Vec::new())),
                            };
                            Capture::Exit((ExitReason::Succeed(exit_status), output))
                        }
                        Err(PrecompileFailure::Error { exit_status }) => {
                            let _ = self.exit_substate(StackExitKind::Failed);
                            Capture::Exit((ExitReason::Error(exit_status), Vec::new()))
                        }
                        Err(PrecompileFailure::Revert { exit_status, output, cost }) => {
                            let _ = self
                                .state
                                .metadata_mut()
                                .gasometer
                                .record_cost(cost);
                            let _ = self.exit_substate(StackExitKind::Reverted);
                            Capture::Exit((
                                ExitReason::Revert(exit_status),
                                encode_revert_message(&output),
                            ))
                        }
                        Err(PrecompileFailure::Fatal { exit_status }) => {
                            self.state.metadata_mut().gasometer.fail();
                            let _ = self.exit_substate(StackExitKind::Failed);
                            Capture::Exit((ExitReason::Fatal(exit_status), Vec::new()))
                        }
                    };
                }
                let mut runtime = Runtime::new(
                    Rc::new(code),
                    Rc::new(input),
                    context,
                    self.config,
                );
                #[cfg(not(feature = "tracing"))]
                let reason = self.execute(&mut runtime);
                {
                    let lvl = ::log::Level::Debug;
                    if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                        ::log::__private_api_log(
                            format_args!(
                                "Call execution using address {0}: {1:?}", code_address,
                                reason
                            ),
                            lvl,
                            &(
                                "evm",
                                "module_evm::runner::state",
                                "modules/evm/src/runner/state.rs",
                                1075u32,
                            ),
                            ::log::__private_api::Option::None,
                        );
                    }
                };
                match reason {
                    ExitReason::Succeed(s) => {
                        let e = self.exit_substate(StackExitKind::Succeeded);
                        match e {
                            Ok(v) => v,
                            Err(e) => return Capture::Exit((e.into(), Vec::new())),
                        };
                        Capture::Exit((
                            ExitReason::Succeed(s),
                            runtime.machine().return_value(),
                        ))
                    }
                    ExitReason::Error(e) => {
                        let _ = self.exit_substate(StackExitKind::Failed);
                        Capture::Exit((ExitReason::Error(e), Vec::new()))
                    }
                    ExitReason::Revert(e) => {
                        let _ = self.exit_substate(StackExitKind::Reverted);
                        Capture::Exit((
                            ExitReason::Revert(e),
                            runtime.machine().return_value(),
                        ))
                    }
                    ExitReason::Fatal(e) => {
                        self.state.metadata_mut().gasometer.fail();
                        let _ = self.exit_substate(StackExitKind::Failed);
                        Capture::Exit((ExitReason::Fatal(e), Vec::new()))
                    }
                }
            }
        }
        impl<'config, 'precompiles, S: StackState<'config>, P: PrecompileSet> Handler
        for StackExecutor<'config, 'precompiles, S, P> {
            type CreateInterrupt = Infallible;
            type CreateFeedback = Infallible;
            type CallInterrupt = Infallible;
            type CallFeedback = Infallible;
            fn balance(&self, address: H160) -> U256 {
                self.state.basic(address).balance
            }
            fn code_size(&self, address: H160) -> U256 {
                self.state.code_size_at_address(address)
            }
            fn code_hash(&self, address: H160) -> H256 {
                if !self.exists(address) {
                    return H256::default();
                }
                self.state.code_hash_at_address(address)
            }
            fn code(&self, address: H160) -> Vec<u8> {
                let code = self.state.code(address);
                if code.len().is_zero() && !self.precompile_set.is_precompile(address) {
                    {
                        let lvl = ::log::Level::Debug;
                        if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                            ::log::__private_api_log(
                                format_args!(
                                    "contract does not exist, address: {0:?}", address
                                ),
                                lvl,
                                &(
                                    "evm",
                                    "module_evm::runner::state",
                                    "modules/evm/src/runner/state.rs",
                                    1127u32,
                                ),
                                ::log::__private_api::Option::None,
                            );
                        }
                    };
                }
                code
            }
            fn storage(&self, address: H160, index: H256) -> H256 {
                self.state.storage(address, index)
            }
            fn original_storage(&self, address: H160, index: H256) -> H256 {
                self.state.original_storage(address, index).unwrap_or_default()
            }
            fn exists(&self, address: H160) -> bool {
                if self.config.empty_considered_exists {
                    self.state.exists(address)
                } else {
                    self.state.exists(address) && !self.state.is_empty(address)
                }
            }
            fn is_cold(&self, address: H160, maybe_index: Option<H256>) -> bool {
                match maybe_index {
                    None => {
                        !self.precompile_set.is_precompile(address)
                            && self.state.is_cold(address)
                    }
                    Some(index) => self.state.is_storage_cold(address, index),
                }
            }
            fn gas_left(&self) -> U256 {
                U256::from(self.state.metadata().gasometer.gas())
            }
            fn gas_price(&self) -> U256 {
                self.state.gas_price()
            }
            fn origin(&self) -> H160 {
                self.state.origin()
            }
            fn block_hash(&self, number: U256) -> H256 {
                self.state.block_hash(number)
            }
            fn block_number(&self) -> U256 {
                self.state.block_number()
            }
            fn block_coinbase(&self) -> H160 {
                self.state.block_coinbase()
            }
            fn block_timestamp(&self) -> U256 {
                self.state.block_timestamp()
            }
            fn block_difficulty(&self) -> U256 {
                self.state.block_difficulty()
            }
            fn block_gas_limit(&self) -> U256 {
                self.state.block_gas_limit()
            }
            fn block_base_fee_per_gas(&self) -> U256 {
                self.state.block_base_fee_per_gas()
            }
            fn chain_id(&self) -> U256 {
                self.state.chain_id()
            }
            fn deleted(&self, address: H160) -> bool {
                self.state.deleted(address)
            }
            fn set_storage(
                &mut self,
                address: H160,
                index: H256,
                value: H256,
            ) -> Result<(), ExitError> {
                self.state.set_storage(address, index, value);
                Ok(())
            }
            fn log(
                &mut self,
                address: H160,
                topics: Vec<H256>,
                data: Vec<u8>,
            ) -> Result<(), ExitError> {
                self.state.log(address, topics, data);
                Ok(())
            }
            fn mark_delete(
                &mut self,
                address: H160,
                target: H160,
            ) -> Result<(), ExitError> {
                let balance = self.balance(address);
                self.state
                    .transfer(Transfer {
                        source: address,
                        target,
                        value: balance,
                    })?;
                self.state.reset_balance(address);
                self.state.set_deleted(address);
                Ok(())
            }
            #[cfg(not(feature = "tracing"))]
            fn create(
                &mut self,
                caller: H160,
                scheme: CreateScheme,
                value: U256,
                init_code: Vec<u8>,
                target_gas: Option<u64>,
            ) -> Capture<(ExitReason, Option<H160>, Vec<u8>), Self::CreateInterrupt> {
                self.create_inner(caller, scheme, value, init_code, target_gas, true)
            }
            #[cfg(not(feature = "tracing"))]
            fn call(
                &mut self,
                code_address: H160,
                transfer: Option<Transfer>,
                input: Vec<u8>,
                target_gas: Option<u64>,
                is_static: bool,
                context: Context,
            ) -> Capture<(ExitReason, Vec<u8>), Self::CallInterrupt> {
                self.call_inner(
                    code_address,
                    transfer,
                    input,
                    target_gas,
                    is_static,
                    true,
                    true,
                    context,
                )
            }
            #[inline]
            fn pre_validate(
                &mut self,
                context: &Context,
                opcode: Opcode,
                stack: &Stack,
            ) -> Result<(), ExitError> {
                if let Some(cost) = gasometer::static_opcode_cost(opcode) {
                    self.state.metadata_mut().gasometer.record_cost(cost)?;
                } else {
                    let is_static = self.state.metadata().is_static;
                    let (gas_cost, target, memory_cost) = gasometer::dynamic_opcode_cost(
                        context.address,
                        opcode,
                        stack,
                        is_static,
                        self.config,
                        self,
                    )?;
                    let gasometer = &mut self.state.metadata_mut().gasometer;
                    gasometer.record_dynamic_cost(gas_cost, memory_cost)?;
                    match target {
                        StorageTarget::Address(address) => {
                            self.state.metadata_mut().access_address(address)
                        }
                        StorageTarget::Slot(address, key) => {
                            self.state.metadata_mut().access_storage(address, key)
                        }
                        StorageTarget::None => {}
                    }
                }
                Ok(())
            }
        }
    }
    pub mod storage_meter {
        use frame_support::log;
        pub struct StorageMeter {
            limit: u32,
            used: u32,
            refunded: u32,
            child_used: u32,
            child_refunded: u32,
        }
        #[automatically_derived]
        impl ::core::default::Default for StorageMeter {
            #[inline]
            fn default() -> StorageMeter {
                StorageMeter {
                    limit: ::core::default::Default::default(),
                    used: ::core::default::Default::default(),
                    refunded: ::core::default::Default::default(),
                    child_used: ::core::default::Default::default(),
                    child_refunded: ::core::default::Default::default(),
                }
            }
        }
        #[automatically_derived]
        impl ::core::clone::Clone for StorageMeter {
            #[inline]
            fn clone(&self) -> StorageMeter {
                StorageMeter {
                    limit: ::core::clone::Clone::clone(&self.limit),
                    used: ::core::clone::Clone::clone(&self.used),
                    refunded: ::core::clone::Clone::clone(&self.refunded),
                    child_used: ::core::clone::Clone::clone(&self.child_used),
                    child_refunded: ::core::clone::Clone::clone(&self.child_refunded),
                }
            }
        }
        #[automatically_derived]
        impl ::core::fmt::Debug for StorageMeter {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::fmt::Formatter::debug_struct_field5_finish(
                    f,
                    "StorageMeter",
                    "limit",
                    &self.limit,
                    "used",
                    &self.used,
                    "refunded",
                    &self.refunded,
                    "child_used",
                    &self.child_used,
                    "child_refunded",
                    &&self.child_refunded,
                )
            }
        }
        impl StorageMeter {
            /// Create a new storage_meter with given storage limit.
            pub fn new(limit: u32) -> Self {
                Self {
                    limit,
                    used: 0,
                    refunded: 0,
                    child_used: 0,
                    child_refunded: 0,
                }
            }
            pub fn child_meter(&mut self) -> Self {
                let storage = self.available_storage();
                StorageMeter::new(storage)
            }
            pub fn storage_limit(&self) -> u32 {
                self.limit
            }
            pub fn used(&self) -> u32 {
                self.used
            }
            pub fn refunded(&self) -> u32 {
                self.refunded
            }
            pub fn total_used(&self) -> u32 {
                self.used.saturating_add(self.child_used)
            }
            pub fn total_refunded(&self) -> u32 {
                self.refunded.saturating_add(self.child_refunded)
            }
            pub fn available_storage(&self) -> u32 {
                self.limit
                    .saturating_add(self.refunded)
                    .saturating_add(self.child_refunded)
                    .saturating_sub(self.used)
                    .saturating_sub(self.child_used)
            }
            pub fn used_storage(&self) -> i32 {
                if self.used > self.refunded {
                    (self.used - self.refunded) as i32
                } else {
                    -((self.refunded - self.used) as i32)
                }
            }
            pub fn finish(&self) -> Option<i32> {
                let total_used = self.total_used();
                let total_refunded = self.total_refunded();
                {
                    let lvl = ::log::Level::Trace;
                    if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                        ::log::__private_api_log(
                            format_args!(
                                "StorageMeter: finish: used {0:?} refunded {1:?}",
                                total_used, total_refunded
                            ),
                            lvl,
                            &(
                                "evm",
                                "module_evm::runner::storage_meter",
                                "modules/evm/src/runner/storage_meter.rs",
                                87u32,
                            ),
                            ::log::__private_api::Option::None,
                        );
                    }
                };
                if self.limit < total_used.saturating_sub(total_refunded) {
                    return None;
                }
                if total_used > total_refunded {
                    Some((total_used - total_refunded) as i32)
                } else {
                    Some(-((total_refunded - total_used) as i32))
                }
            }
            pub fn charge(&mut self, storage: u32) {
                {
                    let lvl = ::log::Level::Trace;
                    if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                        ::log::__private_api_log(
                            format_args!("StorageMeter: charge: storage {0:?}", storage),
                            lvl,
                            &(
                                "evm",
                                "module_evm::runner::storage_meter",
                                "modules/evm/src/runner/storage_meter.rs",
                                105u32,
                            ),
                            ::log::__private_api::Option::None,
                        );
                    }
                };
                self.used = self.used.saturating_add(storage);
            }
            pub fn uncharge(&mut self, storage: u32) {
                {
                    let lvl = ::log::Level::Trace;
                    if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                        ::log::__private_api_log(
                            format_args!(
                                "StorageMeter: uncharge: storage {0:?}", storage
                            ),
                            lvl,
                            &(
                                "evm",
                                "module_evm::runner::storage_meter",
                                "modules/evm/src/runner/storage_meter.rs",
                                114u32,
                            ),
                            ::log::__private_api::Option::None,
                        );
                    }
                };
                self.used = self.used.saturating_sub(storage);
            }
            pub fn refund(&mut self, storage: u32) {
                {
                    let lvl = ::log::Level::Trace;
                    if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                        ::log::__private_api_log(
                            format_args!("StorageMeter: refund: storage {0:?}", storage),
                            lvl,
                            &(
                                "evm",
                                "module_evm::runner::storage_meter",
                                "modules/evm/src/runner/storage_meter.rs",
                                123u32,
                            ),
                            ::log::__private_api::Option::None,
                        );
                    }
                };
                self.refunded = self.refunded.saturating_add(storage);
            }
            pub fn merge(&mut self, other: &Self) {
                self.child_used = self.child_used.saturating_add(other.total_used());
                self
                    .child_refunded = self
                    .child_refunded
                    .saturating_add(other.total_refunded());
            }
        }
    }
    use crate::{BalanceOf, CallInfo, Config, CreateInfo};
    use frame_support::dispatch::DispatchError;
    use module_evm_utility::evm;
    pub use primitives::evm::{EvmAddress, Vicinity};
    use sp_core::{H160, H256};
    use sp_std::vec::Vec;
    pub trait Runner<T: Config> {
        fn call(
            source: H160,
            origin: H160,
            target: H160,
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<(H160, Vec<H256>)>,
            config: &evm::Config,
        ) -> Result<CallInfo, DispatchError>;
        fn create(
            source: H160,
            init: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<(H160, Vec<H256>)>,
            config: &evm::Config,
        ) -> Result<CreateInfo, DispatchError>;
        fn create2(
            source: H160,
            init: Vec<u8>,
            salt: H256,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<(H160, Vec<H256>)>,
            config: &evm::Config,
        ) -> Result<CreateInfo, DispatchError>;
        fn create_at_address(
            source: H160,
            address: H160,
            init: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<(H160, Vec<H256>)>,
            config: &evm::Config,
        ) -> Result<CreateInfo, DispatchError>;
    }
    pub trait RunnerExtended<T: Config>: Runner<T> {
        fn rpc_call(
            source: H160,
            origin: H160,
            target: H160,
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<(H160, Vec<H256>)>,
            config: &evm::Config,
        ) -> Result<CallInfo, DispatchError>;
        fn rpc_create(
            source: H160,
            init: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<(H160, Vec<H256>)>,
            config: &evm::Config,
        ) -> Result<CreateInfo, DispatchError>;
    }
}
pub mod weights {
    //! Autogenerated weights for module_evm
    //!
    //! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
    //! DATE: 2023-04-18, STEPS: `50`, REPEAT: 20, LOW RANGE: `[]`, HIGH RANGE: `[]`
    //! HOSTNAME: `ip-172-31-40-247`, CPU: `Intel(R) Xeon(R) Platinum 8375C CPU @ 2.90GHz`
    //! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024
    #![allow(unused_parens)]
    #![allow(unused_imports)]
    use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
    use sp_std::marker::PhantomData;
    /// Weight functions needed for module_evm.
    pub trait WeightInfo {
        fn create() -> Weight;
        fn create2() -> Weight;
        fn create_nft_contract() -> Weight;
        fn create_predeploy_contract() -> Weight;
        fn call() -> Weight;
        fn transfer_maintainer() -> Weight;
        fn publish_contract() -> Weight;
        fn publish_free() -> Weight;
        fn enable_contract_development() -> Weight;
        fn disable_contract_development() -> Weight;
        fn set_code(c: u32) -> Weight;
        fn selfdestruct() -> Weight;
    }
    /// Weights for module_evm using the Acala node and recommended hardware.
    pub struct AcalaWeight<T>(PhantomData<T>);
    impl<T: frame_system::Config> WeightInfo for AcalaWeight<T> {
        fn create() -> Weight {
            Weight::from_parts(204_527_000, 0)
                .saturating_add(T::DbWeight::get().reads(12))
                .saturating_add(T::DbWeight::get().writes(9))
        }
        fn create2() -> Weight {
            Weight::from_parts(199_650_000, 0)
                .saturating_add(T::DbWeight::get().reads(12))
                .saturating_add(T::DbWeight::get().writes(9))
        }
        fn create_nft_contract() -> Weight {
            Weight::from_parts(227_640_000, 0)
                .saturating_add(T::DbWeight::get().reads(12))
                .saturating_add(T::DbWeight::get().writes(10))
        }
        fn create_predeploy_contract() -> Weight {
            Weight::from_parts(233_183_000, 0)
                .saturating_add(T::DbWeight::get().reads(11))
                .saturating_add(T::DbWeight::get().writes(9))
        }
        fn call() -> Weight {
            Weight::from_parts(189_885_000, 0)
                .saturating_add(T::DbWeight::get().reads(11))
                .saturating_add(T::DbWeight::get().writes(6))
        }
        fn transfer_maintainer() -> Weight {
            Weight::from_parts(122_117_000, 0)
                .saturating_add(T::DbWeight::get().reads(2))
                .saturating_add(T::DbWeight::get().writes(1))
        }
        fn publish_contract() -> Weight {
            Weight::from_parts(150_918_000, 0)
                .saturating_add(T::DbWeight::get().reads(2))
                .saturating_add(T::DbWeight::get().writes(1))
        }
        fn publish_free() -> Weight {
            Weight::from_parts(40_271_000, 0)
                .saturating_add(T::DbWeight::get().reads(1))
                .saturating_add(T::DbWeight::get().writes(1))
        }
        fn enable_contract_development() -> Weight {
            Weight::from_parts(127_492_000, 0)
                .saturating_add(T::DbWeight::get().reads(1))
                .saturating_add(T::DbWeight::get().writes(1))
        }
        fn disable_contract_development() -> Weight {
            Weight::from_parts(129_795_000, 0)
                .saturating_add(T::DbWeight::get().reads(1))
                .saturating_add(T::DbWeight::get().writes(1))
        }
        /// The range of component `c` is `[0, 61440]`.
        fn set_code(c: u32) -> Weight {
            Weight::from_parts(218_913_195, 0)
                .saturating_add(Weight::from_parts(5_766, 0).saturating_mul(c.into()))
                .saturating_add(T::DbWeight::get().reads(10))
                .saturating_add(T::DbWeight::get().writes(9))
        }
        fn selfdestruct() -> Weight {
            Weight::from_parts(246_450_000, 0)
                .saturating_add(T::DbWeight::get().reads(11))
                .saturating_add(T::DbWeight::get().writes(8))
        }
    }
    impl WeightInfo for () {
        fn create() -> Weight {
            Weight::from_parts(204_527_000, 0)
                .saturating_add(RocksDbWeight::get().reads(12))
                .saturating_add(RocksDbWeight::get().writes(9))
        }
        fn create2() -> Weight {
            Weight::from_parts(199_650_000, 0)
                .saturating_add(RocksDbWeight::get().reads(12))
                .saturating_add(RocksDbWeight::get().writes(9))
        }
        fn create_nft_contract() -> Weight {
            Weight::from_parts(227_640_000, 0)
                .saturating_add(RocksDbWeight::get().reads(12))
                .saturating_add(RocksDbWeight::get().writes(10))
        }
        fn create_predeploy_contract() -> Weight {
            Weight::from_parts(233_183_000, 0)
                .saturating_add(RocksDbWeight::get().reads(11))
                .saturating_add(RocksDbWeight::get().writes(9))
        }
        fn call() -> Weight {
            Weight::from_parts(189_885_000, 0)
                .saturating_add(RocksDbWeight::get().reads(11))
                .saturating_add(RocksDbWeight::get().writes(6))
        }
        fn transfer_maintainer() -> Weight {
            Weight::from_parts(122_117_000, 0)
                .saturating_add(RocksDbWeight::get().reads(2))
                .saturating_add(RocksDbWeight::get().writes(1))
        }
        fn publish_contract() -> Weight {
            Weight::from_parts(150_918_000, 0)
                .saturating_add(RocksDbWeight::get().reads(2))
                .saturating_add(RocksDbWeight::get().writes(1))
        }
        fn publish_free() -> Weight {
            Weight::from_parts(40_271_000, 0)
                .saturating_add(RocksDbWeight::get().reads(1))
                .saturating_add(RocksDbWeight::get().writes(1))
        }
        fn enable_contract_development() -> Weight {
            Weight::from_parts(127_492_000, 0)
                .saturating_add(RocksDbWeight::get().reads(1))
                .saturating_add(RocksDbWeight::get().writes(1))
        }
        fn disable_contract_development() -> Weight {
            Weight::from_parts(129_795_000, 0)
                .saturating_add(RocksDbWeight::get().reads(1))
                .saturating_add(RocksDbWeight::get().writes(1))
        }
        /// The range of component `c` is `[0, 61440]`.
        fn set_code(c: u32) -> Weight {
            Weight::from_parts(218_913_195, 0)
                .saturating_add(Weight::from_parts(5_766, 0).saturating_mul(c.into()))
                .saturating_add(RocksDbWeight::get().reads(10))
                .saturating_add(RocksDbWeight::get().writes(9))
        }
        fn selfdestruct() -> Weight {
            Weight::from_parts(246_450_000, 0)
                .saturating_add(RocksDbWeight::get().reads(11))
                .saturating_add(RocksDbWeight::get().writes(8))
        }
    }
}
pub use module::*;
pub use weights::WeightInfo;
/// Storage key size and storage value size.
pub const STORAGE_SIZE: u32 = 64;
/// Remove contract item limit
pub const REMOVE_LIMIT: u32 = 100;
/// Immediate remove contract item limit 50 DB writes
pub const IMMEDIATE_REMOVE_LIMIT: u32 = 50;
/// Type alias for currency balance.
pub type BalanceOf<T> = <<T as Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::Balance;
pub type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;
pub const RESERVE_ID_STORAGE_DEPOSIT: ReserveIdentifier = ReserveIdentifier::EvmStorageDeposit;
pub const RESERVE_ID_DEVELOPER_DEPOSIT: ReserveIdentifier = ReserveIdentifier::EvmDeveloperDeposit;
static ACALA_CONFIG: EvmConfig = EvmConfig {
    refund_sstore_clears: 0,
    sstore_gas_metering: false,
    sstore_revert_under_stipend: false,
    create_contract_limit: Some(MaxCodeSize::get() as usize),
    ..module_evm_utility::evm::Config::london()
};
/// Create an empty contract `contract Empty { }`.
pub const BASE_CREATE_GAS: u64 = 67_066;
/// Call function that just set a storage `function store(uint256 num) public { number = num; }`.
pub const BASE_CALL_GAS: u64 = 43_702;
/// Helper method to calculate `create` weight.
fn create_weight<T: Config>(gas: u64) -> Weight {
    <T as Config>::WeightInfo::create()
        .saturating_add(T::GasToWeight::convert(gas.saturating_sub(BASE_CREATE_GAS)))
}
/// Helper method to calculate `create2` weight.
fn create2_weight<T: Config>(gas: u64) -> Weight {
    <T as Config>::WeightInfo::create2()
        .saturating_add(T::GasToWeight::convert(gas.saturating_sub(BASE_CREATE_GAS)))
}
/// Helper method to calculate `create_predeploy_contract` weight.
fn create_predeploy_contract<T: Config>(gas: u64) -> Weight {
    <T as Config>::WeightInfo::create_predeploy_contract()
        .saturating_add(T::GasToWeight::convert(gas.saturating_sub(BASE_CREATE_GAS)))
}
/// Helper method to calculate `create_nft_contract` weight.
fn create_nft_contract<T: Config>(gas: u64) -> Weight {
    <T as Config>::WeightInfo::create_nft_contract()
        .saturating_add(T::GasToWeight::convert(gas.saturating_sub(BASE_CREATE_GAS)))
}
/// Helper method to calculate `call` weight.
fn call_weight<T: Config>(gas: u64) -> Weight {
    <T as Config>::WeightInfo::call()
        .saturating_add(T::GasToWeight::convert(gas.saturating_sub(BASE_CALL_GAS)))
}
/**The `pallet` module in each FRAME pallet hosts the most important items needed
to construct this pallet.

The main components of this pallet are:
- [`Pallet`], which implements all of the dispatchable extrinsics of the pallet, among
other public functions.
	- The subset of the functions that are dispatchable can be identified either in the
	[`dispatchables`] module or in the [`Call`] enum.
- [`storage_types`], which contains the list of all types that are representing a
storage item. Otherwise, all storage items are listed among [*Type Definitions*](#types).
- [`Config`], which contains the configuration trait of this pallet.
- [`Event`] and [`Error`], which are listed among the [*Enums*](#enums).
		*/
pub mod module {
    use super::*;
    pub struct MaxCodeSize;
    impl MaxCodeSize {
        /// Returns the value of this parameter type.
        pub const fn get() -> u32 {
            60 * 1024
        }
    }
    impl<_I: From<u32>> ::frame_support::traits::Get<_I> for MaxCodeSize {
        fn get() -> _I {
            _I::from(Self::get())
        }
    }
    impl ::frame_support::traits::TypedGet for MaxCodeSize {
        type Type = u32;
        fn get() -> u32 {
            Self::get()
        }
    }
    /**
Configuration trait of this pallet.

The main purpose of this trait is to act as an interface between this pallet and the runtime in
which it is embedded in. A type, function, or constant in this trait is essentially left to be
configured by the runtime that includes this pallet.

Consequently, a runtime that wants to include this pallet must implement this trait.*/
    /// EVM module trait
    pub trait Config: frame_system::Config + pallet_timestamp::Config {
        /// Mapping from address to account id.
        type AddressMapping: AddressMapping<Self::AccountId>;
        /// Currency type for withdraw and balance storage.
        type Currency: NamedReservableCurrency<
                Self::AccountId,
                ReserveIdentifier = ReserveIdentifier,
                Balance = Balance,
            >;
        /// Merge free balance from source to dest.
        type TransferAll: TransferAll<Self::AccountId>;
        /// Charge extra bytes for creating a contract, would be reserved until
        /// the contract deleted.
        type NewContractExtraBytes: Get<u32>;
        /// Storage required for per byte.
        type StorageDepositPerByte: Get<BalanceOf<Self>>;
        /// Tx fee required for per gas.
        /// Provide to the client
        type TxFeePerGas: Get<BalanceOf<Self>>;
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Precompiles associated with this EVM engine.
        type PrecompilesType: PrecompileSet;
        type PrecompilesValue: Get<Self::PrecompilesType>;
        /// Convert gas to weight.
        type GasToWeight: Convert<u64, Weight>;
        /// ChargeTransactionPayment convert weight to fee.
        type ChargeTransactionPayment: TransactionPayment<
                Self::AccountId,
                BalanceOf<Self>,
                NegativeImbalanceOf<Self>,
            >;
        /// EVM config used in the module.
        fn config() -> &'static EvmConfig {
            &ACALA_CONFIG
        }
        /// Required origin for creating system contract.
        type NetworkContractOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// The EVM address for creating system contract.
        type NetworkContractSource: Get<EvmAddress>;
        /// Deposit for the developer.
        type DeveloperDeposit: Get<BalanceOf<Self>>;
        /// The fee for publishing the contract.
        type PublicationFee: Get<BalanceOf<Self>>;
        type TreasuryAccount: Get<Self::AccountId>;
        type FreePublicationOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        /// EVM execution runner.
        type Runner: Runner<Self>;
        /// Find author for the current block.
        type FindAuthor: FindAuthor<Self::AccountId>;
        /// Dispatchable tasks
        type Task: DispatchableTask
            + FullCodec
            + Debug
            + Clone
            + PartialEq
            + TypeInfo
            + From<EvmTask<Self>>;
        /// Idle scheduler for the evm task.
        type IdleScheduler: IdleScheduler<Self::Task>;
        /// Weight information for the extrinsics in this module.
        type WeightInfo: WeightInfo;
    }
    pub struct ContractInfo {
        pub code_hash: H256,
        pub maintainer: EvmAddress,
        pub published: bool,
    }
    #[automatically_derived]
    impl ::core::clone::Clone for ContractInfo {
        #[inline]
        fn clone(&self) -> ContractInfo {
            ContractInfo {
                code_hash: ::core::clone::Clone::clone(&self.code_hash),
                maintainer: ::core::clone::Clone::clone(&self.maintainer),
                published: ::core::clone::Clone::clone(&self.published),
            }
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralEq for ContractInfo {}
    #[automatically_derived]
    impl ::core::cmp::Eq for ContractInfo {
        #[inline]
        #[doc(hidden)]
        #[no_coverage]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<H256>;
            let _: ::core::cmp::AssertParamIsEq<EvmAddress>;
            let _: ::core::cmp::AssertParamIsEq<bool>;
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for ContractInfo {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for ContractInfo {
        #[inline]
        fn eq(&self, other: &ContractInfo) -> bool {
            self.code_hash == other.code_hash && self.maintainer == other.maintainer
                && self.published == other.published
        }
    }
    impl core::fmt::Debug for ContractInfo {
        fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
            fmt.write_str("<wasm:stripped>")
        }
    }
    #[allow(deprecated)]
    const _: () = {
        #[automatically_derived]
        impl ::codec::Encode for ContractInfo {
            fn size_hint(&self) -> usize {
                0_usize
                    .saturating_add(::codec::Encode::size_hint(&self.code_hash))
                    .saturating_add(::codec::Encode::size_hint(&self.maintainer))
                    .saturating_add(::codec::Encode::size_hint(&self.published))
            }
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                ::codec::Encode::encode_to(&self.code_hash, __codec_dest_edqy);
                ::codec::Encode::encode_to(&self.maintainer, __codec_dest_edqy);
                ::codec::Encode::encode_to(&self.published, __codec_dest_edqy);
            }
        }
        #[automatically_derived]
        impl ::codec::EncodeLike for ContractInfo {}
    };
    #[allow(deprecated)]
    const _: () = {
        #[automatically_derived]
        impl ::codec::Decode for ContractInfo {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                ::core::result::Result::Ok(ContractInfo {
                    code_hash: {
                        let __codec_res_edqy = <H256 as ::codec::Decode>::decode(
                            __codec_input_edqy,
                        );
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `ContractInfo::code_hash`"),
                                );
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => {
                                __codec_res_edqy
                            }
                        }
                    },
                    maintainer: {
                        let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                            __codec_input_edqy,
                        );
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `ContractInfo::maintainer`"),
                                );
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => {
                                __codec_res_edqy
                            }
                        }
                    },
                    published: {
                        let __codec_res_edqy = <bool as ::codec::Decode>::decode(
                            __codec_input_edqy,
                        );
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `ContractInfo::published`"),
                                );
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => {
                                __codec_res_edqy
                            }
                        }
                    },
                })
            }
        }
    };
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl ::scale_info::TypeInfo for ContractInfo {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                ::scale_info::Type::builder()
                    .path(::scale_info::Path::new("ContractInfo", "module_evm::module"))
                    .type_params(::alloc::vec::Vec::new())
                    .composite(
                        ::scale_info::build::Fields::named()
                            .field(|f| {
                                f.ty::<H256>().name("code_hash").type_name("H256")
                            })
                            .field(|f| {
                                f
                                    .ty::<EvmAddress>()
                                    .name("maintainer")
                                    .type_name("EvmAddress")
                            })
                            .field(|f| {
                                f.ty::<bool>().name("published").type_name("bool")
                            }),
                    )
            }
        }
    };
    pub struct AccountInfo<Index> {
        pub nonce: Index,
        pub contract_info: Option<ContractInfo>,
    }
    #[automatically_derived]
    impl<Index: ::core::clone::Clone> ::core::clone::Clone for AccountInfo<Index> {
        #[inline]
        fn clone(&self) -> AccountInfo<Index> {
            AccountInfo {
                nonce: ::core::clone::Clone::clone(&self.nonce),
                contract_info: ::core::clone::Clone::clone(&self.contract_info),
            }
        }
    }
    #[automatically_derived]
    impl<Index> ::core::marker::StructuralEq for AccountInfo<Index> {}
    #[automatically_derived]
    impl<Index: ::core::cmp::Eq> ::core::cmp::Eq for AccountInfo<Index> {
        #[inline]
        #[doc(hidden)]
        #[no_coverage]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<Index>;
            let _: ::core::cmp::AssertParamIsEq<Option<ContractInfo>>;
        }
    }
    #[automatically_derived]
    impl<Index> ::core::marker::StructuralPartialEq for AccountInfo<Index> {}
    #[automatically_derived]
    impl<Index: ::core::cmp::PartialEq> ::core::cmp::PartialEq for AccountInfo<Index> {
        #[inline]
        fn eq(&self, other: &AccountInfo<Index>) -> bool {
            self.nonce == other.nonce && self.contract_info == other.contract_info
        }
    }
    impl<Index> core::fmt::Debug for AccountInfo<Index>
    where
        Index: core::fmt::Debug,
    {
        fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
            fmt.write_str("<wasm:stripped>")
        }
    }
    #[allow(deprecated)]
    const _: () = {
        #[automatically_derived]
        impl<Index> ::codec::Encode for AccountInfo<Index>
        where
            Index: ::codec::Encode,
            Index: ::codec::Encode,
        {
            fn size_hint(&self) -> usize {
                0_usize
                    .saturating_add(::codec::Encode::size_hint(&self.nonce))
                    .saturating_add(::codec::Encode::size_hint(&self.contract_info))
            }
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                ::codec::Encode::encode_to(&self.nonce, __codec_dest_edqy);
                ::codec::Encode::encode_to(&self.contract_info, __codec_dest_edqy);
            }
        }
        #[automatically_derived]
        impl<Index> ::codec::EncodeLike for AccountInfo<Index>
        where
            Index: ::codec::Encode,
            Index: ::codec::Encode,
        {}
    };
    #[allow(deprecated)]
    const _: () = {
        #[automatically_derived]
        impl<Index> ::codec::Decode for AccountInfo<Index>
        where
            Index: ::codec::Decode,
            Index: ::codec::Decode,
        {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                ::core::result::Result::Ok(AccountInfo::<Index> {
                    nonce: {
                        let __codec_res_edqy = <Index as ::codec::Decode>::decode(
                            __codec_input_edqy,
                        );
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `AccountInfo::nonce`"),
                                );
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => {
                                __codec_res_edqy
                            }
                        }
                    },
                    contract_info: {
                        let __codec_res_edqy = <Option<
                            ContractInfo,
                        > as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `AccountInfo::contract_info`"),
                                );
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => {
                                __codec_res_edqy
                            }
                        }
                    },
                })
            }
        }
    };
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl<Index> ::scale_info::TypeInfo for AccountInfo<Index>
        where
            Index: ::scale_info::TypeInfo + 'static,
            Index: ::scale_info::TypeInfo + 'static,
        {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                ::scale_info::Type::builder()
                    .path(::scale_info::Path::new("AccountInfo", "module_evm::module"))
                    .type_params(
                        <[_]>::into_vec(
                            #[rustc_box]
                            ::alloc::boxed::Box::new([
                                ::scale_info::TypeParameter::new(
                                    "Index",
                                    ::core::option::Option::Some(
                                        ::scale_info::meta_type::<Index>(),
                                    ),
                                ),
                            ]),
                        ),
                    )
                    .composite(
                        ::scale_info::build::Fields::named()
                            .field(|f| f.ty::<Index>().name("nonce").type_name("Index"))
                            .field(|f| {
                                f
                                    .ty::<Option<ContractInfo>>()
                                    .name("contract_info")
                                    .type_name("Option<ContractInfo>")
                            }),
                    )
            }
        }
    };
    impl<Index> AccountInfo<Index> {
        pub fn new(nonce: Index, contract_info: Option<ContractInfo>) -> Self {
            Self { nonce, contract_info }
        }
    }
    pub struct CodeInfo {
        pub code_size: u32,
        pub ref_count: u32,
    }
    #[automatically_derived]
    impl ::core::clone::Clone for CodeInfo {
        #[inline]
        fn clone(&self) -> CodeInfo {
            let _: ::core::clone::AssertParamIsClone<u32>;
            *self
        }
    }
    #[automatically_derived]
    impl ::core::marker::Copy for CodeInfo {}
    #[automatically_derived]
    impl ::core::marker::StructuralEq for CodeInfo {}
    #[automatically_derived]
    impl ::core::cmp::Eq for CodeInfo {
        #[inline]
        #[doc(hidden)]
        #[no_coverage]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<u32>;
        }
    }
    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for CodeInfo {}
    #[automatically_derived]
    impl ::core::cmp::PartialEq for CodeInfo {
        #[inline]
        fn eq(&self, other: &CodeInfo) -> bool {
            self.code_size == other.code_size && self.ref_count == other.ref_count
        }
    }
    impl core::fmt::Debug for CodeInfo {
        fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
            fmt.write_str("<wasm:stripped>")
        }
    }
    #[allow(deprecated)]
    const _: () = {
        #[automatically_derived]
        impl ::codec::Encode for CodeInfo {
            fn size_hint(&self) -> usize {
                0_usize
                    .saturating_add(::codec::Encode::size_hint(&self.code_size))
                    .saturating_add(::codec::Encode::size_hint(&self.ref_count))
            }
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                ::codec::Encode::encode_to(&self.code_size, __codec_dest_edqy);
                ::codec::Encode::encode_to(&self.ref_count, __codec_dest_edqy);
            }
        }
        #[automatically_derived]
        impl ::codec::EncodeLike for CodeInfo {}
    };
    #[allow(deprecated)]
    const _: () = {
        #[automatically_derived]
        impl ::codec::Decode for CodeInfo {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                ::core::result::Result::Ok(CodeInfo {
                    code_size: {
                        let __codec_res_edqy = <u32 as ::codec::Decode>::decode(
                            __codec_input_edqy,
                        );
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `CodeInfo::code_size`"),
                                );
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => {
                                __codec_res_edqy
                            }
                        }
                    },
                    ref_count: {
                        let __codec_res_edqy = <u32 as ::codec::Decode>::decode(
                            __codec_input_edqy,
                        );
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `CodeInfo::ref_count`"),
                                );
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => {
                                __codec_res_edqy
                            }
                        }
                    },
                })
            }
        }
    };
    const _: () = {
        impl ::codec::MaxEncodedLen for CodeInfo {
            fn max_encoded_len() -> ::core::primitive::usize {
                0_usize
                    .saturating_add(<u32>::max_encoded_len())
                    .saturating_add(<u32>::max_encoded_len())
            }
        }
    };
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl ::scale_info::TypeInfo for CodeInfo {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                ::scale_info::Type::builder()
                    .path(::scale_info::Path::new("CodeInfo", "module_evm::module"))
                    .type_params(::alloc::vec::Vec::new())
                    .composite(
                        ::scale_info::build::Fields::named()
                            .field(|f| f.ty::<u32>().name("code_size").type_name("u32"))
                            .field(|f| f.ty::<u32>().name("ref_count").type_name("u32")),
                    )
            }
        }
    };
    /// Account definition used for genesis block construction.
    pub struct GenesisAccount<Balance, Index> {
        /// Account nonce.
        pub nonce: Index,
        /// Account balance.
        pub balance: Balance,
        /// Full account storage.
        pub storage: BTreeMap<H256, H256>,
        /// Account code.
        pub code: Vec<u8>,
        /// If the account should enable contract development mode
        pub enable_contract_development: bool,
    }
    #[automatically_derived]
    impl<Balance: ::core::clone::Clone, Index: ::core::clone::Clone> ::core::clone::Clone
    for GenesisAccount<Balance, Index> {
        #[inline]
        fn clone(&self) -> GenesisAccount<Balance, Index> {
            GenesisAccount {
                nonce: ::core::clone::Clone::clone(&self.nonce),
                balance: ::core::clone::Clone::clone(&self.balance),
                storage: ::core::clone::Clone::clone(&self.storage),
                code: ::core::clone::Clone::clone(&self.code),
                enable_contract_development: ::core::clone::Clone::clone(
                    &self.enable_contract_development,
                ),
            }
        }
    }
    #[automatically_derived]
    impl<Balance, Index> ::core::marker::StructuralEq
    for GenesisAccount<Balance, Index> {}
    #[automatically_derived]
    impl<Balance: ::core::cmp::Eq, Index: ::core::cmp::Eq> ::core::cmp::Eq
    for GenesisAccount<Balance, Index> {
        #[inline]
        #[doc(hidden)]
        #[no_coverage]
        fn assert_receiver_is_total_eq(&self) -> () {
            let _: ::core::cmp::AssertParamIsEq<Index>;
            let _: ::core::cmp::AssertParamIsEq<Balance>;
            let _: ::core::cmp::AssertParamIsEq<BTreeMap<H256, H256>>;
            let _: ::core::cmp::AssertParamIsEq<Vec<u8>>;
            let _: ::core::cmp::AssertParamIsEq<bool>;
        }
    }
    #[automatically_derived]
    impl<Balance, Index> ::core::marker::StructuralPartialEq
    for GenesisAccount<Balance, Index> {}
    #[automatically_derived]
    impl<
        Balance: ::core::cmp::PartialEq,
        Index: ::core::cmp::PartialEq,
    > ::core::cmp::PartialEq for GenesisAccount<Balance, Index> {
        #[inline]
        fn eq(&self, other: &GenesisAccount<Balance, Index>) -> bool {
            self.nonce == other.nonce && self.balance == other.balance
                && self.storage == other.storage && self.code == other.code
                && self.enable_contract_development == other.enable_contract_development
        }
    }
    #[allow(deprecated)]
    const _: () = {
        #[automatically_derived]
        impl<Balance, Index> ::codec::Encode for GenesisAccount<Balance, Index>
        where
            Index: ::codec::Encode,
            Index: ::codec::Encode,
            Balance: ::codec::Encode,
            Balance: ::codec::Encode,
        {
            fn size_hint(&self) -> usize {
                0_usize
                    .saturating_add(::codec::Encode::size_hint(&self.nonce))
                    .saturating_add(::codec::Encode::size_hint(&self.balance))
                    .saturating_add(::codec::Encode::size_hint(&self.storage))
                    .saturating_add(::codec::Encode::size_hint(&self.code))
                    .saturating_add(
                        ::codec::Encode::size_hint(&self.enable_contract_development),
                    )
            }
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                ::codec::Encode::encode_to(&self.nonce, __codec_dest_edqy);
                ::codec::Encode::encode_to(&self.balance, __codec_dest_edqy);
                ::codec::Encode::encode_to(&self.storage, __codec_dest_edqy);
                ::codec::Encode::encode_to(&self.code, __codec_dest_edqy);
                ::codec::Encode::encode_to(
                    &self.enable_contract_development,
                    __codec_dest_edqy,
                );
            }
        }
        #[automatically_derived]
        impl<Balance, Index> ::codec::EncodeLike for GenesisAccount<Balance, Index>
        where
            Index: ::codec::Encode,
            Index: ::codec::Encode,
            Balance: ::codec::Encode,
            Balance: ::codec::Encode,
        {}
    };
    #[allow(deprecated)]
    const _: () = {
        #[automatically_derived]
        impl<Balance, Index> ::codec::Decode for GenesisAccount<Balance, Index>
        where
            Index: ::codec::Decode,
            Index: ::codec::Decode,
            Balance: ::codec::Decode,
            Balance: ::codec::Decode,
        {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                ::core::result::Result::Ok(GenesisAccount::<Balance, Index> {
                    nonce: {
                        let __codec_res_edqy = <Index as ::codec::Decode>::decode(
                            __codec_input_edqy,
                        );
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `GenesisAccount::nonce`"),
                                );
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => {
                                __codec_res_edqy
                            }
                        }
                    },
                    balance: {
                        let __codec_res_edqy = <Balance as ::codec::Decode>::decode(
                            __codec_input_edqy,
                        );
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `GenesisAccount::balance`"),
                                );
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => {
                                __codec_res_edqy
                            }
                        }
                    },
                    storage: {
                        let __codec_res_edqy = <BTreeMap<
                            H256,
                            H256,
                        > as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `GenesisAccount::storage`"),
                                );
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => {
                                __codec_res_edqy
                            }
                        }
                    },
                    code: {
                        let __codec_res_edqy = <Vec<
                            u8,
                        > as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `GenesisAccount::code`"),
                                );
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => {
                                __codec_res_edqy
                            }
                        }
                    },
                    enable_contract_development: {
                        let __codec_res_edqy = <bool as ::codec::Decode>::decode(
                            __codec_input_edqy,
                        );
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e
                                        .chain(
                                            "Could not decode `GenesisAccount::enable_contract_development`",
                                        ),
                                );
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => {
                                __codec_res_edqy
                            }
                        }
                    },
                })
            }
        }
    };
    impl<Balance, Index> core::fmt::Debug for GenesisAccount<Balance, Index>
    where
        Balance: core::fmt::Debug,
        Index: core::fmt::Debug,
    {
        fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
            fmt.write_str("<wasm:stripped>")
        }
    }
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl<Balance, Index> ::scale_info::TypeInfo for GenesisAccount<Balance, Index>
        where
            Index: ::scale_info::TypeInfo + 'static,
            Balance: ::scale_info::TypeInfo + 'static,
            Balance: ::scale_info::TypeInfo + 'static,
            Index: ::scale_info::TypeInfo + 'static,
        {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                ::scale_info::Type::builder()
                    .path(
                        ::scale_info::Path::new("GenesisAccount", "module_evm::module"),
                    )
                    .type_params(
                        <[_]>::into_vec(
                            #[rustc_box]
                            ::alloc::boxed::Box::new([
                                ::scale_info::TypeParameter::new(
                                    "Balance",
                                    ::core::option::Option::Some(
                                        ::scale_info::meta_type::<Balance>(),
                                    ),
                                ),
                                ::scale_info::TypeParameter::new(
                                    "Index",
                                    ::core::option::Option::Some(
                                        ::scale_info::meta_type::<Index>(),
                                    ),
                                ),
                            ]),
                        ),
                    )
                    .docs(&["Account definition used for genesis block construction."])
                    .composite(
                        ::scale_info::build::Fields::named()
                            .field(|f| {
                                f
                                    .ty::<Index>()
                                    .name("nonce")
                                    .type_name("Index")
                                    .docs(&["Account nonce."])
                            })
                            .field(|f| {
                                f
                                    .ty::<Balance>()
                                    .name("balance")
                                    .type_name("Balance")
                                    .docs(&["Account balance."])
                            })
                            .field(|f| {
                                f
                                    .ty::<BTreeMap<H256, H256>>()
                                    .name("storage")
                                    .type_name("BTreeMap<H256, H256>")
                                    .docs(&["Full account storage."])
                            })
                            .field(|f| {
                                f
                                    .ty::<Vec<u8>>()
                                    .name("code")
                                    .type_name("Vec<u8>")
                                    .docs(&["Account code."])
                            })
                            .field(|f| {
                                f
                                    .ty::<bool>()
                                    .name("enable_contract_development")
                                    .type_name("bool")
                                    .docs(
                                        &["If the account should enable contract development mode"],
                                    )
                            }),
                    )
            }
        }
    };
    #[automatically_derived]
    impl<
        Balance: ::core::default::Default,
        Index: ::core::default::Default,
    > ::core::default::Default for GenesisAccount<Balance, Index> {
        #[inline]
        fn default() -> GenesisAccount<Balance, Index> {
            GenesisAccount {
                nonce: ::core::default::Default::default(),
                balance: ::core::default::Default::default(),
                storage: ::core::default::Default::default(),
                code: ::core::default::Default::default(),
                enable_contract_development: ::core::default::Default::default(),
            }
        }
    }
    /// The EVM Chain ID.
    ///
    /// ChainId: u64
    #[allow(type_alias_bounds)]
    ///
    ///Storage type is [`StorageValue`] with value type `u64`.
    pub type ChainId<T: Config> = StorageValue<
        _GeneratedPrefixForStorageChainId<T>,
        u64,
        ValueQuery,
    >;
    /// The EVM accounts info.
    ///
    /// Accounts: map EvmAddress => Option<AccountInfo<T>>
    #[allow(type_alias_bounds)]
    ///
    ///Storage type is [`StorageMap`] with key type `EvmAddress` and value type `AccountInfo < T :: Index >`.
    pub type Accounts<T: Config> = StorageMap<
        _GeneratedPrefixForStorageAccounts<T>,
        Twox64Concat,
        EvmAddress,
        AccountInfo<T::Index>,
        OptionQuery,
    >;
    /// The storage usage for contracts. Including code size, extra bytes and total AccountStorages
    /// size.
    ///
    /// Accounts: map EvmAddress => u32
    #[allow(type_alias_bounds)]
    ///
    ///Storage type is [`StorageMap`] with key type `EvmAddress` and value type `u32`.
    pub type ContractStorageSizes<T: Config> = StorageMap<
        _GeneratedPrefixForStorageContractStorageSizes<T>,
        Twox64Concat,
        EvmAddress,
        u32,
        ValueQuery,
    >;
    /// The storages for EVM contracts.
    ///
    /// AccountStorages: double_map EvmAddress, H256 => H256
    #[allow(type_alias_bounds)]
    ///
    ///Storage type is [`StorageDoubleMap`] with key1 type EvmAddress, key2 type H256 and value type H256.
    pub type AccountStorages<T: Config> = StorageDoubleMap<
        _GeneratedPrefixForStorageAccountStorages<T>,
        Twox64Concat,
        EvmAddress,
        Blake2_128Concat,
        H256,
        H256,
        ValueQuery,
    >;
    /// The code for EVM contracts.
    /// Key is Keccak256 hash of code.
    ///
    /// Codes: H256 => Vec<u8>
    #[allow(type_alias_bounds)]
    ///
    ///Storage type is [`StorageMap`] with key type `H256` and value type `BoundedVec < u8, MaxCodeSize >`.
    pub type Codes<T: Config> = StorageMap<
        _GeneratedPrefixForStorageCodes<T>,
        Identity,
        H256,
        BoundedVec<u8, MaxCodeSize>,
        ValueQuery,
    >;
    /// The code info for EVM contracts.
    /// Key is Keccak256 hash of code.
    ///
    /// CodeInfos: H256 => Option<CodeInfo>
    #[allow(type_alias_bounds)]
    ///
    ///Storage type is [`StorageMap`] with key type `H256` and value type `CodeInfo`.
    pub type CodeInfos<T: Config> = StorageMap<
        _GeneratedPrefixForStorageCodeInfos<T>,
        Identity,
        H256,
        CodeInfo,
        OptionQuery,
    >;
    /// Next available system contract address.
    ///
    /// NetworkContractIndex: u64
    #[allow(type_alias_bounds)]
    ///
    ///Storage type is [`StorageValue`] with value type `u64`.
    pub type NetworkContractIndex<T: Config> = StorageValue<
        _GeneratedPrefixForStorageNetworkContractIndex<T>,
        u64,
        ValueQuery,
    >;
    /// Extrinsics origin for the current transaction.
    ///
    /// ExtrinsicOrigin: Option<AccountId>
    #[allow(type_alias_bounds)]
    ///
    ///Storage type is [`StorageValue`] with value type `T :: AccountId`.
    pub type ExtrinsicOrigin<T: Config> = StorageValue<
        _GeneratedPrefixForStorageExtrinsicOrigin<T>,
        T::AccountId,
        OptionQuery,
    >;
    /// Xcm origin for the current transaction.
    ///
    /// XcmOrigin: Option<Vec<AccountId>>
    #[allow(type_alias_bounds)]
    ///
    ///Storage type is [`StorageValue`] with value type `Vec < T :: AccountId >`.
    pub type XcmOrigin<T: Config> = StorageValue<
        _GeneratedPrefixForStorageXcmOrigin<T>,
        Vec<T::AccountId>,
        OptionQuery,
    >;
    /**
					Can be used to configure the
					[genesis state](https://docs.substrate.io/build/genesis-configuration/)
					of this pallet.
					*/
    #[serde(rename_all = "camelCase")]
    #[serde(deny_unknown_fields)]
    #[serde(bound(serialize = ""))]
    #[serde(bound(deserialize = ""))]
    #[serde(crate = "frame_support::serde")]
    pub struct GenesisConfig<T: Config> {
        pub chain_id: u64,
        pub accounts: BTreeMap<EvmAddress, GenesisAccount<BalanceOf<T>, T::Index>>,
    }
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        use frame_support::serde as _serde;
        #[automatically_derived]
        impl<T: Config> frame_support::serde::Serialize for GenesisConfig<T> {
            fn serialize<__S>(
                &self,
                __serializer: __S,
            ) -> frame_support::serde::__private::Result<__S::Ok, __S::Error>
            where
                __S: frame_support::serde::Serializer,
            {
                let mut __serde_state = match _serde::Serializer::serialize_struct(
                    __serializer,
                    "GenesisConfig",
                    false as usize + 1 + 1,
                ) {
                    _serde::__private::Ok(__val) => __val,
                    _serde::__private::Err(__err) => {
                        return _serde::__private::Err(__err);
                    }
                };
                match _serde::ser::SerializeStruct::serialize_field(
                    &mut __serde_state,
                    "chainId",
                    &self.chain_id,
                ) {
                    _serde::__private::Ok(__val) => __val,
                    _serde::__private::Err(__err) => {
                        return _serde::__private::Err(__err);
                    }
                };
                match _serde::ser::SerializeStruct::serialize_field(
                    &mut __serde_state,
                    "accounts",
                    &self.accounts,
                ) {
                    _serde::__private::Ok(__val) => __val,
                    _serde::__private::Err(__err) => {
                        return _serde::__private::Err(__err);
                    }
                };
                _serde::ser::SerializeStruct::end(__serde_state)
            }
        }
    };
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        use frame_support::serde as _serde;
        #[automatically_derived]
        impl<'de, T: Config> frame_support::serde::Deserialize<'de>
        for GenesisConfig<T> {
            fn deserialize<__D>(
                __deserializer: __D,
            ) -> frame_support::serde::__private::Result<Self, __D::Error>
            where
                __D: frame_support::serde::Deserializer<'de>,
            {
                #[allow(non_camel_case_types)]
                #[doc(hidden)]
                enum __Field {
                    __field0,
                    __field1,
                }
                #[doc(hidden)]
                struct __FieldVisitor;
                impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                    type Value = __Field;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(
                            __formatter,
                            "field identifier",
                        )
                    }
                    fn visit_u64<__E>(
                        self,
                        __value: u64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            0u64 => _serde::__private::Ok(__Field::__field0),
                            1u64 => _serde::__private::Ok(__Field::__field1),
                            _ => {
                                _serde::__private::Err(
                                    _serde::de::Error::invalid_value(
                                        _serde::de::Unexpected::Unsigned(__value),
                                        &"field index 0 <= i < 2",
                                    ),
                                )
                            }
                        }
                    }
                    fn visit_str<__E>(
                        self,
                        __value: &str,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "chainId" => _serde::__private::Ok(__Field::__field0),
                            "accounts" => _serde::__private::Ok(__Field::__field1),
                            _ => {
                                _serde::__private::Err(
                                    _serde::de::Error::unknown_field(__value, FIELDS),
                                )
                            }
                        }
                    }
                    fn visit_bytes<__E>(
                        self,
                        __value: &[u8],
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"chainId" => _serde::__private::Ok(__Field::__field0),
                            b"accounts" => _serde::__private::Ok(__Field::__field1),
                            _ => {
                                let __value = &_serde::__private::from_utf8_lossy(__value);
                                _serde::__private::Err(
                                    _serde::de::Error::unknown_field(__value, FIELDS),
                                )
                            }
                        }
                    }
                }
                impl<'de> _serde::Deserialize<'de> for __Field {
                    #[inline]
                    fn deserialize<__D>(
                        __deserializer: __D,
                    ) -> _serde::__private::Result<Self, __D::Error>
                    where
                        __D: _serde::Deserializer<'de>,
                    {
                        _serde::Deserializer::deserialize_identifier(
                            __deserializer,
                            __FieldVisitor,
                        )
                    }
                }
                #[doc(hidden)]
                struct __Visitor<'de, T: Config> {
                    marker: _serde::__private::PhantomData<GenesisConfig<T>>,
                    lifetime: _serde::__private::PhantomData<&'de ()>,
                }
                impl<'de, T: Config> _serde::de::Visitor<'de> for __Visitor<'de, T> {
                    type Value = GenesisConfig<T>;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(
                            __formatter,
                            "struct GenesisConfig",
                        )
                    }
                    #[inline]
                    fn visit_seq<__A>(
                        self,
                        mut __seq: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::SeqAccess<'de>,
                    {
                        let __field0 = match match _serde::de::SeqAccess::next_element::<
                            u64,
                        >(&mut __seq) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        } {
                            _serde::__private::Some(__value) => __value,
                            _serde::__private::None => {
                                return _serde::__private::Err(
                                    _serde::de::Error::invalid_length(
                                        0usize,
                                        &"struct GenesisConfig with 2 elements",
                                    ),
                                );
                            }
                        };
                        let __field1 = match match _serde::de::SeqAccess::next_element::<
                            BTreeMap<EvmAddress, GenesisAccount<BalanceOf<T>, T::Index>>,
                        >(&mut __seq) {
                            _serde::__private::Ok(__val) => __val,
                            _serde::__private::Err(__err) => {
                                return _serde::__private::Err(__err);
                            }
                        } {
                            _serde::__private::Some(__value) => __value,
                            _serde::__private::None => {
                                return _serde::__private::Err(
                                    _serde::de::Error::invalid_length(
                                        1usize,
                                        &"struct GenesisConfig with 2 elements",
                                    ),
                                );
                            }
                        };
                        _serde::__private::Ok(GenesisConfig {
                            chain_id: __field0,
                            accounts: __field1,
                        })
                    }
                    #[inline]
                    fn visit_map<__A>(
                        self,
                        mut __map: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::MapAccess<'de>,
                    {
                        let mut __field0: _serde::__private::Option<u64> = _serde::__private::None;
                        let mut __field1: _serde::__private::Option<
                            BTreeMap<EvmAddress, GenesisAccount<BalanceOf<T>, T::Index>>,
                        > = _serde::__private::None;
                        while let _serde::__private::Some(__key)
                            = match _serde::de::MapAccess::next_key::<
                                __Field,
                            >(&mut __map) {
                                _serde::__private::Ok(__val) => __val,
                                _serde::__private::Err(__err) => {
                                    return _serde::__private::Err(__err);
                                }
                            } {
                            match __key {
                                __Field::__field0 => {
                                    if _serde::__private::Option::is_some(&__field0) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "chainId",
                                            ),
                                        );
                                    }
                                    __field0 = _serde::__private::Some(
                                        match _serde::de::MapAccess::next_value::<u64>(&mut __map) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    );
                                }
                                __Field::__field1 => {
                                    if _serde::__private::Option::is_some(&__field1) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "accounts",
                                            ),
                                        );
                                    }
                                    __field1 = _serde::__private::Some(
                                        match _serde::de::MapAccess::next_value::<
                                            BTreeMap<EvmAddress, GenesisAccount<BalanceOf<T>, T::Index>>,
                                        >(&mut __map) {
                                            _serde::__private::Ok(__val) => __val,
                                            _serde::__private::Err(__err) => {
                                                return _serde::__private::Err(__err);
                                            }
                                        },
                                    );
                                }
                            }
                        }
                        let __field0 = match __field0 {
                            _serde::__private::Some(__field0) => __field0,
                            _serde::__private::None => {
                                match _serde::__private::de::missing_field("chainId") {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            }
                        };
                        let __field1 = match __field1 {
                            _serde::__private::Some(__field1) => __field1,
                            _serde::__private::None => {
                                match _serde::__private::de::missing_field("accounts") {
                                    _serde::__private::Ok(__val) => __val,
                                    _serde::__private::Err(__err) => {
                                        return _serde::__private::Err(__err);
                                    }
                                }
                            }
                        };
                        _serde::__private::Ok(GenesisConfig {
                            chain_id: __field0,
                            accounts: __field1,
                        })
                    }
                }
                #[doc(hidden)]
                const FIELDS: &'static [&'static str] = &["chainId", "accounts"];
                _serde::Deserializer::deserialize_struct(
                    __deserializer,
                    "GenesisConfig",
                    FIELDS,
                    __Visitor {
                        marker: _serde::__private::PhantomData::<GenesisConfig<T>>,
                        lifetime: _serde::__private::PhantomData,
                    },
                )
            }
        }
    };
    const _: () = {
        impl<T: Config> core::default::Default for GenesisConfig<T> {
            fn default() -> Self {
                Self {
                    chain_id: core::default::Default::default(),
                    accounts: core::default::Default::default(),
                }
            }
        }
    };
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            use sp_std::rc::Rc;
            let source = T::NetworkContractSource::get();
            self.accounts
                .iter()
                .for_each(|(address, account)| {
                    let account_id = T::AddressMapping::get_account_id(address);
                    let account_info = <AccountInfo<T::Index>>::new(account.nonce, None);
                    <Accounts<T>>::insert(address, account_info);
                    let amount = if account.balance.is_zero() {
                        <T::Currency as Currency<T::AccountId>>::minimum_balance()
                    } else {
                        account.balance
                    };
                    T::Currency::deposit_creating(&account_id, amount);
                    if account.enable_contract_development {
                        T::Currency::ensure_reserved_named(
                                &RESERVE_ID_DEVELOPER_DEPOSIT,
                                &account_id,
                                T::DeveloperDeposit::get(),
                            )
                            .expect(
                                "Failed to reserve developer deposit. Please make sure the account have enough balance.",
                            );
                    }
                    if !account.code.is_empty() {
                        let vicinity = Vicinity {
                            gas_price: U256::one(),
                            ..Default::default()
                        };
                        let context = Context {
                            caller: source,
                            address: *address,
                            apparent_value: Default::default(),
                        };
                        let metadata = StackSubstateMetadata::new(
                            210_000,
                            1000,
                            T::config(),
                        );
                        let state = SubstrateStackState::<T>::new(&vicinity, metadata);
                        let mut executor = StackExecutor::new_with_precompiles(
                            state,
                            T::config(),
                            &(),
                        );
                        let mut runtime = evm::Runtime::new(
                            Rc::new(account.code.clone()),
                            Rc::new(Vec::new()),
                            context,
                            T::config(),
                        );
                        let reason = executor.execute(&mut runtime);
                        if !reason.is_succeed() {
                            ::core::panicking::panic_fmt(
                                format_args!(
                                    "Genesis contract failed to execute, error: {0:?}", reason
                                ),
                            )
                        }
                        let out = runtime.machine().return_value();
                        <Pallet<T>>::create_contract(source, *address, true, out);
                        for (index, value) in &account.storage {
                            AccountStorages::<T>::insert(address, index, value);
                        }
                    }
                });
            ChainId::<T>::put(self.chain_id);
            NetworkContractIndex::<T>::put(MIRRORED_NFT_ADDRESS_START);
        }
    }
    /// EVM events
    #[scale_info(skip_type_params(T), capture_docs = "always")]
    pub enum Event<T: Config> {
        /// A contract has been created at given
        Created {
            from: EvmAddress,
            contract: EvmAddress,
            logs: Vec<Log>,
            used_gas: u64,
            used_storage: i32,
        },
        /// A contract was attempted to be created, but the execution failed.
        CreatedFailed {
            from: EvmAddress,
            contract: EvmAddress,
            exit_reason: ExitReason,
            logs: Vec<Log>,
            used_gas: u64,
            used_storage: i32,
        },
        /// A contract has been executed successfully with states applied.
        Executed {
            from: EvmAddress,
            contract: EvmAddress,
            logs: Vec<Log>,
            used_gas: u64,
            used_storage: i32,
        },
        /// A contract has been executed with errors. States are reverted with
        /// only gas fees applied.
        ExecutedFailed {
            from: EvmAddress,
            contract: EvmAddress,
            exit_reason: ExitReason,
            output: Vec<u8>,
            logs: Vec<Log>,
            used_gas: u64,
            used_storage: i32,
        },
        /// Transferred maintainer.
        TransferredMaintainer { contract: EvmAddress, new_maintainer: EvmAddress },
        /// Enabled contract development.
        ContractDevelopmentEnabled { who: T::AccountId },
        /// Disabled contract development.
        ContractDevelopmentDisabled { who: T::AccountId },
        /// Published contract.
        ContractPublished { contract: EvmAddress },
        /// Set contract code.
        ContractSetCode { contract: EvmAddress },
        /// Selfdestructed contract code.
        ContractSelfdestructed { contract: EvmAddress },
        #[doc(hidden)]
        #[codec(skip)]
        __Ignore(frame_support::sp_std::marker::PhantomData<(T)>, frame_support::Never),
    }
    const _: () = {
        impl<T: Config> core::clone::Clone for Event<T> {
            fn clone(&self) -> Self {
                match self {
                    Self::Created {
                        ref from,
                        ref contract,
                        ref logs,
                        ref used_gas,
                        ref used_storage,
                    } => {
                        Self::Created {
                            from: core::clone::Clone::clone(from),
                            contract: core::clone::Clone::clone(contract),
                            logs: core::clone::Clone::clone(logs),
                            used_gas: core::clone::Clone::clone(used_gas),
                            used_storage: core::clone::Clone::clone(used_storage),
                        }
                    }
                    Self::CreatedFailed {
                        ref from,
                        ref contract,
                        ref exit_reason,
                        ref logs,
                        ref used_gas,
                        ref used_storage,
                    } => {
                        Self::CreatedFailed {
                            from: core::clone::Clone::clone(from),
                            contract: core::clone::Clone::clone(contract),
                            exit_reason: core::clone::Clone::clone(exit_reason),
                            logs: core::clone::Clone::clone(logs),
                            used_gas: core::clone::Clone::clone(used_gas),
                            used_storage: core::clone::Clone::clone(used_storage),
                        }
                    }
                    Self::Executed {
                        ref from,
                        ref contract,
                        ref logs,
                        ref used_gas,
                        ref used_storage,
                    } => {
                        Self::Executed {
                            from: core::clone::Clone::clone(from),
                            contract: core::clone::Clone::clone(contract),
                            logs: core::clone::Clone::clone(logs),
                            used_gas: core::clone::Clone::clone(used_gas),
                            used_storage: core::clone::Clone::clone(used_storage),
                        }
                    }
                    Self::ExecutedFailed {
                        ref from,
                        ref contract,
                        ref exit_reason,
                        ref output,
                        ref logs,
                        ref used_gas,
                        ref used_storage,
                    } => {
                        Self::ExecutedFailed {
                            from: core::clone::Clone::clone(from),
                            contract: core::clone::Clone::clone(contract),
                            exit_reason: core::clone::Clone::clone(exit_reason),
                            output: core::clone::Clone::clone(output),
                            logs: core::clone::Clone::clone(logs),
                            used_gas: core::clone::Clone::clone(used_gas),
                            used_storage: core::clone::Clone::clone(used_storage),
                        }
                    }
                    Self::TransferredMaintainer { ref contract, ref new_maintainer } => {
                        Self::TransferredMaintainer {
                            contract: core::clone::Clone::clone(contract),
                            new_maintainer: core::clone::Clone::clone(new_maintainer),
                        }
                    }
                    Self::ContractDevelopmentEnabled { ref who } => {
                        Self::ContractDevelopmentEnabled {
                            who: core::clone::Clone::clone(who),
                        }
                    }
                    Self::ContractDevelopmentDisabled { ref who } => {
                        Self::ContractDevelopmentDisabled {
                            who: core::clone::Clone::clone(who),
                        }
                    }
                    Self::ContractPublished { ref contract } => {
                        Self::ContractPublished {
                            contract: core::clone::Clone::clone(contract),
                        }
                    }
                    Self::ContractSetCode { ref contract } => {
                        Self::ContractSetCode {
                            contract: core::clone::Clone::clone(contract),
                        }
                    }
                    Self::ContractSelfdestructed { ref contract } => {
                        Self::ContractSelfdestructed {
                            contract: core::clone::Clone::clone(contract),
                        }
                    }
                    Self::__Ignore(ref _0, ref _1) => {
                        Self::__Ignore(
                            core::clone::Clone::clone(_0),
                            core::clone::Clone::clone(_1),
                        )
                    }
                }
            }
        }
    };
    const _: () = {
        impl<T: Config> core::cmp::Eq for Event<T> {}
    };
    const _: () = {
        impl<T: Config> core::cmp::PartialEq for Event<T> {
            fn eq(&self, other: &Self) -> bool {
                match (self, other) {
                    (
                        Self::Created { from, contract, logs, used_gas, used_storage },
                        Self::Created {
                            from: _0,
                            contract: _1,
                            logs: _2,
                            used_gas: _3,
                            used_storage: _4,
                        },
                    ) => {
                        true && from == _0 && contract == _1 && logs == _2
                            && used_gas == _3 && used_storage == _4
                    }
                    (
                        Self::CreatedFailed {
                            from,
                            contract,
                            exit_reason,
                            logs,
                            used_gas,
                            used_storage,
                        },
                        Self::CreatedFailed {
                            from: _0,
                            contract: _1,
                            exit_reason: _2,
                            logs: _3,
                            used_gas: _4,
                            used_storage: _5,
                        },
                    ) => {
                        true && from == _0 && contract == _1 && exit_reason == _2
                            && logs == _3 && used_gas == _4 && used_storage == _5
                    }
                    (
                        Self::Executed { from, contract, logs, used_gas, used_storage },
                        Self::Executed {
                            from: _0,
                            contract: _1,
                            logs: _2,
                            used_gas: _3,
                            used_storage: _4,
                        },
                    ) => {
                        true && from == _0 && contract == _1 && logs == _2
                            && used_gas == _3 && used_storage == _4
                    }
                    (
                        Self::ExecutedFailed {
                            from,
                            contract,
                            exit_reason,
                            output,
                            logs,
                            used_gas,
                            used_storage,
                        },
                        Self::ExecutedFailed {
                            from: _0,
                            contract: _1,
                            exit_reason: _2,
                            output: _3,
                            logs: _4,
                            used_gas: _5,
                            used_storage: _6,
                        },
                    ) => {
                        true && from == _0 && contract == _1 && exit_reason == _2
                            && output == _3 && logs == _4 && used_gas == _5
                            && used_storage == _6
                    }
                    (
                        Self::TransferredMaintainer { contract, new_maintainer },
                        Self::TransferredMaintainer { contract: _0, new_maintainer: _1 },
                    ) => true && contract == _0 && new_maintainer == _1,
                    (
                        Self::ContractDevelopmentEnabled { who },
                        Self::ContractDevelopmentEnabled { who: _0 },
                    ) => true && who == _0,
                    (
                        Self::ContractDevelopmentDisabled { who },
                        Self::ContractDevelopmentDisabled { who: _0 },
                    ) => true && who == _0,
                    (
                        Self::ContractPublished { contract },
                        Self::ContractPublished { contract: _0 },
                    ) => true && contract == _0,
                    (
                        Self::ContractSetCode { contract },
                        Self::ContractSetCode { contract: _0 },
                    ) => true && contract == _0,
                    (
                        Self::ContractSelfdestructed { contract },
                        Self::ContractSelfdestructed { contract: _0 },
                    ) => true && contract == _0,
                    (Self::__Ignore(_0, _1), Self::__Ignore(_0_other, _1_other)) => {
                        true && _0 == _0_other && _1 == _1_other
                    }
                    (Self::Created { .. }, Self::CreatedFailed { .. }) => false,
                    (Self::Created { .. }, Self::Executed { .. }) => false,
                    (Self::Created { .. }, Self::ExecutedFailed { .. }) => false,
                    (Self::Created { .. }, Self::TransferredMaintainer { .. }) => false,
                    (Self::Created { .. }, Self::ContractDevelopmentEnabled { .. }) => {
                        false
                    }
                    (Self::Created { .. }, Self::ContractDevelopmentDisabled { .. }) => {
                        false
                    }
                    (Self::Created { .. }, Self::ContractPublished { .. }) => false,
                    (Self::Created { .. }, Self::ContractSetCode { .. }) => false,
                    (Self::Created { .. }, Self::ContractSelfdestructed { .. }) => false,
                    (Self::Created { .. }, Self::__Ignore { .. }) => false,
                    (Self::CreatedFailed { .. }, Self::Created { .. }) => false,
                    (Self::CreatedFailed { .. }, Self::Executed { .. }) => false,
                    (Self::CreatedFailed { .. }, Self::ExecutedFailed { .. }) => false,
                    (Self::CreatedFailed { .. }, Self::TransferredMaintainer { .. }) => {
                        false
                    }
                    (
                        Self::CreatedFailed { .. },
                        Self::ContractDevelopmentEnabled { .. },
                    ) => false,
                    (
                        Self::CreatedFailed { .. },
                        Self::ContractDevelopmentDisabled { .. },
                    ) => false,
                    (Self::CreatedFailed { .. }, Self::ContractPublished { .. }) => false,
                    (Self::CreatedFailed { .. }, Self::ContractSetCode { .. }) => false,
                    (Self::CreatedFailed { .. }, Self::ContractSelfdestructed { .. }) => {
                        false
                    }
                    (Self::CreatedFailed { .. }, Self::__Ignore { .. }) => false,
                    (Self::Executed { .. }, Self::Created { .. }) => false,
                    (Self::Executed { .. }, Self::CreatedFailed { .. }) => false,
                    (Self::Executed { .. }, Self::ExecutedFailed { .. }) => false,
                    (Self::Executed { .. }, Self::TransferredMaintainer { .. }) => false,
                    (Self::Executed { .. }, Self::ContractDevelopmentEnabled { .. }) => {
                        false
                    }
                    (Self::Executed { .. }, Self::ContractDevelopmentDisabled { .. }) => {
                        false
                    }
                    (Self::Executed { .. }, Self::ContractPublished { .. }) => false,
                    (Self::Executed { .. }, Self::ContractSetCode { .. }) => false,
                    (Self::Executed { .. }, Self::ContractSelfdestructed { .. }) => false,
                    (Self::Executed { .. }, Self::__Ignore { .. }) => false,
                    (Self::ExecutedFailed { .. }, Self::Created { .. }) => false,
                    (Self::ExecutedFailed { .. }, Self::CreatedFailed { .. }) => false,
                    (Self::ExecutedFailed { .. }, Self::Executed { .. }) => false,
                    (Self::ExecutedFailed { .. }, Self::TransferredMaintainer { .. }) => {
                        false
                    }
                    (
                        Self::ExecutedFailed { .. },
                        Self::ContractDevelopmentEnabled { .. },
                    ) => false,
                    (
                        Self::ExecutedFailed { .. },
                        Self::ContractDevelopmentDisabled { .. },
                    ) => false,
                    (Self::ExecutedFailed { .. }, Self::ContractPublished { .. }) => {
                        false
                    }
                    (Self::ExecutedFailed { .. }, Self::ContractSetCode { .. }) => false,
                    (
                        Self::ExecutedFailed { .. },
                        Self::ContractSelfdestructed { .. },
                    ) => false,
                    (Self::ExecutedFailed { .. }, Self::__Ignore { .. }) => false,
                    (Self::TransferredMaintainer { .. }, Self::Created { .. }) => false,
                    (Self::TransferredMaintainer { .. }, Self::CreatedFailed { .. }) => {
                        false
                    }
                    (Self::TransferredMaintainer { .. }, Self::Executed { .. }) => false,
                    (Self::TransferredMaintainer { .. }, Self::ExecutedFailed { .. }) => {
                        false
                    }
                    (
                        Self::TransferredMaintainer { .. },
                        Self::ContractDevelopmentEnabled { .. },
                    ) => false,
                    (
                        Self::TransferredMaintainer { .. },
                        Self::ContractDevelopmentDisabled { .. },
                    ) => false,
                    (
                        Self::TransferredMaintainer { .. },
                        Self::ContractPublished { .. },
                    ) => false,
                    (
                        Self::TransferredMaintainer { .. },
                        Self::ContractSetCode { .. },
                    ) => false,
                    (
                        Self::TransferredMaintainer { .. },
                        Self::ContractSelfdestructed { .. },
                    ) => false,
                    (Self::TransferredMaintainer { .. }, Self::__Ignore { .. }) => false,
                    (Self::ContractDevelopmentEnabled { .. }, Self::Created { .. }) => {
                        false
                    }
                    (
                        Self::ContractDevelopmentEnabled { .. },
                        Self::CreatedFailed { .. },
                    ) => false,
                    (Self::ContractDevelopmentEnabled { .. }, Self::Executed { .. }) => {
                        false
                    }
                    (
                        Self::ContractDevelopmentEnabled { .. },
                        Self::ExecutedFailed { .. },
                    ) => false,
                    (
                        Self::ContractDevelopmentEnabled { .. },
                        Self::TransferredMaintainer { .. },
                    ) => false,
                    (
                        Self::ContractDevelopmentEnabled { .. },
                        Self::ContractDevelopmentDisabled { .. },
                    ) => false,
                    (
                        Self::ContractDevelopmentEnabled { .. },
                        Self::ContractPublished { .. },
                    ) => false,
                    (
                        Self::ContractDevelopmentEnabled { .. },
                        Self::ContractSetCode { .. },
                    ) => false,
                    (
                        Self::ContractDevelopmentEnabled { .. },
                        Self::ContractSelfdestructed { .. },
                    ) => false,
                    (Self::ContractDevelopmentEnabled { .. }, Self::__Ignore { .. }) => {
                        false
                    }
                    (Self::ContractDevelopmentDisabled { .. }, Self::Created { .. }) => {
                        false
                    }
                    (
                        Self::ContractDevelopmentDisabled { .. },
                        Self::CreatedFailed { .. },
                    ) => false,
                    (Self::ContractDevelopmentDisabled { .. }, Self::Executed { .. }) => {
                        false
                    }
                    (
                        Self::ContractDevelopmentDisabled { .. },
                        Self::ExecutedFailed { .. },
                    ) => false,
                    (
                        Self::ContractDevelopmentDisabled { .. },
                        Self::TransferredMaintainer { .. },
                    ) => false,
                    (
                        Self::ContractDevelopmentDisabled { .. },
                        Self::ContractDevelopmentEnabled { .. },
                    ) => false,
                    (
                        Self::ContractDevelopmentDisabled { .. },
                        Self::ContractPublished { .. },
                    ) => false,
                    (
                        Self::ContractDevelopmentDisabled { .. },
                        Self::ContractSetCode { .. },
                    ) => false,
                    (
                        Self::ContractDevelopmentDisabled { .. },
                        Self::ContractSelfdestructed { .. },
                    ) => false,
                    (Self::ContractDevelopmentDisabled { .. }, Self::__Ignore { .. }) => {
                        false
                    }
                    (Self::ContractPublished { .. }, Self::Created { .. }) => false,
                    (Self::ContractPublished { .. }, Self::CreatedFailed { .. }) => false,
                    (Self::ContractPublished { .. }, Self::Executed { .. }) => false,
                    (Self::ContractPublished { .. }, Self::ExecutedFailed { .. }) => {
                        false
                    }
                    (
                        Self::ContractPublished { .. },
                        Self::TransferredMaintainer { .. },
                    ) => false,
                    (
                        Self::ContractPublished { .. },
                        Self::ContractDevelopmentEnabled { .. },
                    ) => false,
                    (
                        Self::ContractPublished { .. },
                        Self::ContractDevelopmentDisabled { .. },
                    ) => false,
                    (Self::ContractPublished { .. }, Self::ContractSetCode { .. }) => {
                        false
                    }
                    (
                        Self::ContractPublished { .. },
                        Self::ContractSelfdestructed { .. },
                    ) => false,
                    (Self::ContractPublished { .. }, Self::__Ignore { .. }) => false,
                    (Self::ContractSetCode { .. }, Self::Created { .. }) => false,
                    (Self::ContractSetCode { .. }, Self::CreatedFailed { .. }) => false,
                    (Self::ContractSetCode { .. }, Self::Executed { .. }) => false,
                    (Self::ContractSetCode { .. }, Self::ExecutedFailed { .. }) => false,
                    (
                        Self::ContractSetCode { .. },
                        Self::TransferredMaintainer { .. },
                    ) => false,
                    (
                        Self::ContractSetCode { .. },
                        Self::ContractDevelopmentEnabled { .. },
                    ) => false,
                    (
                        Self::ContractSetCode { .. },
                        Self::ContractDevelopmentDisabled { .. },
                    ) => false,
                    (Self::ContractSetCode { .. }, Self::ContractPublished { .. }) => {
                        false
                    }
                    (
                        Self::ContractSetCode { .. },
                        Self::ContractSelfdestructed { .. },
                    ) => false,
                    (Self::ContractSetCode { .. }, Self::__Ignore { .. }) => false,
                    (Self::ContractSelfdestructed { .. }, Self::Created { .. }) => false,
                    (Self::ContractSelfdestructed { .. }, Self::CreatedFailed { .. }) => {
                        false
                    }
                    (Self::ContractSelfdestructed { .. }, Self::Executed { .. }) => false,
                    (
                        Self::ContractSelfdestructed { .. },
                        Self::ExecutedFailed { .. },
                    ) => false,
                    (
                        Self::ContractSelfdestructed { .. },
                        Self::TransferredMaintainer { .. },
                    ) => false,
                    (
                        Self::ContractSelfdestructed { .. },
                        Self::ContractDevelopmentEnabled { .. },
                    ) => false,
                    (
                        Self::ContractSelfdestructed { .. },
                        Self::ContractDevelopmentDisabled { .. },
                    ) => false,
                    (
                        Self::ContractSelfdestructed { .. },
                        Self::ContractPublished { .. },
                    ) => false,
                    (
                        Self::ContractSelfdestructed { .. },
                        Self::ContractSetCode { .. },
                    ) => false,
                    (Self::ContractSelfdestructed { .. }, Self::__Ignore { .. }) => false,
                    (Self::__Ignore { .. }, Self::Created { .. }) => false,
                    (Self::__Ignore { .. }, Self::CreatedFailed { .. }) => false,
                    (Self::__Ignore { .. }, Self::Executed { .. }) => false,
                    (Self::__Ignore { .. }, Self::ExecutedFailed { .. }) => false,
                    (Self::__Ignore { .. }, Self::TransferredMaintainer { .. }) => false,
                    (Self::__Ignore { .. }, Self::ContractDevelopmentEnabled { .. }) => {
                        false
                    }
                    (Self::__Ignore { .. }, Self::ContractDevelopmentDisabled { .. }) => {
                        false
                    }
                    (Self::__Ignore { .. }, Self::ContractPublished { .. }) => false,
                    (Self::__Ignore { .. }, Self::ContractSetCode { .. }) => false,
                    (Self::__Ignore { .. }, Self::ContractSelfdestructed { .. }) => false,
                }
            }
        }
    };
    const _: () = {
        impl<T: Config> core::fmt::Debug for Event<T> {
            fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
                fmt.write_str("<wasm:stripped>")
            }
        }
    };
    #[allow(deprecated)]
    const _: () = {
        #[automatically_derived]
        impl<T: Config> ::codec::Encode for Event<T>
        where
            T::AccountId: ::codec::Encode,
            T::AccountId: ::codec::Encode,
            T::AccountId: ::codec::Encode,
            T::AccountId: ::codec::Encode,
        {
            fn size_hint(&self) -> usize {
                1_usize
                    + match *self {
                        Event::Created {
                            ref from,
                            ref contract,
                            ref logs,
                            ref used_gas,
                            ref used_storage,
                        } => {
                            0_usize
                                .saturating_add(::codec::Encode::size_hint(from))
                                .saturating_add(::codec::Encode::size_hint(contract))
                                .saturating_add(::codec::Encode::size_hint(logs))
                                .saturating_add(::codec::Encode::size_hint(used_gas))
                                .saturating_add(::codec::Encode::size_hint(used_storage))
                        }
                        Event::CreatedFailed {
                            ref from,
                            ref contract,
                            ref exit_reason,
                            ref logs,
                            ref used_gas,
                            ref used_storage,
                        } => {
                            0_usize
                                .saturating_add(::codec::Encode::size_hint(from))
                                .saturating_add(::codec::Encode::size_hint(contract))
                                .saturating_add(::codec::Encode::size_hint(exit_reason))
                                .saturating_add(::codec::Encode::size_hint(logs))
                                .saturating_add(::codec::Encode::size_hint(used_gas))
                                .saturating_add(::codec::Encode::size_hint(used_storage))
                        }
                        Event::Executed {
                            ref from,
                            ref contract,
                            ref logs,
                            ref used_gas,
                            ref used_storage,
                        } => {
                            0_usize
                                .saturating_add(::codec::Encode::size_hint(from))
                                .saturating_add(::codec::Encode::size_hint(contract))
                                .saturating_add(::codec::Encode::size_hint(logs))
                                .saturating_add(::codec::Encode::size_hint(used_gas))
                                .saturating_add(::codec::Encode::size_hint(used_storage))
                        }
                        Event::ExecutedFailed {
                            ref from,
                            ref contract,
                            ref exit_reason,
                            ref output,
                            ref logs,
                            ref used_gas,
                            ref used_storage,
                        } => {
                            0_usize
                                .saturating_add(::codec::Encode::size_hint(from))
                                .saturating_add(::codec::Encode::size_hint(contract))
                                .saturating_add(::codec::Encode::size_hint(exit_reason))
                                .saturating_add(::codec::Encode::size_hint(output))
                                .saturating_add(::codec::Encode::size_hint(logs))
                                .saturating_add(::codec::Encode::size_hint(used_gas))
                                .saturating_add(::codec::Encode::size_hint(used_storage))
                        }
                        Event::TransferredMaintainer {
                            ref contract,
                            ref new_maintainer,
                        } => {
                            0_usize
                                .saturating_add(::codec::Encode::size_hint(contract))
                                .saturating_add(::codec::Encode::size_hint(new_maintainer))
                        }
                        Event::ContractDevelopmentEnabled { ref who } => {
                            0_usize.saturating_add(::codec::Encode::size_hint(who))
                        }
                        Event::ContractDevelopmentDisabled { ref who } => {
                            0_usize.saturating_add(::codec::Encode::size_hint(who))
                        }
                        Event::ContractPublished { ref contract } => {
                            0_usize.saturating_add(::codec::Encode::size_hint(contract))
                        }
                        Event::ContractSetCode { ref contract } => {
                            0_usize.saturating_add(::codec::Encode::size_hint(contract))
                        }
                        Event::ContractSelfdestructed { ref contract } => {
                            0_usize.saturating_add(::codec::Encode::size_hint(contract))
                        }
                        _ => 0_usize,
                    }
            }
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                match *self {
                    Event::Created {
                        ref from,
                        ref contract,
                        ref logs,
                        ref used_gas,
                        ref used_storage,
                    } => {
                        __codec_dest_edqy.push_byte(0usize as ::core::primitive::u8);
                        ::codec::Encode::encode_to(from, __codec_dest_edqy);
                        ::codec::Encode::encode_to(contract, __codec_dest_edqy);
                        ::codec::Encode::encode_to(logs, __codec_dest_edqy);
                        ::codec::Encode::encode_to(used_gas, __codec_dest_edqy);
                        ::codec::Encode::encode_to(used_storage, __codec_dest_edqy);
                    }
                    Event::CreatedFailed {
                        ref from,
                        ref contract,
                        ref exit_reason,
                        ref logs,
                        ref used_gas,
                        ref used_storage,
                    } => {
                        __codec_dest_edqy.push_byte(1usize as ::core::primitive::u8);
                        ::codec::Encode::encode_to(from, __codec_dest_edqy);
                        ::codec::Encode::encode_to(contract, __codec_dest_edqy);
                        ::codec::Encode::encode_to(exit_reason, __codec_dest_edqy);
                        ::codec::Encode::encode_to(logs, __codec_dest_edqy);
                        ::codec::Encode::encode_to(used_gas, __codec_dest_edqy);
                        ::codec::Encode::encode_to(used_storage, __codec_dest_edqy);
                    }
                    Event::Executed {
                        ref from,
                        ref contract,
                        ref logs,
                        ref used_gas,
                        ref used_storage,
                    } => {
                        __codec_dest_edqy.push_byte(2usize as ::core::primitive::u8);
                        ::codec::Encode::encode_to(from, __codec_dest_edqy);
                        ::codec::Encode::encode_to(contract, __codec_dest_edqy);
                        ::codec::Encode::encode_to(logs, __codec_dest_edqy);
                        ::codec::Encode::encode_to(used_gas, __codec_dest_edqy);
                        ::codec::Encode::encode_to(used_storage, __codec_dest_edqy);
                    }
                    Event::ExecutedFailed {
                        ref from,
                        ref contract,
                        ref exit_reason,
                        ref output,
                        ref logs,
                        ref used_gas,
                        ref used_storage,
                    } => {
                        __codec_dest_edqy.push_byte(3usize as ::core::primitive::u8);
                        ::codec::Encode::encode_to(from, __codec_dest_edqy);
                        ::codec::Encode::encode_to(contract, __codec_dest_edqy);
                        ::codec::Encode::encode_to(exit_reason, __codec_dest_edqy);
                        ::codec::Encode::encode_to(output, __codec_dest_edqy);
                        ::codec::Encode::encode_to(logs, __codec_dest_edqy);
                        ::codec::Encode::encode_to(used_gas, __codec_dest_edqy);
                        ::codec::Encode::encode_to(used_storage, __codec_dest_edqy);
                    }
                    Event::TransferredMaintainer {
                        ref contract,
                        ref new_maintainer,
                    } => {
                        __codec_dest_edqy.push_byte(4usize as ::core::primitive::u8);
                        ::codec::Encode::encode_to(contract, __codec_dest_edqy);
                        ::codec::Encode::encode_to(new_maintainer, __codec_dest_edqy);
                    }
                    Event::ContractDevelopmentEnabled { ref who } => {
                        __codec_dest_edqy.push_byte(5usize as ::core::primitive::u8);
                        ::codec::Encode::encode_to(who, __codec_dest_edqy);
                    }
                    Event::ContractDevelopmentDisabled { ref who } => {
                        __codec_dest_edqy.push_byte(6usize as ::core::primitive::u8);
                        ::codec::Encode::encode_to(who, __codec_dest_edqy);
                    }
                    Event::ContractPublished { ref contract } => {
                        __codec_dest_edqy.push_byte(7usize as ::core::primitive::u8);
                        ::codec::Encode::encode_to(contract, __codec_dest_edqy);
                    }
                    Event::ContractSetCode { ref contract } => {
                        __codec_dest_edqy.push_byte(8usize as ::core::primitive::u8);
                        ::codec::Encode::encode_to(contract, __codec_dest_edqy);
                    }
                    Event::ContractSelfdestructed { ref contract } => {
                        __codec_dest_edqy.push_byte(9usize as ::core::primitive::u8);
                        ::codec::Encode::encode_to(contract, __codec_dest_edqy);
                    }
                    _ => {}
                }
            }
        }
        #[automatically_derived]
        impl<T: Config> ::codec::EncodeLike for Event<T>
        where
            T::AccountId: ::codec::Encode,
            T::AccountId: ::codec::Encode,
            T::AccountId: ::codec::Encode,
            T::AccountId: ::codec::Encode,
        {}
    };
    #[allow(deprecated)]
    const _: () = {
        #[automatically_derived]
        impl<T: Config> ::codec::Decode for Event<T>
        where
            T::AccountId: ::codec::Decode,
            T::AccountId: ::codec::Decode,
            T::AccountId: ::codec::Decode,
            T::AccountId: ::codec::Decode,
        {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                match __codec_input_edqy
                    .read_byte()
                    .map_err(|e| {
                        e.chain("Could not decode `Event`, failed to read variant byte")
                    })?
                {
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 0usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Event::<T>::Created {
                                from: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::Created::from`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                contract: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::Created::contract`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                logs: {
                                    let __codec_res_edqy = <Vec<
                                        Log,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::Created::logs`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                used_gas: {
                                    let __codec_res_edqy = <u64 as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::Created::used_gas`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                used_storage: {
                                    let __codec_res_edqy = <i32 as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::Created::used_storage`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 1usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Event::<T>::CreatedFailed {
                                from: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::CreatedFailed::from`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                contract: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::CreatedFailed::contract`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                exit_reason: {
                                    let __codec_res_edqy = <ExitReason as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Event::CreatedFailed::exit_reason`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                logs: {
                                    let __codec_res_edqy = <Vec<
                                        Log,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::CreatedFailed::logs`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                used_gas: {
                                    let __codec_res_edqy = <u64 as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::CreatedFailed::used_gas`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                used_storage: {
                                    let __codec_res_edqy = <i32 as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Event::CreatedFailed::used_storage`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 2usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Event::<T>::Executed {
                                from: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::Executed::from`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                contract: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::Executed::contract`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                logs: {
                                    let __codec_res_edqy = <Vec<
                                        Log,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::Executed::logs`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                used_gas: {
                                    let __codec_res_edqy = <u64 as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::Executed::used_gas`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                used_storage: {
                                    let __codec_res_edqy = <i32 as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::Executed::used_storage`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 3usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Event::<T>::ExecutedFailed {
                                from: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::ExecutedFailed::from`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                contract: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain("Could not decode `Event::ExecutedFailed::contract`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                exit_reason: {
                                    let __codec_res_edqy = <ExitReason as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Event::ExecutedFailed::exit_reason`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                output: {
                                    let __codec_res_edqy = <Vec<
                                        u8,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::ExecutedFailed::output`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                logs: {
                                    let __codec_res_edqy = <Vec<
                                        Log,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Event::ExecutedFailed::logs`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                used_gas: {
                                    let __codec_res_edqy = <u64 as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain("Could not decode `Event::ExecutedFailed::used_gas`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                used_storage: {
                                    let __codec_res_edqy = <i32 as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Event::ExecutedFailed::used_storage`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 4usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Event::<
                                T,
                            >::TransferredMaintainer {
                                contract: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Event::TransferredMaintainer::contract`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                new_maintainer: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Event::TransferredMaintainer::new_maintainer`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 5usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Event::<
                                T,
                            >::ContractDevelopmentEnabled {
                                who: {
                                    let __codec_res_edqy = <T::AccountId as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Event::ContractDevelopmentEnabled::who`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 6usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Event::<
                                T,
                            >::ContractDevelopmentDisabled {
                                who: {
                                    let __codec_res_edqy = <T::AccountId as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Event::ContractDevelopmentDisabled::who`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 7usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Event::<T>::ContractPublished {
                                contract: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Event::ContractPublished::contract`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 8usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Event::<T>::ContractSetCode {
                                contract: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Event::ContractSetCode::contract`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 9usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Event::<
                                T,
                            >::ContractSelfdestructed {
                                contract: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Event::ContractSelfdestructed::contract`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    _ => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Err(
                                <_ as ::core::convert::Into<
                                    _,
                                >>::into("Could not decode `Event`, variant doesn't exist"),
                            )
                        })();
                    }
                }
            }
        }
    };
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl<T: Config> ::scale_info::TypeInfo for Event<T>
        where
            T::AccountId: ::scale_info::TypeInfo + 'static,
            T::AccountId: ::scale_info::TypeInfo + 'static,
            frame_support::sp_std::marker::PhantomData<
                (T),
            >: ::scale_info::TypeInfo + 'static,
            T: Config + 'static,
        {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                ::scale_info::Type::builder()
                    .path(::scale_info::Path::new("Event", "module_evm::module"))
                    .type_params(
                        <[_]>::into_vec(
                            #[rustc_box]
                            ::alloc::boxed::Box::new([
                                ::scale_info::TypeParameter::new(
                                    "T",
                                    ::core::option::Option::None,
                                ),
                            ]),
                        ),
                    )
                    .docs_always(&["EVM events"])
                    .variant(
                        ::scale_info::build::Variants::new()
                            .variant(
                                "Created",
                                |v| {
                                    v
                                        .index(0usize as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f.ty::<EvmAddress>().name("from").type_name("EvmAddress")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<EvmAddress>()
                                                        .name("contract")
                                                        .type_name("EvmAddress")
                                                })
                                                .field(|f| {
                                                    f.ty::<Vec<Log>>().name("logs").type_name("Vec<Log>")
                                                })
                                                .field(|f| f.ty::<u64>().name("used_gas").type_name("u64"))
                                                .field(|f| {
                                                    f.ty::<i32>().name("used_storage").type_name("i32")
                                                }),
                                        )
                                        .docs_always(&["A contract has been created at given"])
                                },
                            )
                            .variant(
                                "CreatedFailed",
                                |v| {
                                    v
                                        .index(1usize as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f.ty::<EvmAddress>().name("from").type_name("EvmAddress")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<EvmAddress>()
                                                        .name("contract")
                                                        .type_name("EvmAddress")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<ExitReason>()
                                                        .name("exit_reason")
                                                        .type_name("ExitReason")
                                                })
                                                .field(|f| {
                                                    f.ty::<Vec<Log>>().name("logs").type_name("Vec<Log>")
                                                })
                                                .field(|f| f.ty::<u64>().name("used_gas").type_name("u64"))
                                                .field(|f| {
                                                    f.ty::<i32>().name("used_storage").type_name("i32")
                                                }),
                                        )
                                        .docs_always(
                                            &[
                                                "A contract was attempted to be created, but the execution failed.",
                                            ],
                                        )
                                },
                            )
                            .variant(
                                "Executed",
                                |v| {
                                    v
                                        .index(2usize as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f.ty::<EvmAddress>().name("from").type_name("EvmAddress")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<EvmAddress>()
                                                        .name("contract")
                                                        .type_name("EvmAddress")
                                                })
                                                .field(|f| {
                                                    f.ty::<Vec<Log>>().name("logs").type_name("Vec<Log>")
                                                })
                                                .field(|f| f.ty::<u64>().name("used_gas").type_name("u64"))
                                                .field(|f| {
                                                    f.ty::<i32>().name("used_storage").type_name("i32")
                                                }),
                                        )
                                        .docs_always(
                                            &[
                                                "A contract has been executed successfully with states applied.",
                                            ],
                                        )
                                },
                            )
                            .variant(
                                "ExecutedFailed",
                                |v| {
                                    v
                                        .index(3usize as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f.ty::<EvmAddress>().name("from").type_name("EvmAddress")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<EvmAddress>()
                                                        .name("contract")
                                                        .type_name("EvmAddress")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<ExitReason>()
                                                        .name("exit_reason")
                                                        .type_name("ExitReason")
                                                })
                                                .field(|f| {
                                                    f.ty::<Vec<u8>>().name("output").type_name("Vec<u8>")
                                                })
                                                .field(|f| {
                                                    f.ty::<Vec<Log>>().name("logs").type_name("Vec<Log>")
                                                })
                                                .field(|f| f.ty::<u64>().name("used_gas").type_name("u64"))
                                                .field(|f| {
                                                    f.ty::<i32>().name("used_storage").type_name("i32")
                                                }),
                                        )
                                        .docs_always(
                                            &[
                                                "A contract has been executed with errors. States are reverted with",
                                                "only gas fees applied.",
                                            ],
                                        )
                                },
                            )
                            .variant(
                                "TransferredMaintainer",
                                |v| {
                                    v
                                        .index(4usize as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f
                                                        .ty::<EvmAddress>()
                                                        .name("contract")
                                                        .type_name("EvmAddress")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<EvmAddress>()
                                                        .name("new_maintainer")
                                                        .type_name("EvmAddress")
                                                }),
                                        )
                                        .docs_always(&["Transferred maintainer."])
                                },
                            )
                            .variant(
                                "ContractDevelopmentEnabled",
                                |v| {
                                    v
                                        .index(5usize as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f.ty::<T::AccountId>().name("who").type_name("T::AccountId")
                                                }),
                                        )
                                        .docs_always(&["Enabled contract development."])
                                },
                            )
                            .variant(
                                "ContractDevelopmentDisabled",
                                |v| {
                                    v
                                        .index(6usize as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f.ty::<T::AccountId>().name("who").type_name("T::AccountId")
                                                }),
                                        )
                                        .docs_always(&["Disabled contract development."])
                                },
                            )
                            .variant(
                                "ContractPublished",
                                |v| {
                                    v
                                        .index(7usize as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f
                                                        .ty::<EvmAddress>()
                                                        .name("contract")
                                                        .type_name("EvmAddress")
                                                }),
                                        )
                                        .docs_always(&["Published contract."])
                                },
                            )
                            .variant(
                                "ContractSetCode",
                                |v| {
                                    v
                                        .index(8usize as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f
                                                        .ty::<EvmAddress>()
                                                        .name("contract")
                                                        .type_name("EvmAddress")
                                                }),
                                        )
                                        .docs_always(&["Set contract code."])
                                },
                            )
                            .variant(
                                "ContractSelfdestructed",
                                |v| {
                                    v
                                        .index(9usize as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f
                                                        .ty::<EvmAddress>()
                                                        .name("contract")
                                                        .type_name("EvmAddress")
                                                }),
                                        )
                                        .docs_always(&["Selfdestructed contract code."])
                                },
                            ),
                    )
            }
        }
    };
    #[scale_info(skip_type_params(T), capture_docs = "always")]
    ///The `Error` enum of this pallet.
    pub enum Error<T> {
        #[doc(hidden)]
        #[codec(skip)]
        __Ignore(frame_support::sp_std::marker::PhantomData<(T)>, frame_support::Never),
        /// Address not mapped
        AddressNotMapped,
        /// Contract not found
        ContractNotFound,
        /// No permission
        NoPermission,
        /// Contract development is not enabled
        ContractDevelopmentNotEnabled,
        /// Contract development is already enabled
        ContractDevelopmentAlreadyEnabled,
        /// Contract already published
        ContractAlreadyPublished,
        /// Contract exceeds max code size
        ContractExceedsMaxCodeSize,
        /// Contract already existed
        ContractAlreadyExisted,
        /// Storage usage exceeds storage limit
        OutOfStorage,
        /// Charge fee failed
        ChargeFeeFailed,
        /// Contract cannot be killed due to reference count
        CannotKillContract,
        /// Reserve storage failed
        ReserveStorageFailed,
        /// Unreserve storage failed
        UnreserveStorageFailed,
        /// Charge storage failed
        ChargeStorageFailed,
        /// Invalid decimals
        InvalidDecimals,
        /// Strict call failed
        StrictCallFailed,
    }
    #[allow(deprecated)]
    const _: () = {
        #[automatically_derived]
        impl<T> ::codec::Encode for Error<T> {
            fn size_hint(&self) -> usize {
                1_usize
                    + match *self {
                        Error::AddressNotMapped => 0_usize,
                        Error::ContractNotFound => 0_usize,
                        Error::NoPermission => 0_usize,
                        Error::ContractDevelopmentNotEnabled => 0_usize,
                        Error::ContractDevelopmentAlreadyEnabled => 0_usize,
                        Error::ContractAlreadyPublished => 0_usize,
                        Error::ContractExceedsMaxCodeSize => 0_usize,
                        Error::ContractAlreadyExisted => 0_usize,
                        Error::OutOfStorage => 0_usize,
                        Error::ChargeFeeFailed => 0_usize,
                        Error::CannotKillContract => 0_usize,
                        Error::ReserveStorageFailed => 0_usize,
                        Error::UnreserveStorageFailed => 0_usize,
                        Error::ChargeStorageFailed => 0_usize,
                        Error::InvalidDecimals => 0_usize,
                        Error::StrictCallFailed => 0_usize,
                        _ => 0_usize,
                    }
            }
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                match *self {
                    Error::AddressNotMapped => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(0usize as ::core::primitive::u8);
                    }
                    Error::ContractNotFound => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(1usize as ::core::primitive::u8);
                    }
                    Error::NoPermission => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(2usize as ::core::primitive::u8);
                    }
                    Error::ContractDevelopmentNotEnabled => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(3usize as ::core::primitive::u8);
                    }
                    Error::ContractDevelopmentAlreadyEnabled => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(4usize as ::core::primitive::u8);
                    }
                    Error::ContractAlreadyPublished => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(5usize as ::core::primitive::u8);
                    }
                    Error::ContractExceedsMaxCodeSize => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(6usize as ::core::primitive::u8);
                    }
                    Error::ContractAlreadyExisted => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(7usize as ::core::primitive::u8);
                    }
                    Error::OutOfStorage => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(8usize as ::core::primitive::u8);
                    }
                    Error::ChargeFeeFailed => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(9usize as ::core::primitive::u8);
                    }
                    Error::CannotKillContract => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(10usize as ::core::primitive::u8);
                    }
                    Error::ReserveStorageFailed => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(11usize as ::core::primitive::u8);
                    }
                    Error::UnreserveStorageFailed => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(12usize as ::core::primitive::u8);
                    }
                    Error::ChargeStorageFailed => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(13usize as ::core::primitive::u8);
                    }
                    Error::InvalidDecimals => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(14usize as ::core::primitive::u8);
                    }
                    Error::StrictCallFailed => {
                        #[allow(clippy::unnecessary_cast)]
                        __codec_dest_edqy.push_byte(15usize as ::core::primitive::u8);
                    }
                    _ => {}
                }
            }
        }
        #[automatically_derived]
        impl<T> ::codec::EncodeLike for Error<T> {}
    };
    #[allow(deprecated)]
    const _: () = {
        #[automatically_derived]
        impl<T> ::codec::Decode for Error<T> {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                match __codec_input_edqy
                    .read_byte()
                    .map_err(|e| {
                        e.chain("Could not decode `Error`, failed to read variant byte")
                    })?
                {
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 0usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Error::<T>::AddressNotMapped)
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 1usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Error::<T>::ContractNotFound)
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 2usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Error::<T>::NoPermission)
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 3usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(
                                Error::<T>::ContractDevelopmentNotEnabled,
                            )
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 4usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(
                                Error::<T>::ContractDevelopmentAlreadyEnabled,
                            )
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 5usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(
                                Error::<T>::ContractAlreadyPublished,
                            )
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 6usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(
                                Error::<T>::ContractExceedsMaxCodeSize,
                            )
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 7usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(
                                Error::<T>::ContractAlreadyExisted,
                            )
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 8usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Error::<T>::OutOfStorage)
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 9usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Error::<T>::ChargeFeeFailed)
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 10usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Error::<T>::CannotKillContract)
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 11usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Error::<T>::ReserveStorageFailed)
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 12usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(
                                Error::<T>::UnreserveStorageFailed,
                            )
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 13usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Error::<T>::ChargeStorageFailed)
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 14usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Error::<T>::InvalidDecimals)
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 15usize as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Error::<T>::StrictCallFailed)
                        })();
                    }
                    _ => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Err(
                                <_ as ::core::convert::Into<
                                    _,
                                >>::into("Could not decode `Error`, variant doesn't exist"),
                            )
                        })();
                    }
                }
            }
        }
    };
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl<T> ::scale_info::TypeInfo for Error<T>
        where
            frame_support::sp_std::marker::PhantomData<
                (T),
            >: ::scale_info::TypeInfo + 'static,
            T: 'static,
        {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                ::scale_info::Type::builder()
                    .path(::scale_info::Path::new("Error", "module_evm::module"))
                    .type_params(
                        <[_]>::into_vec(
                            #[rustc_box]
                            ::alloc::boxed::Box::new([
                                ::scale_info::TypeParameter::new(
                                    "T",
                                    ::core::option::Option::None,
                                ),
                            ]),
                        ),
                    )
                    .docs_always(&["The `Error` enum of this pallet."])
                    .variant(
                        ::scale_info::build::Variants::new()
                            .variant(
                                "AddressNotMapped",
                                |v| {
                                    v
                                        .index(0usize as ::core::primitive::u8)
                                        .docs_always(&["Address not mapped"])
                                },
                            )
                            .variant(
                                "ContractNotFound",
                                |v| {
                                    v
                                        .index(1usize as ::core::primitive::u8)
                                        .docs_always(&["Contract not found"])
                                },
                            )
                            .variant(
                                "NoPermission",
                                |v| {
                                    v
                                        .index(2usize as ::core::primitive::u8)
                                        .docs_always(&["No permission"])
                                },
                            )
                            .variant(
                                "ContractDevelopmentNotEnabled",
                                |v| {
                                    v
                                        .index(3usize as ::core::primitive::u8)
                                        .docs_always(&["Contract development is not enabled"])
                                },
                            )
                            .variant(
                                "ContractDevelopmentAlreadyEnabled",
                                |v| {
                                    v
                                        .index(4usize as ::core::primitive::u8)
                                        .docs_always(&["Contract development is already enabled"])
                                },
                            )
                            .variant(
                                "ContractAlreadyPublished",
                                |v| {
                                    v
                                        .index(5usize as ::core::primitive::u8)
                                        .docs_always(&["Contract already published"])
                                },
                            )
                            .variant(
                                "ContractExceedsMaxCodeSize",
                                |v| {
                                    v
                                        .index(6usize as ::core::primitive::u8)
                                        .docs_always(&["Contract exceeds max code size"])
                                },
                            )
                            .variant(
                                "ContractAlreadyExisted",
                                |v| {
                                    v
                                        .index(7usize as ::core::primitive::u8)
                                        .docs_always(&["Contract already existed"])
                                },
                            )
                            .variant(
                                "OutOfStorage",
                                |v| {
                                    v
                                        .index(8usize as ::core::primitive::u8)
                                        .docs_always(&["Storage usage exceeds storage limit"])
                                },
                            )
                            .variant(
                                "ChargeFeeFailed",
                                |v| {
                                    v
                                        .index(9usize as ::core::primitive::u8)
                                        .docs_always(&["Charge fee failed"])
                                },
                            )
                            .variant(
                                "CannotKillContract",
                                |v| {
                                    v
                                        .index(10usize as ::core::primitive::u8)
                                        .docs_always(
                                            &["Contract cannot be killed due to reference count"],
                                        )
                                },
                            )
                            .variant(
                                "ReserveStorageFailed",
                                |v| {
                                    v
                                        .index(11usize as ::core::primitive::u8)
                                        .docs_always(&["Reserve storage failed"])
                                },
                            )
                            .variant(
                                "UnreserveStorageFailed",
                                |v| {
                                    v
                                        .index(12usize as ::core::primitive::u8)
                                        .docs_always(&["Unreserve storage failed"])
                                },
                            )
                            .variant(
                                "ChargeStorageFailed",
                                |v| {
                                    v
                                        .index(13usize as ::core::primitive::u8)
                                        .docs_always(&["Charge storage failed"])
                                },
                            )
                            .variant(
                                "InvalidDecimals",
                                |v| {
                                    v
                                        .index(14usize as ::core::primitive::u8)
                                        .docs_always(&["Invalid decimals"])
                                },
                            )
                            .variant(
                                "StrictCallFailed",
                                |v| {
                                    v
                                        .index(15usize as ::core::primitive::u8)
                                        .docs_always(&["Strict call failed"])
                                },
                            ),
                    )
            }
        }
    };
    const _: () = {
        impl<T> frame_support::traits::PalletError for Error<T> {
            const MAX_ENCODED_SIZE: usize = 1;
        }
    };
    /**
				The `Pallet` struct, the main type that implements traits and standalone
				functions within the pallet.
			*/
    pub struct Pallet<T>(frame_support::sp_std::marker::PhantomData<(T)>);
    const _: () = {
        impl<T> core::clone::Clone for Pallet<T> {
            fn clone(&self) -> Self {
                Self(core::clone::Clone::clone(&self.0))
            }
        }
    };
    const _: () = {
        impl<T> core::cmp::Eq for Pallet<T> {}
    };
    const _: () = {
        impl<T> core::cmp::PartialEq for Pallet<T> {
            fn eq(&self, other: &Self) -> bool {
                true && self.0 == other.0
            }
        }
    };
    const _: () = {
        impl<T> core::fmt::Debug for Pallet<T> {
            fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
                fmt.write_str("<wasm:stripped>")
            }
        }
    };
    impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
        fn integrity_test() {
            if !convert_decimals_from_evm(T::StorageDepositPerByte::get()).is_some() {
                ::core::panicking::panic(
                    "assertion failed: convert_decimals_from_evm(T::StorageDepositPerByte::get()).is_some()",
                )
            }
        }
    }
    impl<T: Config> Pallet<T> {
        #[allow(deprecated)]
        #[deprecated(note = "please migrate to `eth_call_v2`")]
        pub fn eth_call(
            origin: OriginFor<T>,
            action: TransactionAction,
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<AccessListItem>,
            _valid_until: T::BlockNumber,
        ) -> DispatchResultWithPostInfo {
            use frame_support::storage::{with_transaction, TransactionOutcome};
            with_transaction(|| {
                let r = (|| {
                    {
                        frame_support::storage::with_storage_layer(|| {
                            match action {
                                TransactionAction::Call(target) => {
                                    Self::call(
                                        origin,
                                        target,
                                        input,
                                        value,
                                        gas_limit,
                                        storage_limit,
                                        access_list,
                                    )
                                }
                                TransactionAction::Create => {
                                    Self::create(
                                        origin,
                                        input,
                                        value,
                                        gas_limit,
                                        storage_limit,
                                        access_list,
                                    )
                                }
                            }
                        })
                    }
                })();
                if r.is_ok() {
                    TransactionOutcome::Commit(r)
                } else {
                    TransactionOutcome::Rollback(r)
                }
            })
        }
        pub fn eth_call_v2(
            origin: OriginFor<T>,
            action: TransactionAction,
            input: Vec<u8>,
            value: BalanceOf<T>,
            _gas_price: u64,
            gas_limit: u64,
            access_list: Vec<AccessListItem>,
        ) -> DispatchResultWithPostInfo {
            use frame_support::storage::{with_transaction, TransactionOutcome};
            with_transaction(|| {
                let r = (|| {
                    {
                        frame_support::storage::with_storage_layer(|| {
                            let (actual_gas_limit, storage_limit) = decode_gas_limit(
                                gas_limit,
                            );
                            match action {
                                TransactionAction::Call(target) => {
                                    Self::call(
                                        origin,
                                        target,
                                        input,
                                        value,
                                        actual_gas_limit,
                                        storage_limit,
                                        access_list,
                                    )
                                }
                                TransactionAction::Create => {
                                    Self::create(
                                        origin,
                                        input,
                                        value,
                                        actual_gas_limit,
                                        storage_limit,
                                        access_list,
                                    )
                                }
                            }
                        })
                    }
                })();
                if r.is_ok() {
                    TransactionOutcome::Commit(r)
                } else {
                    TransactionOutcome::Rollback(r)
                }
            })
        }
        /// Issue an EVM call operation. This is similar to a message call
        /// transaction in Ethereum.
        ///
        /// - `target`: the contract address to call
        /// - `input`: the data supplied for the call
        /// - `value`: the amount sent for payable calls
        /// - `gas_limit`: the maximum gas the call can use
        /// - `storage_limit`: the total bytes the contract's storage can increase by
        pub fn call(
            origin: OriginFor<T>,
            target: EvmAddress,
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<AccessListItem>,
        ) -> DispatchResultWithPostInfo {
            use frame_support::storage::{with_transaction, TransactionOutcome};
            with_transaction(|| {
                let r = (|| {
                    {
                        frame_support::storage::with_storage_layer(|| {
                            let who = ensure_signed(origin)?;
                            let source = T::AddressMapping::get_or_create_evm_address(
                                &who,
                            );
                            let outcome = T::Runner::call(
                                source,
                                source,
                                target,
                                input,
                                value,
                                gas_limit,
                                storage_limit,
                                access_list
                                    .into_iter()
                                    .map(|v| (v.address, v.storage_keys))
                                    .collect(),
                                T::config(),
                            );
                            Self::inc_nonce_if_needed(&source, &outcome);
                            match outcome {
                                Err(e) => {
                                    Pallet::<
                                        T,
                                    >::deposit_event(Event::<T>::ExecutedFailed {
                                        from: source,
                                        contract: target,
                                        exit_reason: ExitReason::Error(
                                            ExitError::Other(Into::<&str>::into(e).into()),
                                        ),
                                        output: ::alloc::vec::Vec::new(),
                                        logs: ::alloc::vec::Vec::new(),
                                        used_gas: gas_limit,
                                        used_storage: Default::default(),
                                    });
                                    Ok(().into())
                                }
                                Ok(info) => {
                                    let used_gas: u64 = info.used_gas.unique_saturated_into();
                                    if info.exit_reason.is_succeed() {
                                        Pallet::<
                                            T,
                                        >::deposit_event(Event::<T>::Executed {
                                            from: source,
                                            contract: target,
                                            logs: info.logs,
                                            used_gas,
                                            used_storage: info.used_storage,
                                        });
                                    } else {
                                        Pallet::<
                                            T,
                                        >::deposit_event(Event::<T>::ExecutedFailed {
                                            from: source,
                                            contract: target,
                                            exit_reason: info.exit_reason.clone(),
                                            output: info.value.clone(),
                                            logs: info.logs,
                                            used_gas,
                                            used_storage: Default::default(),
                                        });
                                    }
                                    Ok(PostDispatchInfo {
                                        actual_weight: Some(call_weight::<T>(used_gas)),
                                        pays_fee: Pays::Yes,
                                    })
                                }
                            }
                        })
                    }
                })();
                if r.is_ok() {
                    TransactionOutcome::Commit(r)
                } else {
                    TransactionOutcome::Rollback(r)
                }
            })
        }
        /// Issue an EVM call operation on a scheduled contract call, and
        /// refund the unused gas reserved when the call was scheduled.
        ///
        /// - `from`: the address the scheduled call originates from
        /// - `target`: the contract address to call
        /// - `input`: the data supplied for the call
        /// - `value`: the amount sent for payable calls
        /// - `gas_limit`: the maximum gas the call can use
        /// - `storage_limit`: the total bytes the contract's storage can increase by
        pub fn scheduled_call(
            origin: OriginFor<T>,
            from: EvmAddress,
            target: EvmAddress,
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<AccessListItem>,
        ) -> DispatchResultWithPostInfo {
            use frame_support::storage::{with_transaction, TransactionOutcome};
            with_transaction(|| {
                let r = (|| {
                    {
                        frame_support::storage::with_storage_layer(|| {
                            ensure_root(origin)?;
                            let _from_account = T::AddressMapping::get_account_id(&from);
                            let _payed: NegativeImbalanceOf<T>;
                            #[cfg(not(feature = "with-ethereum-compatibility"))]
                            {
                                let weight = T::GasToWeight::convert(gas_limit);
                                let (_, imbalance) = T::ChargeTransactionPayment::unreserve_and_charge_fee(
                                        &_from_account,
                                        weight,
                                    )
                                    .map_err(|_| Error::<T>::ChargeFeeFailed)?;
                                _payed = imbalance;
                            }
                            match T::Runner::call(
                                from,
                                from,
                                target,
                                input,
                                value,
                                gas_limit,
                                storage_limit,
                                access_list
                                    .into_iter()
                                    .map(|v| (v.address, v.storage_keys))
                                    .collect(),
                                T::config(),
                            ) {
                                Err(e) => {
                                    Pallet::<
                                        T,
                                    >::deposit_event(Event::<T>::ExecutedFailed {
                                        from,
                                        contract: target,
                                        exit_reason: ExitReason::Error(
                                            ExitError::Other(Into::<&str>::into(e).into()),
                                        ),
                                        output: ::alloc::vec::Vec::new(),
                                        logs: ::alloc::vec::Vec::new(),
                                        used_gas: gas_limit,
                                        used_storage: Default::default(),
                                    });
                                    Ok(().into())
                                }
                                Ok(info) => {
                                    let used_gas: u64 = info.used_gas.unique_saturated_into();
                                    if info.exit_reason.is_succeed() {
                                        Pallet::<
                                            T,
                                        >::deposit_event(Event::<T>::Executed {
                                            from,
                                            contract: target,
                                            logs: info.logs,
                                            used_gas,
                                            used_storage: info.used_storage,
                                        });
                                    } else {
                                        Pallet::<
                                            T,
                                        >::deposit_event(Event::<T>::ExecutedFailed {
                                            from,
                                            contract: target,
                                            exit_reason: info.exit_reason.clone(),
                                            output: info.value.clone(),
                                            logs: info.logs,
                                            used_gas,
                                            used_storage: Default::default(),
                                        });
                                    }
                                    #[cfg(not(feature = "with-ethereum-compatibility"))]
                                    {
                                        use sp_runtime::traits::Zero;
                                        let refund_gas = gas_limit.saturating_sub(used_gas);
                                        if !refund_gas.is_zero() {
                                            let res = T::ChargeTransactionPayment::refund_fee(
                                                &_from_account,
                                                T::GasToWeight::convert(refund_gas),
                                                _payed,
                                            );
                                            if true {
                                                if !res.is_ok() {
                                                    ::core::panicking::panic("assertion failed: res.is_ok()")
                                                }
                                            }
                                        }
                                    }
                                    Ok(PostDispatchInfo {
                                        actual_weight: Some(T::GasToWeight::convert(used_gas)),
                                        pays_fee: Pays::Yes,
                                    })
                                }
                            }
                        })
                    }
                })();
                if r.is_ok() {
                    TransactionOutcome::Commit(r)
                } else {
                    TransactionOutcome::Rollback(r)
                }
            })
        }
        /// Issue an EVM create operation. This is similar to a contract
        /// creation transaction in Ethereum.
        ///
        /// - `input`: the data supplied for the contract's constructor
        /// - `value`: the amount sent to the contract upon creation
        /// - `gas_limit`: the maximum gas the call can use
        /// - `storage_limit`: the total bytes the contract's storage can increase by
        pub fn create(
            origin: OriginFor<T>,
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<AccessListItem>,
        ) -> DispatchResultWithPostInfo {
            use frame_support::storage::{with_transaction, TransactionOutcome};
            with_transaction(|| {
                let r = (|| {
                    {
                        frame_support::storage::with_storage_layer(|| {
                            let who = ensure_signed(origin)?;
                            let source = T::AddressMapping::get_or_create_evm_address(
                                &who,
                            );
                            let outcome = T::Runner::create(
                                source,
                                input,
                                value,
                                gas_limit,
                                storage_limit,
                                access_list
                                    .into_iter()
                                    .map(|v| (v.address, v.storage_keys))
                                    .collect(),
                                T::config(),
                            );
                            Self::inc_nonce_if_needed(&source, &outcome);
                            match outcome {
                                Err(e) => {
                                    Pallet::<
                                        T,
                                    >::deposit_event(Event::<T>::CreatedFailed {
                                        from: source,
                                        contract: H160::default(),
                                        exit_reason: ExitReason::Error(
                                            ExitError::Other(Into::<&str>::into(e).into()),
                                        ),
                                        logs: ::alloc::vec::Vec::new(),
                                        used_gas: gas_limit,
                                        used_storage: Default::default(),
                                    });
                                    Ok(().into())
                                }
                                Ok(info) => {
                                    let used_gas: u64 = info.used_gas.unique_saturated_into();
                                    if info.exit_reason.is_succeed() {
                                        Pallet::<
                                            T,
                                        >::deposit_event(Event::<T>::Created {
                                            from: source,
                                            contract: info.value,
                                            logs: info.logs,
                                            used_gas,
                                            used_storage: info.used_storage,
                                        });
                                    } else {
                                        Pallet::<
                                            T,
                                        >::deposit_event(Event::<T>::CreatedFailed {
                                            from: source,
                                            contract: info.value,
                                            exit_reason: info.exit_reason.clone(),
                                            logs: info.logs,
                                            used_gas,
                                            used_storage: Default::default(),
                                        });
                                    }
                                    Ok(PostDispatchInfo {
                                        actual_weight: Some(create_weight::<T>(used_gas)),
                                        pays_fee: Pays::Yes,
                                    })
                                }
                            }
                        })
                    }
                })();
                if r.is_ok() {
                    TransactionOutcome::Commit(r)
                } else {
                    TransactionOutcome::Rollback(r)
                }
            })
        }
        /// Issue an EVM create2 operation.
        ///
        /// - `target`: the contract address to call
        /// - `input`: the data supplied for the contract's constructor
        /// - `salt`: used for generating the new contract's address
        /// - `value`: the amount sent for payable calls
        /// - `gas_limit`: the maximum gas the call can use
        /// - `storage_limit`: the total bytes the contract's storage can increase by
        pub fn create2(
            origin: OriginFor<T>,
            input: Vec<u8>,
            salt: H256,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<AccessListItem>,
        ) -> DispatchResultWithPostInfo {
            use frame_support::storage::{with_transaction, TransactionOutcome};
            with_transaction(|| {
                let r = (|| {
                    {
                        frame_support::storage::with_storage_layer(|| {
                            let who = ensure_signed(origin)?;
                            let source = T::AddressMapping::get_or_create_evm_address(
                                &who,
                            );
                            let outcome = T::Runner::create2(
                                source,
                                input,
                                salt,
                                value,
                                gas_limit,
                                storage_limit,
                                access_list
                                    .into_iter()
                                    .map(|v| (v.address, v.storage_keys))
                                    .collect(),
                                T::config(),
                            );
                            Self::inc_nonce_if_needed(&source, &outcome);
                            match outcome {
                                Err(e) => {
                                    Pallet::<
                                        T,
                                    >::deposit_event(Event::<T>::CreatedFailed {
                                        from: source,
                                        contract: H160::default(),
                                        exit_reason: ExitReason::Error(
                                            ExitError::Other(Into::<&str>::into(e).into()),
                                        ),
                                        logs: ::alloc::vec::Vec::new(),
                                        used_gas: gas_limit,
                                        used_storage: Default::default(),
                                    });
                                    Ok(().into())
                                }
                                Ok(info) => {
                                    let used_gas: u64 = info.used_gas.unique_saturated_into();
                                    if info.exit_reason.is_succeed() {
                                        Pallet::<
                                            T,
                                        >::deposit_event(Event::<T>::Created {
                                            from: source,
                                            contract: info.value,
                                            logs: info.logs,
                                            used_gas,
                                            used_storage: info.used_storage,
                                        });
                                    } else {
                                        Pallet::<
                                            T,
                                        >::deposit_event(Event::<T>::CreatedFailed {
                                            from: source,
                                            contract: info.value,
                                            exit_reason: info.exit_reason.clone(),
                                            logs: info.logs,
                                            used_gas,
                                            used_storage: Default::default(),
                                        });
                                    }
                                    Ok(PostDispatchInfo {
                                        actual_weight: Some(create2_weight::<T>(used_gas)),
                                        pays_fee: Pays::Yes,
                                    })
                                }
                            }
                        })
                    }
                })();
                if r.is_ok() {
                    TransactionOutcome::Commit(r)
                } else {
                    TransactionOutcome::Rollback(r)
                }
            })
        }
        /// Create mirrored NFT contract. The next available system contract
        /// address will be used as created contract address.
        ///
        /// - `input`: the data supplied for the contract's constructor
        /// - `value`: the amount sent for payable calls
        /// - `gas_limit`: the maximum gas the call can use
        /// - `storage_limit`: the total bytes the contract's storage can increase by
        pub fn create_nft_contract(
            origin: OriginFor<T>,
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<AccessListItem>,
        ) -> DispatchResultWithPostInfo {
            use frame_support::storage::{with_transaction, TransactionOutcome};
            with_transaction(|| {
                let r = (|| {
                    {
                        frame_support::storage::with_storage_layer(|| {
                            T::NetworkContractOrigin::ensure_origin(origin)?;
                            let source = T::NetworkContractSource::get();
                            let source_account = T::AddressMapping::get_account_id(
                                &source,
                            );
                            let address = MIRRORED_TOKENS_ADDRESS_START
                                | EvmAddress::from_low_u64_be(
                                    Self::network_contract_index(),
                                );
                            let amount = T::Currency::minimum_balance()
                                .saturating_mul(100u32.into());
                            if T::Currency::free_balance(&source_account) < amount {
                                T::Currency::transfer(
                                    &T::TreasuryAccount::get(),
                                    &source_account,
                                    amount,
                                    ExistenceRequirement::AllowDeath,
                                )?;
                            }
                            match T::Runner::create_at_address(
                                source,
                                address,
                                input,
                                value,
                                gas_limit,
                                storage_limit,
                                access_list
                                    .into_iter()
                                    .map(|v| (v.address, v.storage_keys))
                                    .collect(),
                                T::config(),
                            ) {
                                Err(e) => {
                                    Pallet::<
                                        T,
                                    >::deposit_event(Event::<T>::CreatedFailed {
                                        from: source,
                                        contract: H160::default(),
                                        exit_reason: ExitReason::Error(
                                            ExitError::Other(Into::<&str>::into(e).into()),
                                        ),
                                        logs: ::alloc::vec::Vec::new(),
                                        used_gas: gas_limit,
                                        used_storage: Default::default(),
                                    });
                                    Ok(().into())
                                }
                                Ok(info) => {
                                    let used_gas: u64 = info.used_gas.unique_saturated_into();
                                    if info.exit_reason.is_succeed() {
                                        NetworkContractIndex::<
                                            T,
                                        >::mutate(|v| *v = v.saturating_add(One::one()));
                                        Pallet::<
                                            T,
                                        >::deposit_event(Event::<T>::Created {
                                            from: source,
                                            contract: info.value,
                                            logs: info.logs,
                                            used_gas,
                                            used_storage: info.used_storage,
                                        });
                                    } else {
                                        Pallet::<
                                            T,
                                        >::deposit_event(Event::<T>::CreatedFailed {
                                            from: source,
                                            contract: info.value,
                                            exit_reason: info.exit_reason.clone(),
                                            logs: info.logs,
                                            used_gas,
                                            used_storage: Default::default(),
                                        });
                                    }
                                    Ok(PostDispatchInfo {
                                        actual_weight: Some(create_nft_contract::<T>(used_gas)),
                                        pays_fee: Pays::No,
                                    })
                                }
                            }
                        })
                    }
                })();
                if r.is_ok() {
                    TransactionOutcome::Commit(r)
                } else {
                    TransactionOutcome::Rollback(r)
                }
            })
        }
        /// Issue an EVM create operation. The address specified
        /// will be used as created contract address.
        ///
        /// - `target`: the address specified by the contract
        /// - `input`: the data supplied for the contract's constructor
        /// - `value`: the amount sent for payable calls
        /// - `gas_limit`: the maximum gas the call can use
        /// - `storage_limit`: the total bytes the contract's storage can increase by
        pub fn create_predeploy_contract(
            origin: OriginFor<T>,
            target: EvmAddress,
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<AccessListItem>,
        ) -> DispatchResultWithPostInfo {
            use frame_support::storage::{with_transaction, TransactionOutcome};
            with_transaction(|| {
                let r = (|| {
                    {
                        frame_support::storage::with_storage_layer(|| {
                            T::NetworkContractOrigin::ensure_origin(origin)?;
                            {
                                if !Self::accounts(target).is_none() {
                                    { return Err(Error::<T>::ContractAlreadyExisted.into()) };
                                }
                            };
                            let source = T::NetworkContractSource::get();
                            let source_account = T::AddressMapping::get_account_id(
                                &source,
                            );
                            let amount = T::Currency::minimum_balance()
                                .saturating_mul(100u32.into());
                            if T::Currency::free_balance(&source_account) < amount {
                                T::Currency::transfer(
                                    &T::TreasuryAccount::get(),
                                    &source_account,
                                    amount,
                                    ExistenceRequirement::AllowDeath,
                                )?;
                            }
                            match T::Runner::create_at_address(
                                source,
                                target,
                                input,
                                value,
                                gas_limit,
                                storage_limit,
                                access_list
                                    .into_iter()
                                    .map(|v| (v.address, v.storage_keys))
                                    .collect(),
                                T::config(),
                            ) {
                                Err(e) => {
                                    Pallet::<
                                        T,
                                    >::deposit_event(Event::<T>::CreatedFailed {
                                        from: source,
                                        contract: H160::default(),
                                        exit_reason: ExitReason::Error(
                                            ExitError::Other(Into::<&str>::into(e).into()),
                                        ),
                                        logs: ::alloc::vec::Vec::new(),
                                        used_gas: gas_limit,
                                        used_storage: Default::default(),
                                    });
                                    Ok(().into())
                                }
                                Ok(info) => {
                                    let used_gas: u64 = info.used_gas.unique_saturated_into();
                                    let contract = info.value;
                                    if info.exit_reason.is_succeed() {
                                        Pallet::<
                                            T,
                                        >::deposit_event(Event::<T>::Created {
                                            from: source,
                                            contract,
                                            logs: info.logs,
                                            used_gas,
                                            used_storage: info.used_storage,
                                        });
                                    } else {
                                        Pallet::<
                                            T,
                                        >::deposit_event(Event::<T>::CreatedFailed {
                                            from: source,
                                            contract,
                                            exit_reason: info.exit_reason.clone(),
                                            logs: info.logs,
                                            used_gas,
                                            used_storage: Default::default(),
                                        });
                                    }
                                    if info.exit_reason.is_succeed() {
                                        Self::mark_published(contract, Some(source))?;
                                        Pallet::<
                                            T,
                                        >::deposit_event(Event::<T>::ContractPublished {
                                            contract,
                                        });
                                    }
                                    Ok(PostDispatchInfo {
                                        actual_weight: Some(
                                            create_predeploy_contract::<T>(used_gas),
                                        ),
                                        pays_fee: Pays::No,
                                    })
                                }
                            }
                        })
                    }
                })();
                if r.is_ok() {
                    TransactionOutcome::Commit(r)
                } else {
                    TransactionOutcome::Rollback(r)
                }
            })
        }
        /// Transfers Contract maintainership to a new EVM Address.
        ///
        /// - `contract`: the contract whose maintainership is being transferred, the caller must be
        ///   the contract's maintainer
        /// - `new_maintainer`: the address of the new maintainer
        pub fn transfer_maintainer(
            origin: OriginFor<T>,
            contract: EvmAddress,
            new_maintainer: EvmAddress,
        ) -> DispatchResultWithPostInfo {
            use frame_support::storage::{with_transaction, TransactionOutcome};
            with_transaction(|| {
                let r = (|| {
                    {
                        frame_support::storage::with_storage_layer(|| {
                            let who = ensure_signed(origin)?;
                            Self::do_transfer_maintainer(who, contract, new_maintainer)?;
                            Pallet::<
                                T,
                            >::deposit_event(Event::<T>::TransferredMaintainer {
                                contract,
                                new_maintainer,
                            });
                            Ok(().into())
                        })
                    }
                })();
                if r.is_ok() {
                    TransactionOutcome::Commit(r)
                } else {
                    TransactionOutcome::Rollback(r)
                }
            })
        }
        /// Mark a given contract as published.
        ///
        /// - `contract`: The contract to mark as published, the caller must the contract's
        ///   maintainer
        pub fn publish_contract(
            origin: OriginFor<T>,
            contract: EvmAddress,
        ) -> DispatchResultWithPostInfo {
            use frame_support::storage::{with_transaction, TransactionOutcome};
            with_transaction(|| {
                let r = (|| {
                    {
                        frame_support::storage::with_storage_layer(|| {
                            let who = ensure_signed(origin)?;
                            Self::do_publish_contract(who, contract)?;
                            Pallet::<
                                T,
                            >::deposit_event(Event::<T>::ContractPublished {
                                contract,
                            });
                            Ok(().into())
                        })
                    }
                })();
                if r.is_ok() {
                    TransactionOutcome::Commit(r)
                } else {
                    TransactionOutcome::Rollback(r)
                }
            })
        }
        /// Mark a given contract as published without paying the publication fee
        ///
        /// - `contract`: The contract to mark as published, the caller must be the contract's
        ///   maintainer.
        pub fn publish_free(
            origin: OriginFor<T>,
            contract: EvmAddress,
        ) -> DispatchResultWithPostInfo {
            use frame_support::storage::{with_transaction, TransactionOutcome};
            with_transaction(|| {
                let r = (|| {
                    {
                        frame_support::storage::with_storage_layer(|| {
                            T::FreePublicationOrigin::ensure_origin(origin)?;
                            Self::mark_published(contract, None)?;
                            Pallet::<
                                T,
                            >::deposit_event(Event::<T>::ContractPublished {
                                contract,
                            });
                            Ok(().into())
                        })
                    }
                })();
                if r.is_ok() {
                    TransactionOutcome::Commit(r)
                } else {
                    TransactionOutcome::Rollback(r)
                }
            })
        }
        /// Mark the caller's address to allow contract development.
        /// This allows the address to interact with non-published contracts.
        pub fn enable_contract_development(
            origin: OriginFor<T>,
        ) -> DispatchResultWithPostInfo {
            use frame_support::storage::{with_transaction, TransactionOutcome};
            with_transaction(|| {
                let r = (|| {
                    {
                        frame_support::storage::with_storage_layer(|| {
                            let who = ensure_signed(origin)?;
                            Self::do_enable_contract_development(&who)?;
                            Pallet::<
                                T,
                            >::deposit_event(Event::<T>::ContractDevelopmentEnabled {
                                who,
                            });
                            Ok(().into())
                        })
                    }
                })();
                if r.is_ok() {
                    TransactionOutcome::Commit(r)
                } else {
                    TransactionOutcome::Rollback(r)
                }
            })
        }
        /// Mark the caller's address to disable contract development.
        /// This disallows the address to interact with non-published contracts.
        pub fn disable_contract_development(
            origin: OriginFor<T>,
        ) -> DispatchResultWithPostInfo {
            use frame_support::storage::{with_transaction, TransactionOutcome};
            with_transaction(|| {
                let r = (|| {
                    {
                        frame_support::storage::with_storage_layer(|| {
                            let who = ensure_signed(origin)?;
                            Self::do_disable_contract_development(&who)?;
                            Pallet::<
                                T,
                            >::deposit_event(Event::<T>::ContractDevelopmentDisabled {
                                who,
                            });
                            Ok(().into())
                        })
                    }
                })();
                if r.is_ok() {
                    TransactionOutcome::Commit(r)
                } else {
                    TransactionOutcome::Rollback(r)
                }
            })
        }
        /// Set the code of a contract at a given address.
        ///
        /// - `contract`: The contract whose code is being set, must not be marked as published
        /// - `code`: The new ABI bundle for the contract
        pub fn set_code(
            origin: OriginFor<T>,
            contract: EvmAddress,
            code: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            use frame_support::storage::{with_transaction, TransactionOutcome};
            with_transaction(|| {
                let r = (|| {
                    {
                        frame_support::storage::with_storage_layer(|| {
                            let root_or_signed = Self::ensure_root_or_signed(origin)?;
                            Self::do_set_code(root_or_signed, contract, code)?;
                            Pallet::<
                                T,
                            >::deposit_event(Event::<T>::ContractSetCode {
                                contract,
                            });
                            Ok(().into())
                        })
                    }
                })();
                if r.is_ok() {
                    TransactionOutcome::Commit(r)
                } else {
                    TransactionOutcome::Rollback(r)
                }
            })
        }
        /// Remove a contract at a given address.
        ///
        /// - `contract`: The contract to remove, must not be marked as published
        pub fn selfdestruct(
            origin: OriginFor<T>,
            contract: EvmAddress,
        ) -> DispatchResultWithPostInfo {
            use frame_support::storage::{with_transaction, TransactionOutcome};
            with_transaction(|| {
                let r = (|| {
                    {
                        frame_support::storage::with_storage_layer(|| {
                            let who = ensure_signed(origin)?;
                            let caller = T::AddressMapping::get_evm_address(&who)
                                .ok_or(Error::<T>::AddressNotMapped)?;
                            Self::do_selfdestruct(&caller, &contract)?;
                            Pallet::<
                                T,
                            >::deposit_event(Event::<T>::ContractSelfdestructed {
                                contract,
                            });
                            Ok(().into())
                        })
                    }
                })();
                if r.is_ok() {
                    TransactionOutcome::Commit(r)
                } else {
                    TransactionOutcome::Rollback(r)
                }
            })
        }
        /// Issue an EVM call operation in `Utility::batch_all`. This is same as the evm.call but
        /// returns error when it failed. The current evm.call always success and emit event to
        /// indicate it failed.
        ///
        /// - `target`: the contract address to call
        /// - `input`: the data supplied for the call
        /// - `value`: the amount sent for payable calls
        /// - `gas_limit`: the maximum gas the call can use
        /// - `storage_limit`: the total bytes the contract's storage can increase by
        pub fn strict_call(
            origin: OriginFor<T>,
            target: EvmAddress,
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<AccessListItem>,
        ) -> DispatchResultWithPostInfo {
            use frame_support::storage::{with_transaction, TransactionOutcome};
            with_transaction(|| {
                let r = (|| {
                    {
                        frame_support::storage::with_storage_layer(|| {
                            let who = ensure_signed(origin)?;
                            let source = T::AddressMapping::get_or_create_evm_address(
                                &who,
                            );
                            match T::Runner::call(
                                source,
                                source,
                                target,
                                input,
                                value,
                                gas_limit,
                                storage_limit,
                                access_list
                                    .into_iter()
                                    .map(|v| (v.address, v.storage_keys))
                                    .collect(),
                                T::config(),
                            ) {
                                Err(e) => {
                                    Err(DispatchErrorWithPostInfo {
                                        post_info: ().into(),
                                        error: e,
                                    })
                                }
                                Ok(info) => {
                                    let used_gas: u64 = info.used_gas.unique_saturated_into();
                                    if info.exit_reason.is_succeed() {
                                        Pallet::<
                                            T,
                                        >::deposit_event(Event::<T>::Executed {
                                            from: source,
                                            contract: target,
                                            logs: info.logs,
                                            used_gas,
                                            used_storage: info.used_storage,
                                        });
                                        Ok(PostDispatchInfo {
                                            actual_weight: Some(call_weight::<T>(used_gas)),
                                            pays_fee: Pays::Yes,
                                        })
                                    } else {
                                        {
                                            let lvl = ::log::Level::Debug;
                                            if lvl <= ::log::STATIC_MAX_LEVEL
                                                && lvl <= ::log::max_level()
                                            {
                                                ::log::__private_api_log(
                                                    format_args!(
                                                        "batch_call failed: [from: {0:?}, contract: {1:?}, exit_reason: {2:?}, output: {3:?}, logs: {4:?}, used_gas: {5:?}]",
                                                        source, target, info.exit_reason, info.value, info.logs,
                                                        used_gas
                                                    ),
                                                    lvl,
                                                    &(
                                                        "evm",
                                                        "module_evm::module",
                                                        "modules/evm/src/lib.rs",
                                                        1300u32,
                                                    ),
                                                    ::log::__private_api::Option::None,
                                                );
                                            }
                                        };
                                        Err(DispatchErrorWithPostInfo {
                                            post_info: PostDispatchInfo {
                                                actual_weight: Some(call_weight::<T>(used_gas)),
                                                pays_fee: Pays::Yes,
                                            },
                                            error: Error::<T>::StrictCallFailed.into(),
                                        })
                                    }
                                }
                            }
                        })
                    }
                })();
                if r.is_ok() {
                    TransactionOutcome::Commit(r)
                } else {
                    TransactionOutcome::Rollback(r)
                }
            })
        }
    }
    impl<T: Config> Pallet<T> {
        #[doc(hidden)]
        pub fn pallet_documentation_metadata() -> frame_support::sp_std::vec::Vec<
            &'static str,
        > {
            ::alloc::vec::Vec::new()
        }
    }
    impl<T: Config> Pallet<T> {
        #[doc(hidden)]
        pub fn pallet_constants_metadata() -> frame_support::sp_std::vec::Vec<
            frame_support::metadata_ir::PalletConstantMetadataIR,
        > {
            <[_]>::into_vec(
                #[rustc_box]
                ::alloc::boxed::Box::new([
                    {
                        frame_support::metadata_ir::PalletConstantMetadataIR {
                            name: "NewContractExtraBytes",
                            ty: frame_support::scale_info::meta_type::<u32>(),
                            value: {
                                let value = <<T as Config>::NewContractExtraBytes as frame_support::traits::Get<
                                    u32,
                                >>::get();
                                frame_support::codec::Encode::encode(&value)
                            },
                            docs: <[_]>::into_vec(
                                #[rustc_box]
                                ::alloc::boxed::Box::new([
                                    " Charge extra bytes for creating a contract, would be reserved until",
                                    " the contract deleted.",
                                ]),
                            ),
                        }
                    },
                    {
                        frame_support::metadata_ir::PalletConstantMetadataIR {
                            name: "StorageDepositPerByte",
                            ty: frame_support::scale_info::meta_type::<BalanceOf<T>>(),
                            value: {
                                let value = <<T as Config>::StorageDepositPerByte as frame_support::traits::Get<
                                    BalanceOf<T>,
                                >>::get();
                                frame_support::codec::Encode::encode(&value)
                            },
                            docs: <[_]>::into_vec(
                                #[rustc_box]
                                ::alloc::boxed::Box::new([
                                    " Storage required for per byte.",
                                ]),
                            ),
                        }
                    },
                    {
                        frame_support::metadata_ir::PalletConstantMetadataIR {
                            name: "TxFeePerGas",
                            ty: frame_support::scale_info::meta_type::<BalanceOf<T>>(),
                            value: {
                                let value = <<T as Config>::TxFeePerGas as frame_support::traits::Get<
                                    BalanceOf<T>,
                                >>::get();
                                frame_support::codec::Encode::encode(&value)
                            },
                            docs: <[_]>::into_vec(
                                #[rustc_box]
                                ::alloc::boxed::Box::new([
                                    " Tx fee required for per gas.",
                                    " Provide to the client",
                                ]),
                            ),
                        }
                    },
                    {
                        frame_support::metadata_ir::PalletConstantMetadataIR {
                            name: "NetworkContractSource",
                            ty: frame_support::scale_info::meta_type::<EvmAddress>(),
                            value: {
                                let value = <<T as Config>::NetworkContractSource as frame_support::traits::Get<
                                    EvmAddress,
                                >>::get();
                                frame_support::codec::Encode::encode(&value)
                            },
                            docs: <[_]>::into_vec(
                                #[rustc_box]
                                ::alloc::boxed::Box::new([
                                    " The EVM address for creating system contract.",
                                ]),
                            ),
                        }
                    },
                    {
                        frame_support::metadata_ir::PalletConstantMetadataIR {
                            name: "DeveloperDeposit",
                            ty: frame_support::scale_info::meta_type::<BalanceOf<T>>(),
                            value: {
                                let value = <<T as Config>::DeveloperDeposit as frame_support::traits::Get<
                                    BalanceOf<T>,
                                >>::get();
                                frame_support::codec::Encode::encode(&value)
                            },
                            docs: <[_]>::into_vec(
                                #[rustc_box]
                                ::alloc::boxed::Box::new([" Deposit for the developer."]),
                            ),
                        }
                    },
                    {
                        frame_support::metadata_ir::PalletConstantMetadataIR {
                            name: "PublicationFee",
                            ty: frame_support::scale_info::meta_type::<BalanceOf<T>>(),
                            value: {
                                let value = <<T as Config>::PublicationFee as frame_support::traits::Get<
                                    BalanceOf<T>,
                                >>::get();
                                frame_support::codec::Encode::encode(&value)
                            },
                            docs: <[_]>::into_vec(
                                #[rustc_box]
                                ::alloc::boxed::Box::new([
                                    " The fee for publishing the contract.",
                                ]),
                            ),
                        }
                    },
                    {
                        frame_support::metadata_ir::PalletConstantMetadataIR {
                            name: "TreasuryAccount",
                            ty: frame_support::scale_info::meta_type::<T::AccountId>(),
                            value: {
                                let value = <<T as Config>::TreasuryAccount as frame_support::traits::Get<
                                    T::AccountId,
                                >>::get();
                                frame_support::codec::Encode::encode(&value)
                            },
                            docs: ::alloc::vec::Vec::new(),
                        }
                    },
                ]),
            )
        }
    }
    impl<T: Config> Pallet<T> {
        #[doc(hidden)]
        pub fn error_metadata() -> Option<
            frame_support::metadata_ir::PalletErrorMetadataIR,
        > {
            Some(frame_support::metadata_ir::PalletErrorMetadataIR {
                ty: frame_support::scale_info::meta_type::<Error<T>>(),
            })
        }
    }
    /// Type alias to `Pallet`, to be used by `construct_runtime`.
    ///
    /// Generated by `pallet` attribute macro.
    #[deprecated(note = "use `Pallet` instead")]
    #[allow(dead_code)]
    pub type Module<T> = Pallet<T>;
    impl<T: Config> frame_support::traits::GetStorageVersion for Pallet<T> {
        type CurrentStorageVersion = frame_support::traits::NoStorageVersionSet;
        fn current_storage_version() -> Self::CurrentStorageVersion {
            core::default::Default::default()
        }
        fn on_chain_storage_version() -> frame_support::traits::StorageVersion {
            frame_support::traits::StorageVersion::get::<Self>()
        }
    }
    impl<T: Config> frame_support::traits::OnGenesis for Pallet<T> {
        fn on_genesis() {
            let storage_version: frame_support::traits::StorageVersion = core::default::Default::default();
            storage_version.put::<Self>();
        }
    }
    impl<T: Config> frame_support::traits::PalletInfoAccess for Pallet<T> {
        fn index() -> usize {
            <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::index::<
                Self,
            >()
                .expect(
                    "Pallet is part of the runtime because pallet `Config` trait is \
						implemented by the runtime",
                )
        }
        fn name() -> &'static str {
            <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::name::<
                Self,
            >()
                .expect(
                    "Pallet is part of the runtime because pallet `Config` trait is \
						implemented by the runtime",
                )
        }
        fn module_name() -> &'static str {
            <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::module_name::<
                Self,
            >()
                .expect(
                    "Pallet is part of the runtime because pallet `Config` trait is \
						implemented by the runtime",
                )
        }
        fn crate_version() -> frame_support::traits::CrateVersion {
            frame_support::traits::CrateVersion {
                major: 2u16,
                minor: 19u8,
                patch: 0u8,
            }
        }
    }
    impl<T: Config> frame_support::traits::PalletsInfoAccess for Pallet<T> {
        fn count() -> usize {
            1
        }
        fn infos() -> frame_support::sp_std::vec::Vec<
            frame_support::traits::PalletInfoData,
        > {
            use frame_support::traits::PalletInfoAccess;
            let item = frame_support::traits::PalletInfoData {
                index: Self::index(),
                name: Self::name(),
                module_name: Self::module_name(),
                crate_version: Self::crate_version(),
            };
            <[_]>::into_vec(#[rustc_box] ::alloc::boxed::Box::new([item]))
        }
    }
    impl<T: Config> frame_support::traits::StorageInfoTrait for Pallet<T> {
        fn storage_info() -> frame_support::sp_std::vec::Vec<
            frame_support::traits::StorageInfo,
        > {
            #[allow(unused_mut)]
            let mut res = ::alloc::vec::Vec::new();
            {
                let mut storage_info = <ChainId<
                    T,
                > as frame_support::traits::PartialStorageInfoTrait>::partial_storage_info();
                res.append(&mut storage_info);
            }
            {
                let mut storage_info = <Accounts<
                    T,
                > as frame_support::traits::PartialStorageInfoTrait>::partial_storage_info();
                res.append(&mut storage_info);
            }
            {
                let mut storage_info = <ContractStorageSizes<
                    T,
                > as frame_support::traits::PartialStorageInfoTrait>::partial_storage_info();
                res.append(&mut storage_info);
            }
            {
                let mut storage_info = <AccountStorages<
                    T,
                > as frame_support::traits::PartialStorageInfoTrait>::partial_storage_info();
                res.append(&mut storage_info);
            }
            {
                let mut storage_info = <Codes<
                    T,
                > as frame_support::traits::PartialStorageInfoTrait>::partial_storage_info();
                res.append(&mut storage_info);
            }
            {
                let mut storage_info = <CodeInfos<
                    T,
                > as frame_support::traits::PartialStorageInfoTrait>::partial_storage_info();
                res.append(&mut storage_info);
            }
            {
                let mut storage_info = <NetworkContractIndex<
                    T,
                > as frame_support::traits::PartialStorageInfoTrait>::partial_storage_info();
                res.append(&mut storage_info);
            }
            {
                let mut storage_info = <ExtrinsicOrigin<
                    T,
                > as frame_support::traits::PartialStorageInfoTrait>::partial_storage_info();
                res.append(&mut storage_info);
            }
            {
                let mut storage_info = <XcmOrigin<
                    T,
                > as frame_support::traits::PartialStorageInfoTrait>::partial_storage_info();
                res.append(&mut storage_info);
            }
            res
        }
    }
    use frame_support::traits::{
        StorageInfoTrait, TrackedStorageKey, WhitelistedStorageKeys,
    };
    impl<T: Config> WhitelistedStorageKeys for Pallet<T> {
        fn whitelisted_storage_keys() -> frame_support::sp_std::vec::Vec<
            TrackedStorageKey,
        > {
            use frame_support::sp_std::vec;
            ::alloc::vec::Vec::new()
        }
    }
    mod warnings {}
    #[doc(hidden)]
    pub mod __substrate_call_check {
        #[doc(hidden)]
        pub use __is_call_part_defined_0 as is_call_part_defined;
    }
    ///Contains a variant per dispatchable extrinsic that this pallet has.
    #[codec(encode_bound())]
    #[codec(decode_bound())]
    #[scale_info(skip_type_params(T), capture_docs = "always")]
    #[allow(non_camel_case_types)]
    pub enum Call<T: Config> {
        #[doc(hidden)]
        #[codec(skip)]
        __Ignore(frame_support::sp_std::marker::PhantomData<(T,)>, frame_support::Never),
        ///See [`Pallet::eth_call`].
        #[codec(index = 0u8)]
        eth_call {
            #[allow(missing_docs)]
            action: TransactionAction,
            #[allow(missing_docs)]
            input: Vec<u8>,
            #[allow(missing_docs)]
            #[codec(compact)]
            value: BalanceOf<T>,
            #[allow(missing_docs)]
            #[codec(compact)]
            gas_limit: u64,
            #[allow(missing_docs)]
            #[codec(compact)]
            storage_limit: u32,
            #[allow(missing_docs)]
            access_list: Vec<AccessListItem>,
            #[allow(missing_docs)]
            #[codec(compact)]
            valid_until: T::BlockNumber,
        },
        ///See [`Pallet::eth_call_v2`].
        #[codec(index = 15u8)]
        eth_call_v2 {
            #[allow(missing_docs)]
            action: TransactionAction,
            #[allow(missing_docs)]
            input: Vec<u8>,
            #[allow(missing_docs)]
            #[codec(compact)]
            value: BalanceOf<T>,
            #[allow(missing_docs)]
            #[codec(compact)]
            gas_price: u64,
            #[allow(missing_docs)]
            #[codec(compact)]
            gas_limit: u64,
            #[allow(missing_docs)]
            access_list: Vec<AccessListItem>,
        },
        ///See [`Pallet::call`].
        #[codec(index = 1u8)]
        call {
            #[allow(missing_docs)]
            target: EvmAddress,
            #[allow(missing_docs)]
            input: Vec<u8>,
            #[allow(missing_docs)]
            #[codec(compact)]
            value: BalanceOf<T>,
            #[allow(missing_docs)]
            #[codec(compact)]
            gas_limit: u64,
            #[allow(missing_docs)]
            #[codec(compact)]
            storage_limit: u32,
            #[allow(missing_docs)]
            access_list: Vec<AccessListItem>,
        },
        ///See [`Pallet::scheduled_call`].
        #[codec(index = 2u8)]
        scheduled_call {
            #[allow(missing_docs)]
            from: EvmAddress,
            #[allow(missing_docs)]
            target: EvmAddress,
            #[allow(missing_docs)]
            input: Vec<u8>,
            #[allow(missing_docs)]
            #[codec(compact)]
            value: BalanceOf<T>,
            #[allow(missing_docs)]
            #[codec(compact)]
            gas_limit: u64,
            #[allow(missing_docs)]
            #[codec(compact)]
            storage_limit: u32,
            #[allow(missing_docs)]
            access_list: Vec<AccessListItem>,
        },
        ///See [`Pallet::create`].
        #[codec(index = 3u8)]
        create {
            #[allow(missing_docs)]
            input: Vec<u8>,
            #[allow(missing_docs)]
            #[codec(compact)]
            value: BalanceOf<T>,
            #[allow(missing_docs)]
            #[codec(compact)]
            gas_limit: u64,
            #[allow(missing_docs)]
            #[codec(compact)]
            storage_limit: u32,
            #[allow(missing_docs)]
            access_list: Vec<AccessListItem>,
        },
        ///See [`Pallet::create2`].
        #[codec(index = 4u8)]
        create2 {
            #[allow(missing_docs)]
            input: Vec<u8>,
            #[allow(missing_docs)]
            salt: H256,
            #[allow(missing_docs)]
            #[codec(compact)]
            value: BalanceOf<T>,
            #[allow(missing_docs)]
            #[codec(compact)]
            gas_limit: u64,
            #[allow(missing_docs)]
            #[codec(compact)]
            storage_limit: u32,
            #[allow(missing_docs)]
            access_list: Vec<AccessListItem>,
        },
        ///See [`Pallet::create_nft_contract`].
        #[codec(index = 5u8)]
        create_nft_contract {
            #[allow(missing_docs)]
            input: Vec<u8>,
            #[allow(missing_docs)]
            #[codec(compact)]
            value: BalanceOf<T>,
            #[allow(missing_docs)]
            #[codec(compact)]
            gas_limit: u64,
            #[allow(missing_docs)]
            #[codec(compact)]
            storage_limit: u32,
            #[allow(missing_docs)]
            access_list: Vec<AccessListItem>,
        },
        ///See [`Pallet::create_predeploy_contract`].
        #[codec(index = 6u8)]
        create_predeploy_contract {
            #[allow(missing_docs)]
            target: EvmAddress,
            #[allow(missing_docs)]
            input: Vec<u8>,
            #[allow(missing_docs)]
            #[codec(compact)]
            value: BalanceOf<T>,
            #[allow(missing_docs)]
            #[codec(compact)]
            gas_limit: u64,
            #[allow(missing_docs)]
            #[codec(compact)]
            storage_limit: u32,
            #[allow(missing_docs)]
            access_list: Vec<AccessListItem>,
        },
        ///See [`Pallet::transfer_maintainer`].
        #[codec(index = 7u8)]
        transfer_maintainer {
            #[allow(missing_docs)]
            contract: EvmAddress,
            #[allow(missing_docs)]
            new_maintainer: EvmAddress,
        },
        ///See [`Pallet::publish_contract`].
        #[codec(index = 8u8)]
        publish_contract { #[allow(missing_docs)] contract: EvmAddress },
        ///See [`Pallet::publish_free`].
        #[codec(index = 9u8)]
        publish_free { #[allow(missing_docs)] contract: EvmAddress },
        ///See [`Pallet::enable_contract_development`].
        #[codec(index = 10u8)]
        enable_contract_development {},
        ///See [`Pallet::disable_contract_development`].
        #[codec(index = 11u8)]
        disable_contract_development {},
        ///See [`Pallet::set_code`].
        #[codec(index = 12u8)]
        set_code {
            #[allow(missing_docs)]
            contract: EvmAddress,
            #[allow(missing_docs)]
            code: Vec<u8>,
        },
        ///See [`Pallet::selfdestruct`].
        #[codec(index = 13u8)]
        selfdestruct { #[allow(missing_docs)] contract: EvmAddress },
        ///See [`Pallet::strict_call`].
        #[codec(index = 14u8)]
        strict_call {
            #[allow(missing_docs)]
            target: EvmAddress,
            #[allow(missing_docs)]
            input: Vec<u8>,
            #[allow(missing_docs)]
            #[codec(compact)]
            value: BalanceOf<T>,
            #[allow(missing_docs)]
            #[codec(compact)]
            gas_limit: u64,
            #[allow(missing_docs)]
            #[codec(compact)]
            storage_limit: u32,
            #[allow(missing_docs)]
            access_list: Vec<AccessListItem>,
        },
    }
    const _: () = {
        impl<T: Config> core::fmt::Debug for Call<T> {
            fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
                fmt.write_str("<wasm:stripped>")
            }
        }
    };
    const _: () = {
        impl<T: Config> core::clone::Clone for Call<T> {
            fn clone(&self) -> Self {
                match self {
                    Self::__Ignore(ref _0, ref _1) => {
                        Self::__Ignore(
                            core::clone::Clone::clone(_0),
                            core::clone::Clone::clone(_1),
                        )
                    }
                    Self::eth_call {
                        ref action,
                        ref input,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                        ref access_list,
                        ref valid_until,
                    } => {
                        Self::eth_call {
                            action: core::clone::Clone::clone(action),
                            input: core::clone::Clone::clone(input),
                            value: core::clone::Clone::clone(value),
                            gas_limit: core::clone::Clone::clone(gas_limit),
                            storage_limit: core::clone::Clone::clone(storage_limit),
                            access_list: core::clone::Clone::clone(access_list),
                            valid_until: core::clone::Clone::clone(valid_until),
                        }
                    }
                    Self::eth_call_v2 {
                        ref action,
                        ref input,
                        ref value,
                        ref gas_price,
                        ref gas_limit,
                        ref access_list,
                    } => {
                        Self::eth_call_v2 {
                            action: core::clone::Clone::clone(action),
                            input: core::clone::Clone::clone(input),
                            value: core::clone::Clone::clone(value),
                            gas_price: core::clone::Clone::clone(gas_price),
                            gas_limit: core::clone::Clone::clone(gas_limit),
                            access_list: core::clone::Clone::clone(access_list),
                        }
                    }
                    Self::call {
                        ref target,
                        ref input,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                        ref access_list,
                    } => {
                        Self::call {
                            target: core::clone::Clone::clone(target),
                            input: core::clone::Clone::clone(input),
                            value: core::clone::Clone::clone(value),
                            gas_limit: core::clone::Clone::clone(gas_limit),
                            storage_limit: core::clone::Clone::clone(storage_limit),
                            access_list: core::clone::Clone::clone(access_list),
                        }
                    }
                    Self::scheduled_call {
                        ref from,
                        ref target,
                        ref input,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                        ref access_list,
                    } => {
                        Self::scheduled_call {
                            from: core::clone::Clone::clone(from),
                            target: core::clone::Clone::clone(target),
                            input: core::clone::Clone::clone(input),
                            value: core::clone::Clone::clone(value),
                            gas_limit: core::clone::Clone::clone(gas_limit),
                            storage_limit: core::clone::Clone::clone(storage_limit),
                            access_list: core::clone::Clone::clone(access_list),
                        }
                    }
                    Self::create {
                        ref input,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                        ref access_list,
                    } => {
                        Self::create {
                            input: core::clone::Clone::clone(input),
                            value: core::clone::Clone::clone(value),
                            gas_limit: core::clone::Clone::clone(gas_limit),
                            storage_limit: core::clone::Clone::clone(storage_limit),
                            access_list: core::clone::Clone::clone(access_list),
                        }
                    }
                    Self::create2 {
                        ref input,
                        ref salt,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                        ref access_list,
                    } => {
                        Self::create2 {
                            input: core::clone::Clone::clone(input),
                            salt: core::clone::Clone::clone(salt),
                            value: core::clone::Clone::clone(value),
                            gas_limit: core::clone::Clone::clone(gas_limit),
                            storage_limit: core::clone::Clone::clone(storage_limit),
                            access_list: core::clone::Clone::clone(access_list),
                        }
                    }
                    Self::create_nft_contract {
                        ref input,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                        ref access_list,
                    } => {
                        Self::create_nft_contract {
                            input: core::clone::Clone::clone(input),
                            value: core::clone::Clone::clone(value),
                            gas_limit: core::clone::Clone::clone(gas_limit),
                            storage_limit: core::clone::Clone::clone(storage_limit),
                            access_list: core::clone::Clone::clone(access_list),
                        }
                    }
                    Self::create_predeploy_contract {
                        ref target,
                        ref input,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                        ref access_list,
                    } => {
                        Self::create_predeploy_contract {
                            target: core::clone::Clone::clone(target),
                            input: core::clone::Clone::clone(input),
                            value: core::clone::Clone::clone(value),
                            gas_limit: core::clone::Clone::clone(gas_limit),
                            storage_limit: core::clone::Clone::clone(storage_limit),
                            access_list: core::clone::Clone::clone(access_list),
                        }
                    }
                    Self::transfer_maintainer { ref contract, ref new_maintainer } => {
                        Self::transfer_maintainer {
                            contract: core::clone::Clone::clone(contract),
                            new_maintainer: core::clone::Clone::clone(new_maintainer),
                        }
                    }
                    Self::publish_contract { ref contract } => {
                        Self::publish_contract {
                            contract: core::clone::Clone::clone(contract),
                        }
                    }
                    Self::publish_free { ref contract } => {
                        Self::publish_free {
                            contract: core::clone::Clone::clone(contract),
                        }
                    }
                    Self::enable_contract_development {} => {
                        Self::enable_contract_development {
                        }
                    }
                    Self::disable_contract_development {} => {
                        Self::disable_contract_development {
                        }
                    }
                    Self::set_code { ref contract, ref code } => {
                        Self::set_code {
                            contract: core::clone::Clone::clone(contract),
                            code: core::clone::Clone::clone(code),
                        }
                    }
                    Self::selfdestruct { ref contract } => {
                        Self::selfdestruct {
                            contract: core::clone::Clone::clone(contract),
                        }
                    }
                    Self::strict_call {
                        ref target,
                        ref input,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                        ref access_list,
                    } => {
                        Self::strict_call {
                            target: core::clone::Clone::clone(target),
                            input: core::clone::Clone::clone(input),
                            value: core::clone::Clone::clone(value),
                            gas_limit: core::clone::Clone::clone(gas_limit),
                            storage_limit: core::clone::Clone::clone(storage_limit),
                            access_list: core::clone::Clone::clone(access_list),
                        }
                    }
                }
            }
        }
    };
    const _: () = {
        impl<T: Config> core::cmp::Eq for Call<T> {}
    };
    const _: () = {
        impl<T: Config> core::cmp::PartialEq for Call<T> {
            fn eq(&self, other: &Self) -> bool {
                match (self, other) {
                    (Self::__Ignore(_0, _1), Self::__Ignore(_0_other, _1_other)) => {
                        true && _0 == _0_other && _1 == _1_other
                    }
                    (
                        Self::eth_call {
                            action,
                            input,
                            value,
                            gas_limit,
                            storage_limit,
                            access_list,
                            valid_until,
                        },
                        Self::eth_call {
                            action: _0,
                            input: _1,
                            value: _2,
                            gas_limit: _3,
                            storage_limit: _4,
                            access_list: _5,
                            valid_until: _6,
                        },
                    ) => {
                        true && action == _0 && input == _1 && value == _2
                            && gas_limit == _3 && storage_limit == _4
                            && access_list == _5 && valid_until == _6
                    }
                    (
                        Self::eth_call_v2 {
                            action,
                            input,
                            value,
                            gas_price,
                            gas_limit,
                            access_list,
                        },
                        Self::eth_call_v2 {
                            action: _0,
                            input: _1,
                            value: _2,
                            gas_price: _3,
                            gas_limit: _4,
                            access_list: _5,
                        },
                    ) => {
                        true && action == _0 && input == _1 && value == _2
                            && gas_price == _3 && gas_limit == _4 && access_list == _5
                    }
                    (
                        Self::call {
                            target,
                            input,
                            value,
                            gas_limit,
                            storage_limit,
                            access_list,
                        },
                        Self::call {
                            target: _0,
                            input: _1,
                            value: _2,
                            gas_limit: _3,
                            storage_limit: _4,
                            access_list: _5,
                        },
                    ) => {
                        true && target == _0 && input == _1 && value == _2
                            && gas_limit == _3 && storage_limit == _4
                            && access_list == _5
                    }
                    (
                        Self::scheduled_call {
                            from,
                            target,
                            input,
                            value,
                            gas_limit,
                            storage_limit,
                            access_list,
                        },
                        Self::scheduled_call {
                            from: _0,
                            target: _1,
                            input: _2,
                            value: _3,
                            gas_limit: _4,
                            storage_limit: _5,
                            access_list: _6,
                        },
                    ) => {
                        true && from == _0 && target == _1 && input == _2 && value == _3
                            && gas_limit == _4 && storage_limit == _5
                            && access_list == _6
                    }
                    (
                        Self::create {
                            input,
                            value,
                            gas_limit,
                            storage_limit,
                            access_list,
                        },
                        Self::create {
                            input: _0,
                            value: _1,
                            gas_limit: _2,
                            storage_limit: _3,
                            access_list: _4,
                        },
                    ) => {
                        true && input == _0 && value == _1 && gas_limit == _2
                            && storage_limit == _3 && access_list == _4
                    }
                    (
                        Self::create2 {
                            input,
                            salt,
                            value,
                            gas_limit,
                            storage_limit,
                            access_list,
                        },
                        Self::create2 {
                            input: _0,
                            salt: _1,
                            value: _2,
                            gas_limit: _3,
                            storage_limit: _4,
                            access_list: _5,
                        },
                    ) => {
                        true && input == _0 && salt == _1 && value == _2
                            && gas_limit == _3 && storage_limit == _4
                            && access_list == _5
                    }
                    (
                        Self::create_nft_contract {
                            input,
                            value,
                            gas_limit,
                            storage_limit,
                            access_list,
                        },
                        Self::create_nft_contract {
                            input: _0,
                            value: _1,
                            gas_limit: _2,
                            storage_limit: _3,
                            access_list: _4,
                        },
                    ) => {
                        true && input == _0 && value == _1 && gas_limit == _2
                            && storage_limit == _3 && access_list == _4
                    }
                    (
                        Self::create_predeploy_contract {
                            target,
                            input,
                            value,
                            gas_limit,
                            storage_limit,
                            access_list,
                        },
                        Self::create_predeploy_contract {
                            target: _0,
                            input: _1,
                            value: _2,
                            gas_limit: _3,
                            storage_limit: _4,
                            access_list: _5,
                        },
                    ) => {
                        true && target == _0 && input == _1 && value == _2
                            && gas_limit == _3 && storage_limit == _4
                            && access_list == _5
                    }
                    (
                        Self::transfer_maintainer { contract, new_maintainer },
                        Self::transfer_maintainer { contract: _0, new_maintainer: _1 },
                    ) => true && contract == _0 && new_maintainer == _1,
                    (
                        Self::publish_contract { contract },
                        Self::publish_contract { contract: _0 },
                    ) => true && contract == _0,
                    (
                        Self::publish_free { contract },
                        Self::publish_free { contract: _0 },
                    ) => true && contract == _0,
                    (
                        Self::enable_contract_development {},
                        Self::enable_contract_development {},
                    ) => true,
                    (
                        Self::disable_contract_development {},
                        Self::disable_contract_development {},
                    ) => true,
                    (
                        Self::set_code { contract, code },
                        Self::set_code { contract: _0, code: _1 },
                    ) => true && contract == _0 && code == _1,
                    (
                        Self::selfdestruct { contract },
                        Self::selfdestruct { contract: _0 },
                    ) => true && contract == _0,
                    (
                        Self::strict_call {
                            target,
                            input,
                            value,
                            gas_limit,
                            storage_limit,
                            access_list,
                        },
                        Self::strict_call {
                            target: _0,
                            input: _1,
                            value: _2,
                            gas_limit: _3,
                            storage_limit: _4,
                            access_list: _5,
                        },
                    ) => {
                        true && target == _0 && input == _1 && value == _2
                            && gas_limit == _3 && storage_limit == _4
                            && access_list == _5
                    }
                    (Self::__Ignore { .. }, Self::eth_call { .. }) => false,
                    (Self::__Ignore { .. }, Self::eth_call_v2 { .. }) => false,
                    (Self::__Ignore { .. }, Self::call { .. }) => false,
                    (Self::__Ignore { .. }, Self::scheduled_call { .. }) => false,
                    (Self::__Ignore { .. }, Self::create { .. }) => false,
                    (Self::__Ignore { .. }, Self::create2 { .. }) => false,
                    (Self::__Ignore { .. }, Self::create_nft_contract { .. }) => false,
                    (Self::__Ignore { .. }, Self::create_predeploy_contract { .. }) => {
                        false
                    }
                    (Self::__Ignore { .. }, Self::transfer_maintainer { .. }) => false,
                    (Self::__Ignore { .. }, Self::publish_contract { .. }) => false,
                    (Self::__Ignore { .. }, Self::publish_free { .. }) => false,
                    (Self::__Ignore { .. }, Self::enable_contract_development { .. }) => {
                        false
                    }
                    (
                        Self::__Ignore { .. },
                        Self::disable_contract_development { .. },
                    ) => false,
                    (Self::__Ignore { .. }, Self::set_code { .. }) => false,
                    (Self::__Ignore { .. }, Self::selfdestruct { .. }) => false,
                    (Self::__Ignore { .. }, Self::strict_call { .. }) => false,
                    (Self::eth_call { .. }, Self::__Ignore { .. }) => false,
                    (Self::eth_call { .. }, Self::eth_call_v2 { .. }) => false,
                    (Self::eth_call { .. }, Self::call { .. }) => false,
                    (Self::eth_call { .. }, Self::scheduled_call { .. }) => false,
                    (Self::eth_call { .. }, Self::create { .. }) => false,
                    (Self::eth_call { .. }, Self::create2 { .. }) => false,
                    (Self::eth_call { .. }, Self::create_nft_contract { .. }) => false,
                    (Self::eth_call { .. }, Self::create_predeploy_contract { .. }) => {
                        false
                    }
                    (Self::eth_call { .. }, Self::transfer_maintainer { .. }) => false,
                    (Self::eth_call { .. }, Self::publish_contract { .. }) => false,
                    (Self::eth_call { .. }, Self::publish_free { .. }) => false,
                    (Self::eth_call { .. }, Self::enable_contract_development { .. }) => {
                        false
                    }
                    (
                        Self::eth_call { .. },
                        Self::disable_contract_development { .. },
                    ) => false,
                    (Self::eth_call { .. }, Self::set_code { .. }) => false,
                    (Self::eth_call { .. }, Self::selfdestruct { .. }) => false,
                    (Self::eth_call { .. }, Self::strict_call { .. }) => false,
                    (Self::eth_call_v2 { .. }, Self::__Ignore { .. }) => false,
                    (Self::eth_call_v2 { .. }, Self::eth_call { .. }) => false,
                    (Self::eth_call_v2 { .. }, Self::call { .. }) => false,
                    (Self::eth_call_v2 { .. }, Self::scheduled_call { .. }) => false,
                    (Self::eth_call_v2 { .. }, Self::create { .. }) => false,
                    (Self::eth_call_v2 { .. }, Self::create2 { .. }) => false,
                    (Self::eth_call_v2 { .. }, Self::create_nft_contract { .. }) => false,
                    (
                        Self::eth_call_v2 { .. },
                        Self::create_predeploy_contract { .. },
                    ) => false,
                    (Self::eth_call_v2 { .. }, Self::transfer_maintainer { .. }) => false,
                    (Self::eth_call_v2 { .. }, Self::publish_contract { .. }) => false,
                    (Self::eth_call_v2 { .. }, Self::publish_free { .. }) => false,
                    (
                        Self::eth_call_v2 { .. },
                        Self::enable_contract_development { .. },
                    ) => false,
                    (
                        Self::eth_call_v2 { .. },
                        Self::disable_contract_development { .. },
                    ) => false,
                    (Self::eth_call_v2 { .. }, Self::set_code { .. }) => false,
                    (Self::eth_call_v2 { .. }, Self::selfdestruct { .. }) => false,
                    (Self::eth_call_v2 { .. }, Self::strict_call { .. }) => false,
                    (Self::call { .. }, Self::__Ignore { .. }) => false,
                    (Self::call { .. }, Self::eth_call { .. }) => false,
                    (Self::call { .. }, Self::eth_call_v2 { .. }) => false,
                    (Self::call { .. }, Self::scheduled_call { .. }) => false,
                    (Self::call { .. }, Self::create { .. }) => false,
                    (Self::call { .. }, Self::create2 { .. }) => false,
                    (Self::call { .. }, Self::create_nft_contract { .. }) => false,
                    (Self::call { .. }, Self::create_predeploy_contract { .. }) => false,
                    (Self::call { .. }, Self::transfer_maintainer { .. }) => false,
                    (Self::call { .. }, Self::publish_contract { .. }) => false,
                    (Self::call { .. }, Self::publish_free { .. }) => false,
                    (Self::call { .. }, Self::enable_contract_development { .. }) => {
                        false
                    }
                    (Self::call { .. }, Self::disable_contract_development { .. }) => {
                        false
                    }
                    (Self::call { .. }, Self::set_code { .. }) => false,
                    (Self::call { .. }, Self::selfdestruct { .. }) => false,
                    (Self::call { .. }, Self::strict_call { .. }) => false,
                    (Self::scheduled_call { .. }, Self::__Ignore { .. }) => false,
                    (Self::scheduled_call { .. }, Self::eth_call { .. }) => false,
                    (Self::scheduled_call { .. }, Self::eth_call_v2 { .. }) => false,
                    (Self::scheduled_call { .. }, Self::call { .. }) => false,
                    (Self::scheduled_call { .. }, Self::create { .. }) => false,
                    (Self::scheduled_call { .. }, Self::create2 { .. }) => false,
                    (Self::scheduled_call { .. }, Self::create_nft_contract { .. }) => {
                        false
                    }
                    (
                        Self::scheduled_call { .. },
                        Self::create_predeploy_contract { .. },
                    ) => false,
                    (Self::scheduled_call { .. }, Self::transfer_maintainer { .. }) => {
                        false
                    }
                    (Self::scheduled_call { .. }, Self::publish_contract { .. }) => false,
                    (Self::scheduled_call { .. }, Self::publish_free { .. }) => false,
                    (
                        Self::scheduled_call { .. },
                        Self::enable_contract_development { .. },
                    ) => false,
                    (
                        Self::scheduled_call { .. },
                        Self::disable_contract_development { .. },
                    ) => false,
                    (Self::scheduled_call { .. }, Self::set_code { .. }) => false,
                    (Self::scheduled_call { .. }, Self::selfdestruct { .. }) => false,
                    (Self::scheduled_call { .. }, Self::strict_call { .. }) => false,
                    (Self::create { .. }, Self::__Ignore { .. }) => false,
                    (Self::create { .. }, Self::eth_call { .. }) => false,
                    (Self::create { .. }, Self::eth_call_v2 { .. }) => false,
                    (Self::create { .. }, Self::call { .. }) => false,
                    (Self::create { .. }, Self::scheduled_call { .. }) => false,
                    (Self::create { .. }, Self::create2 { .. }) => false,
                    (Self::create { .. }, Self::create_nft_contract { .. }) => false,
                    (Self::create { .. }, Self::create_predeploy_contract { .. }) => {
                        false
                    }
                    (Self::create { .. }, Self::transfer_maintainer { .. }) => false,
                    (Self::create { .. }, Self::publish_contract { .. }) => false,
                    (Self::create { .. }, Self::publish_free { .. }) => false,
                    (Self::create { .. }, Self::enable_contract_development { .. }) => {
                        false
                    }
                    (Self::create { .. }, Self::disable_contract_development { .. }) => {
                        false
                    }
                    (Self::create { .. }, Self::set_code { .. }) => false,
                    (Self::create { .. }, Self::selfdestruct { .. }) => false,
                    (Self::create { .. }, Self::strict_call { .. }) => false,
                    (Self::create2 { .. }, Self::__Ignore { .. }) => false,
                    (Self::create2 { .. }, Self::eth_call { .. }) => false,
                    (Self::create2 { .. }, Self::eth_call_v2 { .. }) => false,
                    (Self::create2 { .. }, Self::call { .. }) => false,
                    (Self::create2 { .. }, Self::scheduled_call { .. }) => false,
                    (Self::create2 { .. }, Self::create { .. }) => false,
                    (Self::create2 { .. }, Self::create_nft_contract { .. }) => false,
                    (Self::create2 { .. }, Self::create_predeploy_contract { .. }) => {
                        false
                    }
                    (Self::create2 { .. }, Self::transfer_maintainer { .. }) => false,
                    (Self::create2 { .. }, Self::publish_contract { .. }) => false,
                    (Self::create2 { .. }, Self::publish_free { .. }) => false,
                    (Self::create2 { .. }, Self::enable_contract_development { .. }) => {
                        false
                    }
                    (Self::create2 { .. }, Self::disable_contract_development { .. }) => {
                        false
                    }
                    (Self::create2 { .. }, Self::set_code { .. }) => false,
                    (Self::create2 { .. }, Self::selfdestruct { .. }) => false,
                    (Self::create2 { .. }, Self::strict_call { .. }) => false,
                    (Self::create_nft_contract { .. }, Self::__Ignore { .. }) => false,
                    (Self::create_nft_contract { .. }, Self::eth_call { .. }) => false,
                    (Self::create_nft_contract { .. }, Self::eth_call_v2 { .. }) => false,
                    (Self::create_nft_contract { .. }, Self::call { .. }) => false,
                    (Self::create_nft_contract { .. }, Self::scheduled_call { .. }) => {
                        false
                    }
                    (Self::create_nft_contract { .. }, Self::create { .. }) => false,
                    (Self::create_nft_contract { .. }, Self::create2 { .. }) => false,
                    (
                        Self::create_nft_contract { .. },
                        Self::create_predeploy_contract { .. },
                    ) => false,
                    (
                        Self::create_nft_contract { .. },
                        Self::transfer_maintainer { .. },
                    ) => false,
                    (Self::create_nft_contract { .. }, Self::publish_contract { .. }) => {
                        false
                    }
                    (Self::create_nft_contract { .. }, Self::publish_free { .. }) => {
                        false
                    }
                    (
                        Self::create_nft_contract { .. },
                        Self::enable_contract_development { .. },
                    ) => false,
                    (
                        Self::create_nft_contract { .. },
                        Self::disable_contract_development { .. },
                    ) => false,
                    (Self::create_nft_contract { .. }, Self::set_code { .. }) => false,
                    (Self::create_nft_contract { .. }, Self::selfdestruct { .. }) => {
                        false
                    }
                    (Self::create_nft_contract { .. }, Self::strict_call { .. }) => false,
                    (Self::create_predeploy_contract { .. }, Self::__Ignore { .. }) => {
                        false
                    }
                    (Self::create_predeploy_contract { .. }, Self::eth_call { .. }) => {
                        false
                    }
                    (
                        Self::create_predeploy_contract { .. },
                        Self::eth_call_v2 { .. },
                    ) => false,
                    (Self::create_predeploy_contract { .. }, Self::call { .. }) => false,
                    (
                        Self::create_predeploy_contract { .. },
                        Self::scheduled_call { .. },
                    ) => false,
                    (Self::create_predeploy_contract { .. }, Self::create { .. }) => {
                        false
                    }
                    (Self::create_predeploy_contract { .. }, Self::create2 { .. }) => {
                        false
                    }
                    (
                        Self::create_predeploy_contract { .. },
                        Self::create_nft_contract { .. },
                    ) => false,
                    (
                        Self::create_predeploy_contract { .. },
                        Self::transfer_maintainer { .. },
                    ) => false,
                    (
                        Self::create_predeploy_contract { .. },
                        Self::publish_contract { .. },
                    ) => false,
                    (
                        Self::create_predeploy_contract { .. },
                        Self::publish_free { .. },
                    ) => false,
                    (
                        Self::create_predeploy_contract { .. },
                        Self::enable_contract_development { .. },
                    ) => false,
                    (
                        Self::create_predeploy_contract { .. },
                        Self::disable_contract_development { .. },
                    ) => false,
                    (Self::create_predeploy_contract { .. }, Self::set_code { .. }) => {
                        false
                    }
                    (
                        Self::create_predeploy_contract { .. },
                        Self::selfdestruct { .. },
                    ) => false,
                    (
                        Self::create_predeploy_contract { .. },
                        Self::strict_call { .. },
                    ) => false,
                    (Self::transfer_maintainer { .. }, Self::__Ignore { .. }) => false,
                    (Self::transfer_maintainer { .. }, Self::eth_call { .. }) => false,
                    (Self::transfer_maintainer { .. }, Self::eth_call_v2 { .. }) => false,
                    (Self::transfer_maintainer { .. }, Self::call { .. }) => false,
                    (Self::transfer_maintainer { .. }, Self::scheduled_call { .. }) => {
                        false
                    }
                    (Self::transfer_maintainer { .. }, Self::create { .. }) => false,
                    (Self::transfer_maintainer { .. }, Self::create2 { .. }) => false,
                    (
                        Self::transfer_maintainer { .. },
                        Self::create_nft_contract { .. },
                    ) => false,
                    (
                        Self::transfer_maintainer { .. },
                        Self::create_predeploy_contract { .. },
                    ) => false,
                    (Self::transfer_maintainer { .. }, Self::publish_contract { .. }) => {
                        false
                    }
                    (Self::transfer_maintainer { .. }, Self::publish_free { .. }) => {
                        false
                    }
                    (
                        Self::transfer_maintainer { .. },
                        Self::enable_contract_development { .. },
                    ) => false,
                    (
                        Self::transfer_maintainer { .. },
                        Self::disable_contract_development { .. },
                    ) => false,
                    (Self::transfer_maintainer { .. }, Self::set_code { .. }) => false,
                    (Self::transfer_maintainer { .. }, Self::selfdestruct { .. }) => {
                        false
                    }
                    (Self::transfer_maintainer { .. }, Self::strict_call { .. }) => false,
                    (Self::publish_contract { .. }, Self::__Ignore { .. }) => false,
                    (Self::publish_contract { .. }, Self::eth_call { .. }) => false,
                    (Self::publish_contract { .. }, Self::eth_call_v2 { .. }) => false,
                    (Self::publish_contract { .. }, Self::call { .. }) => false,
                    (Self::publish_contract { .. }, Self::scheduled_call { .. }) => false,
                    (Self::publish_contract { .. }, Self::create { .. }) => false,
                    (Self::publish_contract { .. }, Self::create2 { .. }) => false,
                    (Self::publish_contract { .. }, Self::create_nft_contract { .. }) => {
                        false
                    }
                    (
                        Self::publish_contract { .. },
                        Self::create_predeploy_contract { .. },
                    ) => false,
                    (Self::publish_contract { .. }, Self::transfer_maintainer { .. }) => {
                        false
                    }
                    (Self::publish_contract { .. }, Self::publish_free { .. }) => false,
                    (
                        Self::publish_contract { .. },
                        Self::enable_contract_development { .. },
                    ) => false,
                    (
                        Self::publish_contract { .. },
                        Self::disable_contract_development { .. },
                    ) => false,
                    (Self::publish_contract { .. }, Self::set_code { .. }) => false,
                    (Self::publish_contract { .. }, Self::selfdestruct { .. }) => false,
                    (Self::publish_contract { .. }, Self::strict_call { .. }) => false,
                    (Self::publish_free { .. }, Self::__Ignore { .. }) => false,
                    (Self::publish_free { .. }, Self::eth_call { .. }) => false,
                    (Self::publish_free { .. }, Self::eth_call_v2 { .. }) => false,
                    (Self::publish_free { .. }, Self::call { .. }) => false,
                    (Self::publish_free { .. }, Self::scheduled_call { .. }) => false,
                    (Self::publish_free { .. }, Self::create { .. }) => false,
                    (Self::publish_free { .. }, Self::create2 { .. }) => false,
                    (Self::publish_free { .. }, Self::create_nft_contract { .. }) => {
                        false
                    }
                    (
                        Self::publish_free { .. },
                        Self::create_predeploy_contract { .. },
                    ) => false,
                    (Self::publish_free { .. }, Self::transfer_maintainer { .. }) => {
                        false
                    }
                    (Self::publish_free { .. }, Self::publish_contract { .. }) => false,
                    (
                        Self::publish_free { .. },
                        Self::enable_contract_development { .. },
                    ) => false,
                    (
                        Self::publish_free { .. },
                        Self::disable_contract_development { .. },
                    ) => false,
                    (Self::publish_free { .. }, Self::set_code { .. }) => false,
                    (Self::publish_free { .. }, Self::selfdestruct { .. }) => false,
                    (Self::publish_free { .. }, Self::strict_call { .. }) => false,
                    (Self::enable_contract_development { .. }, Self::__Ignore { .. }) => {
                        false
                    }
                    (Self::enable_contract_development { .. }, Self::eth_call { .. }) => {
                        false
                    }
                    (
                        Self::enable_contract_development { .. },
                        Self::eth_call_v2 { .. },
                    ) => false,
                    (Self::enable_contract_development { .. }, Self::call { .. }) => {
                        false
                    }
                    (
                        Self::enable_contract_development { .. },
                        Self::scheduled_call { .. },
                    ) => false,
                    (Self::enable_contract_development { .. }, Self::create { .. }) => {
                        false
                    }
                    (Self::enable_contract_development { .. }, Self::create2 { .. }) => {
                        false
                    }
                    (
                        Self::enable_contract_development { .. },
                        Self::create_nft_contract { .. },
                    ) => false,
                    (
                        Self::enable_contract_development { .. },
                        Self::create_predeploy_contract { .. },
                    ) => false,
                    (
                        Self::enable_contract_development { .. },
                        Self::transfer_maintainer { .. },
                    ) => false,
                    (
                        Self::enable_contract_development { .. },
                        Self::publish_contract { .. },
                    ) => false,
                    (
                        Self::enable_contract_development { .. },
                        Self::publish_free { .. },
                    ) => false,
                    (
                        Self::enable_contract_development { .. },
                        Self::disable_contract_development { .. },
                    ) => false,
                    (Self::enable_contract_development { .. }, Self::set_code { .. }) => {
                        false
                    }
                    (
                        Self::enable_contract_development { .. },
                        Self::selfdestruct { .. },
                    ) => false,
                    (
                        Self::enable_contract_development { .. },
                        Self::strict_call { .. },
                    ) => false,
                    (
                        Self::disable_contract_development { .. },
                        Self::__Ignore { .. },
                    ) => false,
                    (
                        Self::disable_contract_development { .. },
                        Self::eth_call { .. },
                    ) => false,
                    (
                        Self::disable_contract_development { .. },
                        Self::eth_call_v2 { .. },
                    ) => false,
                    (Self::disable_contract_development { .. }, Self::call { .. }) => {
                        false
                    }
                    (
                        Self::disable_contract_development { .. },
                        Self::scheduled_call { .. },
                    ) => false,
                    (Self::disable_contract_development { .. }, Self::create { .. }) => {
                        false
                    }
                    (Self::disable_contract_development { .. }, Self::create2 { .. }) => {
                        false
                    }
                    (
                        Self::disable_contract_development { .. },
                        Self::create_nft_contract { .. },
                    ) => false,
                    (
                        Self::disable_contract_development { .. },
                        Self::create_predeploy_contract { .. },
                    ) => false,
                    (
                        Self::disable_contract_development { .. },
                        Self::transfer_maintainer { .. },
                    ) => false,
                    (
                        Self::disable_contract_development { .. },
                        Self::publish_contract { .. },
                    ) => false,
                    (
                        Self::disable_contract_development { .. },
                        Self::publish_free { .. },
                    ) => false,
                    (
                        Self::disable_contract_development { .. },
                        Self::enable_contract_development { .. },
                    ) => false,
                    (
                        Self::disable_contract_development { .. },
                        Self::set_code { .. },
                    ) => false,
                    (
                        Self::disable_contract_development { .. },
                        Self::selfdestruct { .. },
                    ) => false,
                    (
                        Self::disable_contract_development { .. },
                        Self::strict_call { .. },
                    ) => false,
                    (Self::set_code { .. }, Self::__Ignore { .. }) => false,
                    (Self::set_code { .. }, Self::eth_call { .. }) => false,
                    (Self::set_code { .. }, Self::eth_call_v2 { .. }) => false,
                    (Self::set_code { .. }, Self::call { .. }) => false,
                    (Self::set_code { .. }, Self::scheduled_call { .. }) => false,
                    (Self::set_code { .. }, Self::create { .. }) => false,
                    (Self::set_code { .. }, Self::create2 { .. }) => false,
                    (Self::set_code { .. }, Self::create_nft_contract { .. }) => false,
                    (Self::set_code { .. }, Self::create_predeploy_contract { .. }) => {
                        false
                    }
                    (Self::set_code { .. }, Self::transfer_maintainer { .. }) => false,
                    (Self::set_code { .. }, Self::publish_contract { .. }) => false,
                    (Self::set_code { .. }, Self::publish_free { .. }) => false,
                    (Self::set_code { .. }, Self::enable_contract_development { .. }) => {
                        false
                    }
                    (
                        Self::set_code { .. },
                        Self::disable_contract_development { .. },
                    ) => false,
                    (Self::set_code { .. }, Self::selfdestruct { .. }) => false,
                    (Self::set_code { .. }, Self::strict_call { .. }) => false,
                    (Self::selfdestruct { .. }, Self::__Ignore { .. }) => false,
                    (Self::selfdestruct { .. }, Self::eth_call { .. }) => false,
                    (Self::selfdestruct { .. }, Self::eth_call_v2 { .. }) => false,
                    (Self::selfdestruct { .. }, Self::call { .. }) => false,
                    (Self::selfdestruct { .. }, Self::scheduled_call { .. }) => false,
                    (Self::selfdestruct { .. }, Self::create { .. }) => false,
                    (Self::selfdestruct { .. }, Self::create2 { .. }) => false,
                    (Self::selfdestruct { .. }, Self::create_nft_contract { .. }) => {
                        false
                    }
                    (
                        Self::selfdestruct { .. },
                        Self::create_predeploy_contract { .. },
                    ) => false,
                    (Self::selfdestruct { .. }, Self::transfer_maintainer { .. }) => {
                        false
                    }
                    (Self::selfdestruct { .. }, Self::publish_contract { .. }) => false,
                    (Self::selfdestruct { .. }, Self::publish_free { .. }) => false,
                    (
                        Self::selfdestruct { .. },
                        Self::enable_contract_development { .. },
                    ) => false,
                    (
                        Self::selfdestruct { .. },
                        Self::disable_contract_development { .. },
                    ) => false,
                    (Self::selfdestruct { .. }, Self::set_code { .. }) => false,
                    (Self::selfdestruct { .. }, Self::strict_call { .. }) => false,
                    (Self::strict_call { .. }, Self::__Ignore { .. }) => false,
                    (Self::strict_call { .. }, Self::eth_call { .. }) => false,
                    (Self::strict_call { .. }, Self::eth_call_v2 { .. }) => false,
                    (Self::strict_call { .. }, Self::call { .. }) => false,
                    (Self::strict_call { .. }, Self::scheduled_call { .. }) => false,
                    (Self::strict_call { .. }, Self::create { .. }) => false,
                    (Self::strict_call { .. }, Self::create2 { .. }) => false,
                    (Self::strict_call { .. }, Self::create_nft_contract { .. }) => false,
                    (
                        Self::strict_call { .. },
                        Self::create_predeploy_contract { .. },
                    ) => false,
                    (Self::strict_call { .. }, Self::transfer_maintainer { .. }) => false,
                    (Self::strict_call { .. }, Self::publish_contract { .. }) => false,
                    (Self::strict_call { .. }, Self::publish_free { .. }) => false,
                    (
                        Self::strict_call { .. },
                        Self::enable_contract_development { .. },
                    ) => false,
                    (
                        Self::strict_call { .. },
                        Self::disable_contract_development { .. },
                    ) => false,
                    (Self::strict_call { .. }, Self::set_code { .. }) => false,
                    (Self::strict_call { .. }, Self::selfdestruct { .. }) => false,
                }
            }
        }
    };
    #[allow(deprecated)]
    const _: () = {
        #[allow(non_camel_case_types)]
        #[automatically_derived]
        impl<T: Config> ::codec::Encode for Call<T> {
            fn size_hint(&self) -> usize {
                1_usize
                    + match *self {
                        Call::eth_call {
                            ref action,
                            ref input,
                            ref value,
                            ref gas_limit,
                            ref storage_limit,
                            ref access_list,
                            ref valid_until,
                        } => {
                            0_usize
                                .saturating_add(::codec::Encode::size_hint(action))
                                .saturating_add(::codec::Encode::size_hint(input))
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<BalanceOf<
                                            T,
                                        > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            BalanceOf<T>,
                                        >>::RefType::from(value),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u64,
                                        >>::RefType::from(gas_limit),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u32 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u32,
                                        >>::RefType::from(storage_limit),
                                    ),
                                )
                                .saturating_add(::codec::Encode::size_hint(access_list))
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<T::BlockNumber as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            T::BlockNumber,
                                        >>::RefType::from(valid_until),
                                    ),
                                )
                        }
                        Call::eth_call_v2 {
                            ref action,
                            ref input,
                            ref value,
                            ref gas_price,
                            ref gas_limit,
                            ref access_list,
                        } => {
                            0_usize
                                .saturating_add(::codec::Encode::size_hint(action))
                                .saturating_add(::codec::Encode::size_hint(input))
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<BalanceOf<
                                            T,
                                        > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            BalanceOf<T>,
                                        >>::RefType::from(value),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u64,
                                        >>::RefType::from(gas_price),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u64,
                                        >>::RefType::from(gas_limit),
                                    ),
                                )
                                .saturating_add(::codec::Encode::size_hint(access_list))
                        }
                        Call::call {
                            ref target,
                            ref input,
                            ref value,
                            ref gas_limit,
                            ref storage_limit,
                            ref access_list,
                        } => {
                            0_usize
                                .saturating_add(::codec::Encode::size_hint(target))
                                .saturating_add(::codec::Encode::size_hint(input))
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<BalanceOf<
                                            T,
                                        > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            BalanceOf<T>,
                                        >>::RefType::from(value),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u64,
                                        >>::RefType::from(gas_limit),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u32 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u32,
                                        >>::RefType::from(storage_limit),
                                    ),
                                )
                                .saturating_add(::codec::Encode::size_hint(access_list))
                        }
                        Call::scheduled_call {
                            ref from,
                            ref target,
                            ref input,
                            ref value,
                            ref gas_limit,
                            ref storage_limit,
                            ref access_list,
                        } => {
                            0_usize
                                .saturating_add(::codec::Encode::size_hint(from))
                                .saturating_add(::codec::Encode::size_hint(target))
                                .saturating_add(::codec::Encode::size_hint(input))
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<BalanceOf<
                                            T,
                                        > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            BalanceOf<T>,
                                        >>::RefType::from(value),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u64,
                                        >>::RefType::from(gas_limit),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u32 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u32,
                                        >>::RefType::from(storage_limit),
                                    ),
                                )
                                .saturating_add(::codec::Encode::size_hint(access_list))
                        }
                        Call::create {
                            ref input,
                            ref value,
                            ref gas_limit,
                            ref storage_limit,
                            ref access_list,
                        } => {
                            0_usize
                                .saturating_add(::codec::Encode::size_hint(input))
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<BalanceOf<
                                            T,
                                        > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            BalanceOf<T>,
                                        >>::RefType::from(value),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u64,
                                        >>::RefType::from(gas_limit),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u32 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u32,
                                        >>::RefType::from(storage_limit),
                                    ),
                                )
                                .saturating_add(::codec::Encode::size_hint(access_list))
                        }
                        Call::create2 {
                            ref input,
                            ref salt,
                            ref value,
                            ref gas_limit,
                            ref storage_limit,
                            ref access_list,
                        } => {
                            0_usize
                                .saturating_add(::codec::Encode::size_hint(input))
                                .saturating_add(::codec::Encode::size_hint(salt))
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<BalanceOf<
                                            T,
                                        > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            BalanceOf<T>,
                                        >>::RefType::from(value),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u64,
                                        >>::RefType::from(gas_limit),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u32 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u32,
                                        >>::RefType::from(storage_limit),
                                    ),
                                )
                                .saturating_add(::codec::Encode::size_hint(access_list))
                        }
                        Call::create_nft_contract {
                            ref input,
                            ref value,
                            ref gas_limit,
                            ref storage_limit,
                            ref access_list,
                        } => {
                            0_usize
                                .saturating_add(::codec::Encode::size_hint(input))
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<BalanceOf<
                                            T,
                                        > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            BalanceOf<T>,
                                        >>::RefType::from(value),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u64,
                                        >>::RefType::from(gas_limit),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u32 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u32,
                                        >>::RefType::from(storage_limit),
                                    ),
                                )
                                .saturating_add(::codec::Encode::size_hint(access_list))
                        }
                        Call::create_predeploy_contract {
                            ref target,
                            ref input,
                            ref value,
                            ref gas_limit,
                            ref storage_limit,
                            ref access_list,
                        } => {
                            0_usize
                                .saturating_add(::codec::Encode::size_hint(target))
                                .saturating_add(::codec::Encode::size_hint(input))
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<BalanceOf<
                                            T,
                                        > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            BalanceOf<T>,
                                        >>::RefType::from(value),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u64,
                                        >>::RefType::from(gas_limit),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u32 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u32,
                                        >>::RefType::from(storage_limit),
                                    ),
                                )
                                .saturating_add(::codec::Encode::size_hint(access_list))
                        }
                        Call::transfer_maintainer {
                            ref contract,
                            ref new_maintainer,
                        } => {
                            0_usize
                                .saturating_add(::codec::Encode::size_hint(contract))
                                .saturating_add(::codec::Encode::size_hint(new_maintainer))
                        }
                        Call::publish_contract { ref contract } => {
                            0_usize.saturating_add(::codec::Encode::size_hint(contract))
                        }
                        Call::publish_free { ref contract } => {
                            0_usize.saturating_add(::codec::Encode::size_hint(contract))
                        }
                        Call::enable_contract_development {} => 0_usize,
                        Call::disable_contract_development {} => 0_usize,
                        Call::set_code { ref contract, ref code } => {
                            0_usize
                                .saturating_add(::codec::Encode::size_hint(contract))
                                .saturating_add(::codec::Encode::size_hint(code))
                        }
                        Call::selfdestruct { ref contract } => {
                            0_usize.saturating_add(::codec::Encode::size_hint(contract))
                        }
                        Call::strict_call {
                            ref target,
                            ref input,
                            ref value,
                            ref gas_limit,
                            ref storage_limit,
                            ref access_list,
                        } => {
                            0_usize
                                .saturating_add(::codec::Encode::size_hint(target))
                                .saturating_add(::codec::Encode::size_hint(input))
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<BalanceOf<
                                            T,
                                        > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            BalanceOf<T>,
                                        >>::RefType::from(value),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u64,
                                        >>::RefType::from(gas_limit),
                                    ),
                                )
                                .saturating_add(
                                    ::codec::Encode::size_hint(
                                        &<<u32 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                            '_,
                                            u32,
                                        >>::RefType::from(storage_limit),
                                    ),
                                )
                                .saturating_add(::codec::Encode::size_hint(access_list))
                        }
                        _ => 0_usize,
                    }
            }
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                match *self {
                    Call::eth_call {
                        ref action,
                        ref input,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                        ref access_list,
                        ref valid_until,
                    } => {
                        __codec_dest_edqy.push_byte(0u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(action, __codec_dest_edqy);
                        ::codec::Encode::encode_to(input, __codec_dest_edqy);
                        {
                            ::codec::Encode::encode_to(
                                &<<BalanceOf<
                                    T,
                                > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    BalanceOf<T>,
                                >>::RefType::from(value),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u64,
                                >>::RefType::from(gas_limit),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u32 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u32,
                                >>::RefType::from(storage_limit),
                                __codec_dest_edqy,
                            );
                        }
                        ::codec::Encode::encode_to(access_list, __codec_dest_edqy);
                        {
                            ::codec::Encode::encode_to(
                                &<<T::BlockNumber as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    T::BlockNumber,
                                >>::RefType::from(valid_until),
                                __codec_dest_edqy,
                            );
                        }
                    }
                    Call::eth_call_v2 {
                        ref action,
                        ref input,
                        ref value,
                        ref gas_price,
                        ref gas_limit,
                        ref access_list,
                    } => {
                        __codec_dest_edqy.push_byte(15u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(action, __codec_dest_edqy);
                        ::codec::Encode::encode_to(input, __codec_dest_edqy);
                        {
                            ::codec::Encode::encode_to(
                                &<<BalanceOf<
                                    T,
                                > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    BalanceOf<T>,
                                >>::RefType::from(value),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u64,
                                >>::RefType::from(gas_price),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u64,
                                >>::RefType::from(gas_limit),
                                __codec_dest_edqy,
                            );
                        }
                        ::codec::Encode::encode_to(access_list, __codec_dest_edqy);
                    }
                    Call::call {
                        ref target,
                        ref input,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                        ref access_list,
                    } => {
                        __codec_dest_edqy.push_byte(1u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(target, __codec_dest_edqy);
                        ::codec::Encode::encode_to(input, __codec_dest_edqy);
                        {
                            ::codec::Encode::encode_to(
                                &<<BalanceOf<
                                    T,
                                > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    BalanceOf<T>,
                                >>::RefType::from(value),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u64,
                                >>::RefType::from(gas_limit),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u32 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u32,
                                >>::RefType::from(storage_limit),
                                __codec_dest_edqy,
                            );
                        }
                        ::codec::Encode::encode_to(access_list, __codec_dest_edqy);
                    }
                    Call::scheduled_call {
                        ref from,
                        ref target,
                        ref input,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                        ref access_list,
                    } => {
                        __codec_dest_edqy.push_byte(2u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(from, __codec_dest_edqy);
                        ::codec::Encode::encode_to(target, __codec_dest_edqy);
                        ::codec::Encode::encode_to(input, __codec_dest_edqy);
                        {
                            ::codec::Encode::encode_to(
                                &<<BalanceOf<
                                    T,
                                > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    BalanceOf<T>,
                                >>::RefType::from(value),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u64,
                                >>::RefType::from(gas_limit),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u32 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u32,
                                >>::RefType::from(storage_limit),
                                __codec_dest_edqy,
                            );
                        }
                        ::codec::Encode::encode_to(access_list, __codec_dest_edqy);
                    }
                    Call::create {
                        ref input,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                        ref access_list,
                    } => {
                        __codec_dest_edqy.push_byte(3u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(input, __codec_dest_edqy);
                        {
                            ::codec::Encode::encode_to(
                                &<<BalanceOf<
                                    T,
                                > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    BalanceOf<T>,
                                >>::RefType::from(value),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u64,
                                >>::RefType::from(gas_limit),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u32 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u32,
                                >>::RefType::from(storage_limit),
                                __codec_dest_edqy,
                            );
                        }
                        ::codec::Encode::encode_to(access_list, __codec_dest_edqy);
                    }
                    Call::create2 {
                        ref input,
                        ref salt,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                        ref access_list,
                    } => {
                        __codec_dest_edqy.push_byte(4u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(input, __codec_dest_edqy);
                        ::codec::Encode::encode_to(salt, __codec_dest_edqy);
                        {
                            ::codec::Encode::encode_to(
                                &<<BalanceOf<
                                    T,
                                > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    BalanceOf<T>,
                                >>::RefType::from(value),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u64,
                                >>::RefType::from(gas_limit),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u32 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u32,
                                >>::RefType::from(storage_limit),
                                __codec_dest_edqy,
                            );
                        }
                        ::codec::Encode::encode_to(access_list, __codec_dest_edqy);
                    }
                    Call::create_nft_contract {
                        ref input,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                        ref access_list,
                    } => {
                        __codec_dest_edqy.push_byte(5u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(input, __codec_dest_edqy);
                        {
                            ::codec::Encode::encode_to(
                                &<<BalanceOf<
                                    T,
                                > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    BalanceOf<T>,
                                >>::RefType::from(value),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u64,
                                >>::RefType::from(gas_limit),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u32 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u32,
                                >>::RefType::from(storage_limit),
                                __codec_dest_edqy,
                            );
                        }
                        ::codec::Encode::encode_to(access_list, __codec_dest_edqy);
                    }
                    Call::create_predeploy_contract {
                        ref target,
                        ref input,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                        ref access_list,
                    } => {
                        __codec_dest_edqy.push_byte(6u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(target, __codec_dest_edqy);
                        ::codec::Encode::encode_to(input, __codec_dest_edqy);
                        {
                            ::codec::Encode::encode_to(
                                &<<BalanceOf<
                                    T,
                                > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    BalanceOf<T>,
                                >>::RefType::from(value),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u64,
                                >>::RefType::from(gas_limit),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u32 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u32,
                                >>::RefType::from(storage_limit),
                                __codec_dest_edqy,
                            );
                        }
                        ::codec::Encode::encode_to(access_list, __codec_dest_edqy);
                    }
                    Call::transfer_maintainer { ref contract, ref new_maintainer } => {
                        __codec_dest_edqy.push_byte(7u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(contract, __codec_dest_edqy);
                        ::codec::Encode::encode_to(new_maintainer, __codec_dest_edqy);
                    }
                    Call::publish_contract { ref contract } => {
                        __codec_dest_edqy.push_byte(8u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(contract, __codec_dest_edqy);
                    }
                    Call::publish_free { ref contract } => {
                        __codec_dest_edqy.push_byte(9u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(contract, __codec_dest_edqy);
                    }
                    Call::enable_contract_development {} => {
                        __codec_dest_edqy.push_byte(10u8 as ::core::primitive::u8);
                    }
                    Call::disable_contract_development {} => {
                        __codec_dest_edqy.push_byte(11u8 as ::core::primitive::u8);
                    }
                    Call::set_code { ref contract, ref code } => {
                        __codec_dest_edqy.push_byte(12u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(contract, __codec_dest_edqy);
                        ::codec::Encode::encode_to(code, __codec_dest_edqy);
                    }
                    Call::selfdestruct { ref contract } => {
                        __codec_dest_edqy.push_byte(13u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(contract, __codec_dest_edqy);
                    }
                    Call::strict_call {
                        ref target,
                        ref input,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                        ref access_list,
                    } => {
                        __codec_dest_edqy.push_byte(14u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(target, __codec_dest_edqy);
                        ::codec::Encode::encode_to(input, __codec_dest_edqy);
                        {
                            ::codec::Encode::encode_to(
                                &<<BalanceOf<
                                    T,
                                > as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    BalanceOf<T>,
                                >>::RefType::from(value),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u64 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u64,
                                >>::RefType::from(gas_limit),
                                __codec_dest_edqy,
                            );
                        }
                        {
                            ::codec::Encode::encode_to(
                                &<<u32 as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                    '_,
                                    u32,
                                >>::RefType::from(storage_limit),
                                __codec_dest_edqy,
                            );
                        }
                        ::codec::Encode::encode_to(access_list, __codec_dest_edqy);
                    }
                    _ => {}
                }
            }
        }
        #[automatically_derived]
        impl<T: Config> ::codec::EncodeLike for Call<T> {}
    };
    #[allow(deprecated)]
    const _: () = {
        #[allow(non_camel_case_types)]
        #[automatically_derived]
        impl<T: Config> ::codec::Decode for Call<T> {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                match __codec_input_edqy
                    .read_byte()
                    .map_err(|e| {
                        e.chain("Could not decode `Call`, failed to read variant byte")
                    })?
                {
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy == 0u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<T>::eth_call {
                                action: {
                                    let __codec_res_edqy = <TransactionAction as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::eth_call::action`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                input: {
                                    let __codec_res_edqy = <Vec<
                                        u8,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::eth_call::input`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                value: {
                                    let __codec_res_edqy = <<BalanceOf<
                                        T,
                                    > as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::eth_call::value`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                gas_limit: {
                                    let __codec_res_edqy = <<u64 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::eth_call::gas_limit`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                storage_limit: {
                                    let __codec_res_edqy = <<u32 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::eth_call::storage_limit`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                access_list: {
                                    let __codec_res_edqy = <Vec<
                                        AccessListItem,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::eth_call::access_list`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                valid_until: {
                                    let __codec_res_edqy = <<T::BlockNumber as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::eth_call::valid_until`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 15u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<T>::eth_call_v2 {
                                action: {
                                    let __codec_res_edqy = <TransactionAction as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::eth_call_v2::action`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                input: {
                                    let __codec_res_edqy = <Vec<
                                        u8,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::eth_call_v2::input`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                value: {
                                    let __codec_res_edqy = <<BalanceOf<
                                        T,
                                    > as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::eth_call_v2::value`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                gas_price: {
                                    let __codec_res_edqy = <<u64 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::eth_call_v2::gas_price`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                gas_limit: {
                                    let __codec_res_edqy = <<u64 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::eth_call_v2::gas_limit`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                access_list: {
                                    let __codec_res_edqy = <Vec<
                                        AccessListItem,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::eth_call_v2::access_list`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy == 1u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<T>::call {
                                target: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::call::target`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                input: {
                                    let __codec_res_edqy = <Vec<
                                        u8,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::call::input`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                value: {
                                    let __codec_res_edqy = <<BalanceOf<
                                        T,
                                    > as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::call::value`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                gas_limit: {
                                    let __codec_res_edqy = <<u64 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::call::gas_limit`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                storage_limit: {
                                    let __codec_res_edqy = <<u32 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::call::storage_limit`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                access_list: {
                                    let __codec_res_edqy = <Vec<
                                        AccessListItem,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::call::access_list`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy == 2u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<T>::scheduled_call {
                                from: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::scheduled_call::from`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                target: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::scheduled_call::target`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                input: {
                                    let __codec_res_edqy = <Vec<
                                        u8,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::scheduled_call::input`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                value: {
                                    let __codec_res_edqy = <<BalanceOf<
                                        T,
                                    > as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::scheduled_call::value`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                gas_limit: {
                                    let __codec_res_edqy = <<u64 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain("Could not decode `Call::scheduled_call::gas_limit`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                storage_limit: {
                                    let __codec_res_edqy = <<u32 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::scheduled_call::storage_limit`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                access_list: {
                                    let __codec_res_edqy = <Vec<
                                        AccessListItem,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::scheduled_call::access_list`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy == 3u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<T>::create {
                                input: {
                                    let __codec_res_edqy = <Vec<
                                        u8,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::create::input`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                value: {
                                    let __codec_res_edqy = <<BalanceOf<
                                        T,
                                    > as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::create::value`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                gas_limit: {
                                    let __codec_res_edqy = <<u64 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::create::gas_limit`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                storage_limit: {
                                    let __codec_res_edqy = <<u32 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::create::storage_limit`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                access_list: {
                                    let __codec_res_edqy = <Vec<
                                        AccessListItem,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::create::access_list`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy == 4u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<T>::create2 {
                                input: {
                                    let __codec_res_edqy = <Vec<
                                        u8,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::create2::input`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                salt: {
                                    let __codec_res_edqy = <H256 as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::create2::salt`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                value: {
                                    let __codec_res_edqy = <<BalanceOf<
                                        T,
                                    > as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::create2::value`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                gas_limit: {
                                    let __codec_res_edqy = <<u64 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::create2::gas_limit`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                storage_limit: {
                                    let __codec_res_edqy = <<u32 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::create2::storage_limit`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                access_list: {
                                    let __codec_res_edqy = <Vec<
                                        AccessListItem,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::create2::access_list`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy == 5u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<T>::create_nft_contract {
                                input: {
                                    let __codec_res_edqy = <Vec<
                                        u8,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::create_nft_contract::input`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                value: {
                                    let __codec_res_edqy = <<BalanceOf<
                                        T,
                                    > as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::create_nft_contract::value`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                gas_limit: {
                                    let __codec_res_edqy = <<u64 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::create_nft_contract::gas_limit`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                storage_limit: {
                                    let __codec_res_edqy = <<u32 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::create_nft_contract::storage_limit`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                access_list: {
                                    let __codec_res_edqy = <Vec<
                                        AccessListItem,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::create_nft_contract::access_list`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy == 6u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<
                                T,
                            >::create_predeploy_contract {
                                target: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::create_predeploy_contract::target`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                input: {
                                    let __codec_res_edqy = <Vec<
                                        u8,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::create_predeploy_contract::input`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                value: {
                                    let __codec_res_edqy = <<BalanceOf<
                                        T,
                                    > as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::create_predeploy_contract::value`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                gas_limit: {
                                    let __codec_res_edqy = <<u64 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::create_predeploy_contract::gas_limit`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                storage_limit: {
                                    let __codec_res_edqy = <<u32 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::create_predeploy_contract::storage_limit`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                access_list: {
                                    let __codec_res_edqy = <Vec<
                                        AccessListItem,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::create_predeploy_contract::access_list`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy == 7u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<T>::transfer_maintainer {
                                contract: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::transfer_maintainer::contract`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                new_maintainer: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::transfer_maintainer::new_maintainer`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy == 8u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<T>::publish_contract {
                                contract: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::publish_contract::contract`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy == 9u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<T>::publish_free {
                                contract: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::publish_free::contract`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 10u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<
                                T,
                            >::enable_contract_development {})
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 11u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<
                                T,
                            >::disable_contract_development {})
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 12u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<T>::set_code {
                                contract: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::set_code::contract`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                code: {
                                    let __codec_res_edqy = <Vec<
                                        u8,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::set_code::code`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 13u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<T>::selfdestruct {
                                contract: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::selfdestruct::contract`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    #[allow(clippy::unnecessary_cast)]
                    __codec_x_edqy if __codec_x_edqy
                        == 14u8 as ::core::primitive::u8 => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Ok(Call::<T>::strict_call {
                                target: {
                                    let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::strict_call::target`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                input: {
                                    let __codec_res_edqy = <Vec<
                                        u8,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::strict_call::input`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                                value: {
                                    let __codec_res_edqy = <<BalanceOf<
                                        T,
                                    > as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::strict_call::value`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                gas_limit: {
                                    let __codec_res_edqy = <<u64 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::strict_call::gas_limit`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                storage_limit: {
                                    let __codec_res_edqy = <<u32 as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                        __codec_input_edqy,
                                    );
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e
                                                    .chain(
                                                        "Could not decode `Call::strict_call::storage_limit`",
                                                    ),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy.into()
                                        }
                                    }
                                },
                                access_list: {
                                    let __codec_res_edqy = <Vec<
                                        AccessListItem,
                                    > as ::codec::Decode>::decode(__codec_input_edqy);
                                    match __codec_res_edqy {
                                        ::core::result::Result::Err(e) => {
                                            return ::core::result::Result::Err(
                                                e.chain("Could not decode `Call::strict_call::access_list`"),
                                            );
                                        }
                                        ::core::result::Result::Ok(__codec_res_edqy) => {
                                            __codec_res_edqy
                                        }
                                    }
                                },
                            })
                        })();
                    }
                    _ => {
                        #[allow(clippy::redundant_closure_call)]
                        return (move || {
                            ::core::result::Result::Err(
                                <_ as ::core::convert::Into<
                                    _,
                                >>::into("Could not decode `Call`, variant doesn't exist"),
                            )
                        })();
                    }
                }
            }
        }
    };
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl<T: Config> ::scale_info::TypeInfo for Call<T>
        where
            frame_support::sp_std::marker::PhantomData<
                (T,),
            >: ::scale_info::TypeInfo + 'static,
            BalanceOf<T>: ::scale_info::scale::HasCompact,
            T::BlockNumber: ::scale_info::scale::HasCompact,
            BalanceOf<T>: ::scale_info::scale::HasCompact,
            BalanceOf<T>: ::scale_info::scale::HasCompact,
            BalanceOf<T>: ::scale_info::scale::HasCompact,
            BalanceOf<T>: ::scale_info::scale::HasCompact,
            BalanceOf<T>: ::scale_info::scale::HasCompact,
            BalanceOf<T>: ::scale_info::scale::HasCompact,
            BalanceOf<T>: ::scale_info::scale::HasCompact,
            BalanceOf<T>: ::scale_info::scale::HasCompact,
            T: Config + 'static,
        {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                ::scale_info::Type::builder()
                    .path(::scale_info::Path::new("Call", "module_evm::module"))
                    .type_params(
                        <[_]>::into_vec(
                            #[rustc_box]
                            ::alloc::boxed::Box::new([
                                ::scale_info::TypeParameter::new(
                                    "T",
                                    ::core::option::Option::None,
                                ),
                            ]),
                        ),
                    )
                    .docs_always(
                        &[
                            "Contains a variant per dispatchable extrinsic that this pallet has.",
                        ],
                    )
                    .variant(
                        ::scale_info::build::Variants::new()
                            .variant(
                                "eth_call",
                                |v| {
                                    v
                                        .index(0u8 as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f
                                                        .ty::<TransactionAction>()
                                                        .name("action")
                                                        .type_name("TransactionAction")
                                                })
                                                .field(|f| {
                                                    f.ty::<Vec<u8>>().name("input").type_name("Vec<u8>")
                                                })
                                                .field(|f| {
                                                    f
                                                        .compact::<BalanceOf<T>>()
                                                        .name("value")
                                                        .type_name("BalanceOf<T>")
                                                })
                                                .field(|f| {
                                                    f.compact::<u64>().name("gas_limit").type_name("u64")
                                                })
                                                .field(|f| {
                                                    f.compact::<u32>().name("storage_limit").type_name("u32")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<Vec<AccessListItem>>()
                                                        .name("access_list")
                                                        .type_name("Vec<AccessListItem>")
                                                })
                                                .field(|f| {
                                                    f
                                                        .compact::<T::BlockNumber>()
                                                        .name("valid_until")
                                                        .type_name("T::BlockNumber")
                                                }),
                                        )
                                        .docs_always(&["See [`Pallet::eth_call`]."])
                                },
                            )
                            .variant(
                                "eth_call_v2",
                                |v| {
                                    v
                                        .index(15u8 as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f
                                                        .ty::<TransactionAction>()
                                                        .name("action")
                                                        .type_name("TransactionAction")
                                                })
                                                .field(|f| {
                                                    f.ty::<Vec<u8>>().name("input").type_name("Vec<u8>")
                                                })
                                                .field(|f| {
                                                    f
                                                        .compact::<BalanceOf<T>>()
                                                        .name("value")
                                                        .type_name("BalanceOf<T>")
                                                })
                                                .field(|f| {
                                                    f.compact::<u64>().name("gas_price").type_name("u64")
                                                })
                                                .field(|f| {
                                                    f.compact::<u64>().name("gas_limit").type_name("u64")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<Vec<AccessListItem>>()
                                                        .name("access_list")
                                                        .type_name("Vec<AccessListItem>")
                                                }),
                                        )
                                        .docs_always(&["See [`Pallet::eth_call_v2`]."])
                                },
                            )
                            .variant(
                                "call",
                                |v| {
                                    v
                                        .index(1u8 as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f.ty::<EvmAddress>().name("target").type_name("EvmAddress")
                                                })
                                                .field(|f| {
                                                    f.ty::<Vec<u8>>().name("input").type_name("Vec<u8>")
                                                })
                                                .field(|f| {
                                                    f
                                                        .compact::<BalanceOf<T>>()
                                                        .name("value")
                                                        .type_name("BalanceOf<T>")
                                                })
                                                .field(|f| {
                                                    f.compact::<u64>().name("gas_limit").type_name("u64")
                                                })
                                                .field(|f| {
                                                    f.compact::<u32>().name("storage_limit").type_name("u32")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<Vec<AccessListItem>>()
                                                        .name("access_list")
                                                        .type_name("Vec<AccessListItem>")
                                                }),
                                        )
                                        .docs_always(&["See [`Pallet::call`]."])
                                },
                            )
                            .variant(
                                "scheduled_call",
                                |v| {
                                    v
                                        .index(2u8 as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f.ty::<EvmAddress>().name("from").type_name("EvmAddress")
                                                })
                                                .field(|f| {
                                                    f.ty::<EvmAddress>().name("target").type_name("EvmAddress")
                                                })
                                                .field(|f| {
                                                    f.ty::<Vec<u8>>().name("input").type_name("Vec<u8>")
                                                })
                                                .field(|f| {
                                                    f
                                                        .compact::<BalanceOf<T>>()
                                                        .name("value")
                                                        .type_name("BalanceOf<T>")
                                                })
                                                .field(|f| {
                                                    f.compact::<u64>().name("gas_limit").type_name("u64")
                                                })
                                                .field(|f| {
                                                    f.compact::<u32>().name("storage_limit").type_name("u32")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<Vec<AccessListItem>>()
                                                        .name("access_list")
                                                        .type_name("Vec<AccessListItem>")
                                                }),
                                        )
                                        .docs_always(&["See [`Pallet::scheduled_call`]."])
                                },
                            )
                            .variant(
                                "create",
                                |v| {
                                    v
                                        .index(3u8 as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f.ty::<Vec<u8>>().name("input").type_name("Vec<u8>")
                                                })
                                                .field(|f| {
                                                    f
                                                        .compact::<BalanceOf<T>>()
                                                        .name("value")
                                                        .type_name("BalanceOf<T>")
                                                })
                                                .field(|f| {
                                                    f.compact::<u64>().name("gas_limit").type_name("u64")
                                                })
                                                .field(|f| {
                                                    f.compact::<u32>().name("storage_limit").type_name("u32")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<Vec<AccessListItem>>()
                                                        .name("access_list")
                                                        .type_name("Vec<AccessListItem>")
                                                }),
                                        )
                                        .docs_always(&["See [`Pallet::create`]."])
                                },
                            )
                            .variant(
                                "create2",
                                |v| {
                                    v
                                        .index(4u8 as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f.ty::<Vec<u8>>().name("input").type_name("Vec<u8>")
                                                })
                                                .field(|f| f.ty::<H256>().name("salt").type_name("H256"))
                                                .field(|f| {
                                                    f
                                                        .compact::<BalanceOf<T>>()
                                                        .name("value")
                                                        .type_name("BalanceOf<T>")
                                                })
                                                .field(|f| {
                                                    f.compact::<u64>().name("gas_limit").type_name("u64")
                                                })
                                                .field(|f| {
                                                    f.compact::<u32>().name("storage_limit").type_name("u32")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<Vec<AccessListItem>>()
                                                        .name("access_list")
                                                        .type_name("Vec<AccessListItem>")
                                                }),
                                        )
                                        .docs_always(&["See [`Pallet::create2`]."])
                                },
                            )
                            .variant(
                                "create_nft_contract",
                                |v| {
                                    v
                                        .index(5u8 as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f.ty::<Vec<u8>>().name("input").type_name("Vec<u8>")
                                                })
                                                .field(|f| {
                                                    f
                                                        .compact::<BalanceOf<T>>()
                                                        .name("value")
                                                        .type_name("BalanceOf<T>")
                                                })
                                                .field(|f| {
                                                    f.compact::<u64>().name("gas_limit").type_name("u64")
                                                })
                                                .field(|f| {
                                                    f.compact::<u32>().name("storage_limit").type_name("u32")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<Vec<AccessListItem>>()
                                                        .name("access_list")
                                                        .type_name("Vec<AccessListItem>")
                                                }),
                                        )
                                        .docs_always(&["See [`Pallet::create_nft_contract`]."])
                                },
                            )
                            .variant(
                                "create_predeploy_contract",
                                |v| {
                                    v
                                        .index(6u8 as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f.ty::<EvmAddress>().name("target").type_name("EvmAddress")
                                                })
                                                .field(|f| {
                                                    f.ty::<Vec<u8>>().name("input").type_name("Vec<u8>")
                                                })
                                                .field(|f| {
                                                    f
                                                        .compact::<BalanceOf<T>>()
                                                        .name("value")
                                                        .type_name("BalanceOf<T>")
                                                })
                                                .field(|f| {
                                                    f.compact::<u64>().name("gas_limit").type_name("u64")
                                                })
                                                .field(|f| {
                                                    f.compact::<u32>().name("storage_limit").type_name("u32")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<Vec<AccessListItem>>()
                                                        .name("access_list")
                                                        .type_name("Vec<AccessListItem>")
                                                }),
                                        )
                                        .docs_always(
                                            &["See [`Pallet::create_predeploy_contract`]."],
                                        )
                                },
                            )
                            .variant(
                                "transfer_maintainer",
                                |v| {
                                    v
                                        .index(7u8 as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f
                                                        .ty::<EvmAddress>()
                                                        .name("contract")
                                                        .type_name("EvmAddress")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<EvmAddress>()
                                                        .name("new_maintainer")
                                                        .type_name("EvmAddress")
                                                }),
                                        )
                                        .docs_always(&["See [`Pallet::transfer_maintainer`]."])
                                },
                            )
                            .variant(
                                "publish_contract",
                                |v| {
                                    v
                                        .index(8u8 as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f
                                                        .ty::<EvmAddress>()
                                                        .name("contract")
                                                        .type_name("EvmAddress")
                                                }),
                                        )
                                        .docs_always(&["See [`Pallet::publish_contract`]."])
                                },
                            )
                            .variant(
                                "publish_free",
                                |v| {
                                    v
                                        .index(9u8 as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f
                                                        .ty::<EvmAddress>()
                                                        .name("contract")
                                                        .type_name("EvmAddress")
                                                }),
                                        )
                                        .docs_always(&["See [`Pallet::publish_free`]."])
                                },
                            )
                            .variant(
                                "enable_contract_development",
                                |v| {
                                    v
                                        .index(10u8 as ::core::primitive::u8)
                                        .fields(::scale_info::build::Fields::named())
                                        .docs_always(
                                            &["See [`Pallet::enable_contract_development`]."],
                                        )
                                },
                            )
                            .variant(
                                "disable_contract_development",
                                |v| {
                                    v
                                        .index(11u8 as ::core::primitive::u8)
                                        .fields(::scale_info::build::Fields::named())
                                        .docs_always(
                                            &["See [`Pallet::disable_contract_development`]."],
                                        )
                                },
                            )
                            .variant(
                                "set_code",
                                |v| {
                                    v
                                        .index(12u8 as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f
                                                        .ty::<EvmAddress>()
                                                        .name("contract")
                                                        .type_name("EvmAddress")
                                                })
                                                .field(|f| {
                                                    f.ty::<Vec<u8>>().name("code").type_name("Vec<u8>")
                                                }),
                                        )
                                        .docs_always(&["See [`Pallet::set_code`]."])
                                },
                            )
                            .variant(
                                "selfdestruct",
                                |v| {
                                    v
                                        .index(13u8 as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f
                                                        .ty::<EvmAddress>()
                                                        .name("contract")
                                                        .type_name("EvmAddress")
                                                }),
                                        )
                                        .docs_always(&["See [`Pallet::selfdestruct`]."])
                                },
                            )
                            .variant(
                                "strict_call",
                                |v| {
                                    v
                                        .index(14u8 as ::core::primitive::u8)
                                        .fields(
                                            ::scale_info::build::Fields::named()
                                                .field(|f| {
                                                    f.ty::<EvmAddress>().name("target").type_name("EvmAddress")
                                                })
                                                .field(|f| {
                                                    f.ty::<Vec<u8>>().name("input").type_name("Vec<u8>")
                                                })
                                                .field(|f| {
                                                    f
                                                        .compact::<BalanceOf<T>>()
                                                        .name("value")
                                                        .type_name("BalanceOf<T>")
                                                })
                                                .field(|f| {
                                                    f.compact::<u64>().name("gas_limit").type_name("u64")
                                                })
                                                .field(|f| {
                                                    f.compact::<u32>().name("storage_limit").type_name("u32")
                                                })
                                                .field(|f| {
                                                    f
                                                        .ty::<Vec<AccessListItem>>()
                                                        .name("access_list")
                                                        .type_name("Vec<AccessListItem>")
                                                }),
                                        )
                                        .docs_always(&["See [`Pallet::strict_call`]."])
                                },
                            ),
                    )
            }
        }
    };
    impl<T: Config> Call<T> {
        ///Create a call with the variant `eth_call`.
        pub fn new_call_variant_eth_call(
            action: TransactionAction,
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<AccessListItem>,
            valid_until: T::BlockNumber,
        ) -> Self {
            Self::eth_call {
                action,
                input,
                value,
                gas_limit,
                storage_limit,
                access_list,
                valid_until,
            }
        }
        ///Create a call with the variant `eth_call_v2`.
        pub fn new_call_variant_eth_call_v2(
            action: TransactionAction,
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_price: u64,
            gas_limit: u64,
            access_list: Vec<AccessListItem>,
        ) -> Self {
            Self::eth_call_v2 {
                action,
                input,
                value,
                gas_price,
                gas_limit,
                access_list,
            }
        }
        ///Create a call with the variant `call`.
        pub fn new_call_variant_call(
            target: EvmAddress,
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<AccessListItem>,
        ) -> Self {
            Self::call {
                target,
                input,
                value,
                gas_limit,
                storage_limit,
                access_list,
            }
        }
        ///Create a call with the variant `scheduled_call`.
        pub fn new_call_variant_scheduled_call(
            from: EvmAddress,
            target: EvmAddress,
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<AccessListItem>,
        ) -> Self {
            Self::scheduled_call {
                from,
                target,
                input,
                value,
                gas_limit,
                storage_limit,
                access_list,
            }
        }
        ///Create a call with the variant `create`.
        pub fn new_call_variant_create(
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<AccessListItem>,
        ) -> Self {
            Self::create {
                input,
                value,
                gas_limit,
                storage_limit,
                access_list,
            }
        }
        ///Create a call with the variant `create2`.
        pub fn new_call_variant_create2(
            input: Vec<u8>,
            salt: H256,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<AccessListItem>,
        ) -> Self {
            Self::create2 {
                input,
                salt,
                value,
                gas_limit,
                storage_limit,
                access_list,
            }
        }
        ///Create a call with the variant `create_nft_contract`.
        pub fn new_call_variant_create_nft_contract(
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<AccessListItem>,
        ) -> Self {
            Self::create_nft_contract {
                input,
                value,
                gas_limit,
                storage_limit,
                access_list,
            }
        }
        ///Create a call with the variant `create_predeploy_contract`.
        pub fn new_call_variant_create_predeploy_contract(
            target: EvmAddress,
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<AccessListItem>,
        ) -> Self {
            Self::create_predeploy_contract {
                target,
                input,
                value,
                gas_limit,
                storage_limit,
                access_list,
            }
        }
        ///Create a call with the variant `transfer_maintainer`.
        pub fn new_call_variant_transfer_maintainer(
            contract: EvmAddress,
            new_maintainer: EvmAddress,
        ) -> Self {
            Self::transfer_maintainer {
                contract,
                new_maintainer,
            }
        }
        ///Create a call with the variant `publish_contract`.
        pub fn new_call_variant_publish_contract(contract: EvmAddress) -> Self {
            Self::publish_contract { contract }
        }
        ///Create a call with the variant `publish_free`.
        pub fn new_call_variant_publish_free(contract: EvmAddress) -> Self {
            Self::publish_free { contract }
        }
        ///Create a call with the variant `enable_contract_development`.
        pub fn new_call_variant_enable_contract_development() -> Self {
            Self::enable_contract_development {
            }
        }
        ///Create a call with the variant `disable_contract_development`.
        pub fn new_call_variant_disable_contract_development() -> Self {
            Self::disable_contract_development {
            }
        }
        ///Create a call with the variant `set_code`.
        pub fn new_call_variant_set_code(contract: EvmAddress, code: Vec<u8>) -> Self {
            Self::set_code { contract, code }
        }
        ///Create a call with the variant `selfdestruct`.
        pub fn new_call_variant_selfdestruct(contract: EvmAddress) -> Self {
            Self::selfdestruct { contract }
        }
        ///Create a call with the variant `strict_call`.
        pub fn new_call_variant_strict_call(
            target: EvmAddress,
            input: Vec<u8>,
            value: BalanceOf<T>,
            gas_limit: u64,
            storage_limit: u32,
            access_list: Vec<AccessListItem>,
        ) -> Self {
            Self::strict_call {
                target,
                input,
                value,
                gas_limit,
                storage_limit,
                access_list,
            }
        }
    }
    impl<T: Config> frame_support::dispatch::GetDispatchInfo for Call<T> {
        fn get_dispatch_info(&self) -> frame_support::dispatch::DispatchInfo {
            match *self {
                Self::eth_call {
                    ref action,
                    ref input,
                    ref value,
                    ref gas_limit,
                    ref storage_limit,
                    ref access_list,
                    valid_until: ref _valid_until,
                } => {
                    let __pallet_base_weight = match *action {
                        TransactionAction::Call(_) => call_weight::<T>(*gas_limit),
                        TransactionAction::Create => create_weight::<T>(*gas_limit),
                    };
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<
                        (
                            &TransactionAction,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                            &T::BlockNumber,
                        ),
                    >>::weigh_data(
                        &__pallet_base_weight,
                        (
                            action,
                            input,
                            value,
                            gas_limit,
                            storage_limit,
                            access_list,
                            _valid_until,
                        ),
                    );
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<
                        (
                            &TransactionAction,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                            &T::BlockNumber,
                        ),
                    >>::classify_dispatch(
                        &__pallet_base_weight,
                        (
                            action,
                            input,
                            value,
                            gas_limit,
                            storage_limit,
                            access_list,
                            _valid_until,
                        ),
                    );
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<
                        (
                            &TransactionAction,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                            &T::BlockNumber,
                        ),
                    >>::pays_fee(
                        &__pallet_base_weight,
                        (
                            action,
                            input,
                            value,
                            gas_limit,
                            storage_limit,
                            access_list,
                            _valid_until,
                        ),
                    );
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::eth_call_v2 {
                    ref action,
                    ref input,
                    ref value,
                    gas_price: ref _gas_price,
                    ref gas_limit,
                    ref access_list,
                } => {
                    let __pallet_base_weight = match *action {
                        TransactionAction::Call(_) => {
                            call_weight::<T>(decode_gas_limit(*gas_limit).0)
                        }
                        TransactionAction::Create => {
                            create_weight::<T>(decode_gas_limit(*gas_limit).0)
                        }
                    };
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<
                        (
                            &TransactionAction,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u64,
                            &Vec<AccessListItem>,
                        ),
                    >>::weigh_data(
                        &__pallet_base_weight,
                        (action, input, value, _gas_price, gas_limit, access_list),
                    );
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<
                        (
                            &TransactionAction,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u64,
                            &Vec<AccessListItem>,
                        ),
                    >>::classify_dispatch(
                        &__pallet_base_weight,
                        (action, input, value, _gas_price, gas_limit, access_list),
                    );
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<
                        (
                            &TransactionAction,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u64,
                            &Vec<AccessListItem>,
                        ),
                    >>::pays_fee(
                        &__pallet_base_weight,
                        (action, input, value, _gas_price, gas_limit, access_list),
                    );
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::call {
                    ref target,
                    ref input,
                    ref value,
                    ref gas_limit,
                    ref storage_limit,
                    ref access_list,
                } => {
                    let __pallet_base_weight = call_weight::<T>(*gas_limit);
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<
                        (
                            &EvmAddress,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                        ),
                    >>::weigh_data(
                        &__pallet_base_weight,
                        (target, input, value, gas_limit, storage_limit, access_list),
                    );
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<
                        (
                            &EvmAddress,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                        ),
                    >>::classify_dispatch(
                        &__pallet_base_weight,
                        (target, input, value, gas_limit, storage_limit, access_list),
                    );
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<
                        (
                            &EvmAddress,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                        ),
                    >>::pays_fee(
                        &__pallet_base_weight,
                        (target, input, value, gas_limit, storage_limit, access_list),
                    );
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::scheduled_call {
                    ref from,
                    ref target,
                    ref input,
                    ref value,
                    ref gas_limit,
                    ref storage_limit,
                    ref access_list,
                } => {
                    let __pallet_base_weight = T::GasToWeight::convert(*gas_limit);
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<
                        (
                            &EvmAddress,
                            &EvmAddress,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                        ),
                    >>::weigh_data(
                        &__pallet_base_weight,
                        (
                            from,
                            target,
                            input,
                            value,
                            gas_limit,
                            storage_limit,
                            access_list,
                        ),
                    );
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<
                        (
                            &EvmAddress,
                            &EvmAddress,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                        ),
                    >>::classify_dispatch(
                        &__pallet_base_weight,
                        (
                            from,
                            target,
                            input,
                            value,
                            gas_limit,
                            storage_limit,
                            access_list,
                        ),
                    );
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<
                        (
                            &EvmAddress,
                            &EvmAddress,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                        ),
                    >>::pays_fee(
                        &__pallet_base_weight,
                        (
                            from,
                            target,
                            input,
                            value,
                            gas_limit,
                            storage_limit,
                            access_list,
                        ),
                    );
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::create {
                    ref input,
                    ref value,
                    ref gas_limit,
                    ref storage_limit,
                    ref access_list,
                } => {
                    let __pallet_base_weight = create_weight::<T>(*gas_limit);
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<
                        (&Vec<u8>, &BalanceOf<T>, &u64, &u32, &Vec<AccessListItem>),
                    >>::weigh_data(
                        &__pallet_base_weight,
                        (input, value, gas_limit, storage_limit, access_list),
                    );
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<
                        (&Vec<u8>, &BalanceOf<T>, &u64, &u32, &Vec<AccessListItem>),
                    >>::classify_dispatch(
                        &__pallet_base_weight,
                        (input, value, gas_limit, storage_limit, access_list),
                    );
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<
                        (&Vec<u8>, &BalanceOf<T>, &u64, &u32, &Vec<AccessListItem>),
                    >>::pays_fee(
                        &__pallet_base_weight,
                        (input, value, gas_limit, storage_limit, access_list),
                    );
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::create2 {
                    ref input,
                    ref salt,
                    ref value,
                    ref gas_limit,
                    ref storage_limit,
                    ref access_list,
                } => {
                    let __pallet_base_weight = create2_weight::<T>(*gas_limit);
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<
                        (
                            &Vec<u8>,
                            &H256,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                        ),
                    >>::weigh_data(
                        &__pallet_base_weight,
                        (input, salt, value, gas_limit, storage_limit, access_list),
                    );
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<
                        (
                            &Vec<u8>,
                            &H256,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                        ),
                    >>::classify_dispatch(
                        &__pallet_base_weight,
                        (input, salt, value, gas_limit, storage_limit, access_list),
                    );
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<
                        (
                            &Vec<u8>,
                            &H256,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                        ),
                    >>::pays_fee(
                        &__pallet_base_weight,
                        (input, salt, value, gas_limit, storage_limit, access_list),
                    );
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::create_nft_contract {
                    ref input,
                    ref value,
                    ref gas_limit,
                    ref storage_limit,
                    ref access_list,
                } => {
                    let __pallet_base_weight = create_nft_contract::<T>(*gas_limit);
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<
                        (&Vec<u8>, &BalanceOf<T>, &u64, &u32, &Vec<AccessListItem>),
                    >>::weigh_data(
                        &__pallet_base_weight,
                        (input, value, gas_limit, storage_limit, access_list),
                    );
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<
                        (&Vec<u8>, &BalanceOf<T>, &u64, &u32, &Vec<AccessListItem>),
                    >>::classify_dispatch(
                        &__pallet_base_weight,
                        (input, value, gas_limit, storage_limit, access_list),
                    );
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<
                        (&Vec<u8>, &BalanceOf<T>, &u64, &u32, &Vec<AccessListItem>),
                    >>::pays_fee(
                        &__pallet_base_weight,
                        (input, value, gas_limit, storage_limit, access_list),
                    );
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::create_predeploy_contract {
                    ref target,
                    ref input,
                    ref value,
                    ref gas_limit,
                    ref storage_limit,
                    ref access_list,
                } => {
                    let __pallet_base_weight = create_predeploy_contract::<
                        T,
                    >(*gas_limit);
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<
                        (
                            &EvmAddress,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                        ),
                    >>::weigh_data(
                        &__pallet_base_weight,
                        (target, input, value, gas_limit, storage_limit, access_list),
                    );
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<
                        (
                            &EvmAddress,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                        ),
                    >>::classify_dispatch(
                        &__pallet_base_weight,
                        (target, input, value, gas_limit, storage_limit, access_list),
                    );
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<
                        (
                            &EvmAddress,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                        ),
                    >>::pays_fee(
                        &__pallet_base_weight,
                        (target, input, value, gas_limit, storage_limit, access_list),
                    );
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::transfer_maintainer { ref contract, ref new_maintainer } => {
                    let __pallet_base_weight = <T as Config>::WeightInfo::transfer_maintainer();
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<
                        (&EvmAddress, &EvmAddress),
                    >>::weigh_data(&__pallet_base_weight, (contract, new_maintainer));
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<
                        (&EvmAddress, &EvmAddress),
                    >>::classify_dispatch(
                        &__pallet_base_weight,
                        (contract, new_maintainer),
                    );
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<
                        (&EvmAddress, &EvmAddress),
                    >>::pays_fee(&__pallet_base_weight, (contract, new_maintainer));
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::publish_contract { ref contract } => {
                    let __pallet_base_weight = <T as Config>::WeightInfo::publish_contract();
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<
                        (&EvmAddress,),
                    >>::weigh_data(&__pallet_base_weight, (contract,));
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<
                        (&EvmAddress,),
                    >>::classify_dispatch(&__pallet_base_weight, (contract,));
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<
                        (&EvmAddress,),
                    >>::pays_fee(&__pallet_base_weight, (contract,));
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::publish_free { ref contract } => {
                    let __pallet_base_weight = <T as Config>::WeightInfo::publish_free();
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<
                        (&EvmAddress,),
                    >>::weigh_data(&__pallet_base_weight, (contract,));
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<
                        (&EvmAddress,),
                    >>::classify_dispatch(&__pallet_base_weight, (contract,));
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<
                        (&EvmAddress,),
                    >>::pays_fee(&__pallet_base_weight, (contract,));
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::enable_contract_development {} => {
                    let __pallet_base_weight = <T as Config>::WeightInfo::enable_contract_development();
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<
                        (),
                    >>::weigh_data(&__pallet_base_weight, ());
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<
                        (),
                    >>::classify_dispatch(&__pallet_base_weight, ());
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<
                        (),
                    >>::pays_fee(&__pallet_base_weight, ());
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::disable_contract_development {} => {
                    let __pallet_base_weight = <T as Config>::WeightInfo::disable_contract_development();
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<
                        (),
                    >>::weigh_data(&__pallet_base_weight, ());
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<
                        (),
                    >>::classify_dispatch(&__pallet_base_weight, ());
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<
                        (),
                    >>::pays_fee(&__pallet_base_weight, ());
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::set_code { ref contract, ref code } => {
                    let __pallet_base_weight = <T as Config>::WeightInfo::set_code(
                        code.len() as u32,
                    );
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<
                        (&EvmAddress, &Vec<u8>),
                    >>::weigh_data(&__pallet_base_weight, (contract, code));
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<
                        (&EvmAddress, &Vec<u8>),
                    >>::classify_dispatch(&__pallet_base_weight, (contract, code));
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<
                        (&EvmAddress, &Vec<u8>),
                    >>::pays_fee(&__pallet_base_weight, (contract, code));
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::selfdestruct { ref contract } => {
                    let __pallet_base_weight = <T as Config>::WeightInfo::selfdestruct();
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<
                        (&EvmAddress,),
                    >>::weigh_data(&__pallet_base_weight, (contract,));
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<
                        (&EvmAddress,),
                    >>::classify_dispatch(&__pallet_base_weight, (contract,));
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<
                        (&EvmAddress,),
                    >>::pays_fee(&__pallet_base_weight, (contract,));
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::strict_call {
                    ref target,
                    ref input,
                    ref value,
                    ref gas_limit,
                    ref storage_limit,
                    ref access_list,
                } => {
                    let __pallet_base_weight = call_weight::<T>(*gas_limit);
                    let __pallet_weight = <dyn frame_support::dispatch::WeighData<
                        (
                            &EvmAddress,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                        ),
                    >>::weigh_data(
                        &__pallet_base_weight,
                        (target, input, value, gas_limit, storage_limit, access_list),
                    );
                    let __pallet_class = <dyn frame_support::dispatch::ClassifyDispatch<
                        (
                            &EvmAddress,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                        ),
                    >>::classify_dispatch(
                        &__pallet_base_weight,
                        (target, input, value, gas_limit, storage_limit, access_list),
                    );
                    let __pallet_pays_fee = <dyn frame_support::dispatch::PaysFee<
                        (
                            &EvmAddress,
                            &Vec<u8>,
                            &BalanceOf<T>,
                            &u64,
                            &u32,
                            &Vec<AccessListItem>,
                        ),
                    >>::pays_fee(
                        &__pallet_base_weight,
                        (target, input, value, gas_limit, storage_limit, access_list),
                    );
                    frame_support::dispatch::DispatchInfo {
                        weight: __pallet_weight,
                        class: __pallet_class,
                        pays_fee: __pallet_pays_fee,
                    }
                }
                Self::__Ignore(_, _) => {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "internal error: entered unreachable code: {0}",
                            format_args!("__Ignore cannot be used")
                        ),
                    )
                }
            }
        }
    }
    #[allow(deprecated)]
    impl<T: Config> frame_support::weights::GetDispatchInfo for Call<T> {}
    impl<T: Config> frame_support::dispatch::GetCallName for Call<T> {
        fn get_call_name(&self) -> &'static str {
            match *self {
                Self::eth_call { .. } => "eth_call",
                Self::eth_call_v2 { .. } => "eth_call_v2",
                Self::call { .. } => "call",
                Self::scheduled_call { .. } => "scheduled_call",
                Self::create { .. } => "create",
                Self::create2 { .. } => "create2",
                Self::create_nft_contract { .. } => "create_nft_contract",
                Self::create_predeploy_contract { .. } => "create_predeploy_contract",
                Self::transfer_maintainer { .. } => "transfer_maintainer",
                Self::publish_contract { .. } => "publish_contract",
                Self::publish_free { .. } => "publish_free",
                Self::enable_contract_development { .. } => "enable_contract_development",
                Self::disable_contract_development { .. } => {
                    "disable_contract_development"
                }
                Self::set_code { .. } => "set_code",
                Self::selfdestruct { .. } => "selfdestruct",
                Self::strict_call { .. } => "strict_call",
                Self::__Ignore(_, _) => {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "internal error: entered unreachable code: {0}",
                            format_args!("__PhantomItem cannot be used.")
                        ),
                    )
                }
            }
        }
        fn get_call_names() -> &'static [&'static str] {
            &[
                "eth_call",
                "eth_call_v2",
                "call",
                "scheduled_call",
                "create",
                "create2",
                "create_nft_contract",
                "create_predeploy_contract",
                "transfer_maintainer",
                "publish_contract",
                "publish_free",
                "enable_contract_development",
                "disable_contract_development",
                "set_code",
                "selfdestruct",
                "strict_call",
            ]
        }
    }
    impl<T: Config> frame_support::dispatch::GetCallIndex for Call<T> {
        fn get_call_index(&self) -> u8 {
            match *self {
                Self::eth_call { .. } => 0u8,
                Self::eth_call_v2 { .. } => 15u8,
                Self::call { .. } => 1u8,
                Self::scheduled_call { .. } => 2u8,
                Self::create { .. } => 3u8,
                Self::create2 { .. } => 4u8,
                Self::create_nft_contract { .. } => 5u8,
                Self::create_predeploy_contract { .. } => 6u8,
                Self::transfer_maintainer { .. } => 7u8,
                Self::publish_contract { .. } => 8u8,
                Self::publish_free { .. } => 9u8,
                Self::enable_contract_development { .. } => 10u8,
                Self::disable_contract_development { .. } => 11u8,
                Self::set_code { .. } => 12u8,
                Self::selfdestruct { .. } => 13u8,
                Self::strict_call { .. } => 14u8,
                Self::__Ignore(_, _) => {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "internal error: entered unreachable code: {0}",
                            format_args!("__PhantomItem cannot be used.")
                        ),
                    )
                }
            }
        }
        fn get_call_indices() -> &'static [u8] {
            &[
                0u8,
                15u8,
                1u8,
                2u8,
                3u8,
                4u8,
                5u8,
                6u8,
                7u8,
                8u8,
                9u8,
                10u8,
                11u8,
                12u8,
                13u8,
                14u8,
            ]
        }
    }
    impl<T: Config> frame_support::traits::UnfilteredDispatchable for Call<T> {
        type RuntimeOrigin = frame_system::pallet_prelude::OriginFor<T>;
        fn dispatch_bypass_filter(
            self,
            origin: Self::RuntimeOrigin,
        ) -> frame_support::dispatch::DispatchResultWithPostInfo {
            frame_support::dispatch_context::run_in_context(|| {
                match self {
                    Self::eth_call {
                        action,
                        input,
                        value,
                        gas_limit,
                        storage_limit,
                        access_list,
                        valid_until: _valid_until,
                    } => {
                        #[allow(deprecated)]
                        <Pallet<
                            T,
                        >>::eth_call(
                                origin,
                                action,
                                input,
                                value,
                                gas_limit,
                                storage_limit,
                                access_list,
                                _valid_until,
                            )
                            .map(Into::into)
                            .map_err(Into::into)
                    }
                    Self::eth_call_v2 {
                        action,
                        input,
                        value,
                        gas_price: _gas_price,
                        gas_limit,
                        access_list,
                    } => {
                        <Pallet<
                            T,
                        >>::eth_call_v2(
                                origin,
                                action,
                                input,
                                value,
                                _gas_price,
                                gas_limit,
                                access_list,
                            )
                            .map(Into::into)
                            .map_err(Into::into)
                    }
                    Self::call {
                        target,
                        input,
                        value,
                        gas_limit,
                        storage_limit,
                        access_list,
                    } => {
                        <Pallet<
                            T,
                        >>::call(
                                origin,
                                target,
                                input,
                                value,
                                gas_limit,
                                storage_limit,
                                access_list,
                            )
                            .map(Into::into)
                            .map_err(Into::into)
                    }
                    Self::scheduled_call {
                        from,
                        target,
                        input,
                        value,
                        gas_limit,
                        storage_limit,
                        access_list,
                    } => {
                        <Pallet<
                            T,
                        >>::scheduled_call(
                                origin,
                                from,
                                target,
                                input,
                                value,
                                gas_limit,
                                storage_limit,
                                access_list,
                            )
                            .map(Into::into)
                            .map_err(Into::into)
                    }
                    Self::create {
                        input,
                        value,
                        gas_limit,
                        storage_limit,
                        access_list,
                    } => {
                        <Pallet<
                            T,
                        >>::create(
                                origin,
                                input,
                                value,
                                gas_limit,
                                storage_limit,
                                access_list,
                            )
                            .map(Into::into)
                            .map_err(Into::into)
                    }
                    Self::create2 {
                        input,
                        salt,
                        value,
                        gas_limit,
                        storage_limit,
                        access_list,
                    } => {
                        <Pallet<
                            T,
                        >>::create2(
                                origin,
                                input,
                                salt,
                                value,
                                gas_limit,
                                storage_limit,
                                access_list,
                            )
                            .map(Into::into)
                            .map_err(Into::into)
                    }
                    Self::create_nft_contract {
                        input,
                        value,
                        gas_limit,
                        storage_limit,
                        access_list,
                    } => {
                        <Pallet<
                            T,
                        >>::create_nft_contract(
                                origin,
                                input,
                                value,
                                gas_limit,
                                storage_limit,
                                access_list,
                            )
                            .map(Into::into)
                            .map_err(Into::into)
                    }
                    Self::create_predeploy_contract {
                        target,
                        input,
                        value,
                        gas_limit,
                        storage_limit,
                        access_list,
                    } => {
                        <Pallet<
                            T,
                        >>::create_predeploy_contract(
                                origin,
                                target,
                                input,
                                value,
                                gas_limit,
                                storage_limit,
                                access_list,
                            )
                            .map(Into::into)
                            .map_err(Into::into)
                    }
                    Self::transfer_maintainer { contract, new_maintainer } => {
                        <Pallet<
                            T,
                        >>::transfer_maintainer(origin, contract, new_maintainer)
                            .map(Into::into)
                            .map_err(Into::into)
                    }
                    Self::publish_contract { contract } => {
                        <Pallet<T>>::publish_contract(origin, contract)
                            .map(Into::into)
                            .map_err(Into::into)
                    }
                    Self::publish_free { contract } => {
                        <Pallet<T>>::publish_free(origin, contract)
                            .map(Into::into)
                            .map_err(Into::into)
                    }
                    Self::enable_contract_development {} => {
                        <Pallet<T>>::enable_contract_development(origin)
                            .map(Into::into)
                            .map_err(Into::into)
                    }
                    Self::disable_contract_development {} => {
                        <Pallet<T>>::disable_contract_development(origin)
                            .map(Into::into)
                            .map_err(Into::into)
                    }
                    Self::set_code { contract, code } => {
                        <Pallet<T>>::set_code(origin, contract, code)
                            .map(Into::into)
                            .map_err(Into::into)
                    }
                    Self::selfdestruct { contract } => {
                        <Pallet<T>>::selfdestruct(origin, contract)
                            .map(Into::into)
                            .map_err(Into::into)
                    }
                    Self::strict_call {
                        target,
                        input,
                        value,
                        gas_limit,
                        storage_limit,
                        access_list,
                    } => {
                        <Pallet<
                            T,
                        >>::strict_call(
                                origin,
                                target,
                                input,
                                value,
                                gas_limit,
                                storage_limit,
                                access_list,
                            )
                            .map(Into::into)
                            .map_err(Into::into)
                    }
                    Self::__Ignore(_, _) => {
                        let _ = origin;
                        ::core::panicking::panic_fmt(
                            format_args!(
                                "internal error: entered unreachable code: {0}",
                                format_args!("__PhantomItem cannot be used.")
                            ),
                        );
                    }
                }
            })
        }
    }
    impl<T: Config> frame_support::dispatch::Callable<T> for Pallet<T> {
        type RuntimeCall = Call<T>;
    }
    impl<T: Config> Pallet<T> {
        #[doc(hidden)]
        pub fn call_functions() -> frame_support::metadata_ir::PalletCallMetadataIR {
            frame_support::scale_info::meta_type::<Call<T>>().into()
        }
    }
    impl<T: Config> frame_support::sp_std::fmt::Debug for Error<T> {
        fn fmt(
            &self,
            f: &mut frame_support::sp_std::fmt::Formatter<'_>,
        ) -> frame_support::sp_std::fmt::Result {
            f.write_str(self.as_str())
        }
    }
    impl<T: Config> Error<T> {
        #[doc(hidden)]
        pub fn as_str(&self) -> &'static str {
            match &self {
                Self::__Ignore(_, _) => {
                    ::core::panicking::panic_fmt(
                        format_args!(
                            "internal error: entered unreachable code: {0}",
                            format_args!("`__Ignore` can never be constructed")
                        ),
                    )
                }
                Self::AddressNotMapped => "AddressNotMapped",
                Self::ContractNotFound => "ContractNotFound",
                Self::NoPermission => "NoPermission",
                Self::ContractDevelopmentNotEnabled => "ContractDevelopmentNotEnabled",
                Self::ContractDevelopmentAlreadyEnabled => {
                    "ContractDevelopmentAlreadyEnabled"
                }
                Self::ContractAlreadyPublished => "ContractAlreadyPublished",
                Self::ContractExceedsMaxCodeSize => "ContractExceedsMaxCodeSize",
                Self::ContractAlreadyExisted => "ContractAlreadyExisted",
                Self::OutOfStorage => "OutOfStorage",
                Self::ChargeFeeFailed => "ChargeFeeFailed",
                Self::CannotKillContract => "CannotKillContract",
                Self::ReserveStorageFailed => "ReserveStorageFailed",
                Self::UnreserveStorageFailed => "UnreserveStorageFailed",
                Self::ChargeStorageFailed => "ChargeStorageFailed",
                Self::InvalidDecimals => "InvalidDecimals",
                Self::StrictCallFailed => "StrictCallFailed",
            }
        }
    }
    impl<T: Config> From<Error<T>> for &'static str {
        fn from(err: Error<T>) -> &'static str {
            err.as_str()
        }
    }
    impl<T: Config> From<Error<T>> for frame_support::sp_runtime::DispatchError {
        fn from(err: Error<T>) -> Self {
            use frame_support::codec::Encode;
            let index = <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::index::<
                Pallet<T>,
            >()
                .expect("Every active module has an index in the runtime; qed") as u8;
            let mut encoded = err.encode();
            encoded.resize(frame_support::MAX_MODULE_ERROR_ENCODED_SIZE, 0);
            frame_support::sp_runtime::DispatchError::Module(frame_support::sp_runtime::ModuleError {
                index,
                error: TryInto::try_into(encoded)
                    .expect(
                        "encoded error is resized to be equal to the maximum encoded error size; qed",
                    ),
                message: Some(err.as_str()),
            })
        }
    }
    pub use __tt_error_token_1 as tt_error_token;
    #[doc(hidden)]
    pub mod __substrate_event_check {
        #[doc(hidden)]
        pub use __is_event_part_defined_2 as is_event_part_defined;
    }
    impl<T: Config> Pallet<T> {
        pub(crate) fn deposit_event(event: Event<T>) {
            let event = <<T as Config>::RuntimeEvent as From<Event<T>>>::from(event);
            let event = <<T as Config>::RuntimeEvent as Into<
                <T as frame_system::Config>::RuntimeEvent,
            >>::into(event);
            <frame_system::Pallet<T>>::deposit_event(event)
        }
    }
    impl<T: Config> From<Event<T>> for () {
        fn from(_: Event<T>) {}
    }
    impl<T: Config> Pallet<T> {
        #[doc(hidden)]
        pub fn storage_metadata() -> frame_support::metadata_ir::PalletStorageMetadataIR {
            frame_support::metadata_ir::PalletStorageMetadataIR {
                prefix: <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::name::<
                    Pallet<T>,
                >()
                    .expect(
                        "No name found for the pallet in the runtime! This usually means that the pallet wasn't added to `construct_runtime!`.",
                    ),
                entries: {
                    #[allow(unused_mut)]
                    let mut entries = ::alloc::vec::Vec::new();
                    {
                        <ChainId<
                            T,
                        > as frame_support::storage::StorageEntryMetadataBuilder>::build_metadata(
                            <[_]>::into_vec(
                                #[rustc_box]
                                ::alloc::boxed::Box::new([
                                    " The EVM Chain ID.",
                                    "",
                                    " ChainId: u64",
                                ]),
                            ),
                            &mut entries,
                        );
                    }
                    {
                        <Accounts<
                            T,
                        > as frame_support::storage::StorageEntryMetadataBuilder>::build_metadata(
                            <[_]>::into_vec(
                                #[rustc_box]
                                ::alloc::boxed::Box::new([
                                    " The EVM accounts info.",
                                    "",
                                    " Accounts: map EvmAddress => Option<AccountInfo<T>>",
                                ]),
                            ),
                            &mut entries,
                        );
                    }
                    {
                        <ContractStorageSizes<
                            T,
                        > as frame_support::storage::StorageEntryMetadataBuilder>::build_metadata(
                            <[_]>::into_vec(
                                #[rustc_box]
                                ::alloc::boxed::Box::new([
                                    " The storage usage for contracts. Including code size, extra bytes and total AccountStorages",
                                    " size.",
                                    "",
                                    " Accounts: map EvmAddress => u32",
                                ]),
                            ),
                            &mut entries,
                        );
                    }
                    {
                        <AccountStorages<
                            T,
                        > as frame_support::storage::StorageEntryMetadataBuilder>::build_metadata(
                            <[_]>::into_vec(
                                #[rustc_box]
                                ::alloc::boxed::Box::new([
                                    " The storages for EVM contracts.",
                                    "",
                                    " AccountStorages: double_map EvmAddress, H256 => H256",
                                ]),
                            ),
                            &mut entries,
                        );
                    }
                    {
                        <Codes<
                            T,
                        > as frame_support::storage::StorageEntryMetadataBuilder>::build_metadata(
                            <[_]>::into_vec(
                                #[rustc_box]
                                ::alloc::boxed::Box::new([
                                    " The code for EVM contracts.",
                                    " Key is Keccak256 hash of code.",
                                    "",
                                    " Codes: H256 => Vec<u8>",
                                ]),
                            ),
                            &mut entries,
                        );
                    }
                    {
                        <CodeInfos<
                            T,
                        > as frame_support::storage::StorageEntryMetadataBuilder>::build_metadata(
                            <[_]>::into_vec(
                                #[rustc_box]
                                ::alloc::boxed::Box::new([
                                    " The code info for EVM contracts.",
                                    " Key is Keccak256 hash of code.",
                                    "",
                                    " CodeInfos: H256 => Option<CodeInfo>",
                                ]),
                            ),
                            &mut entries,
                        );
                    }
                    {
                        <NetworkContractIndex<
                            T,
                        > as frame_support::storage::StorageEntryMetadataBuilder>::build_metadata(
                            <[_]>::into_vec(
                                #[rustc_box]
                                ::alloc::boxed::Box::new([
                                    " Next available system contract address.",
                                    "",
                                    " NetworkContractIndex: u64",
                                ]),
                            ),
                            &mut entries,
                        );
                    }
                    {
                        <ExtrinsicOrigin<
                            T,
                        > as frame_support::storage::StorageEntryMetadataBuilder>::build_metadata(
                            <[_]>::into_vec(
                                #[rustc_box]
                                ::alloc::boxed::Box::new([
                                    " Extrinsics origin for the current transaction.",
                                    "",
                                    " ExtrinsicOrigin: Option<AccountId>",
                                ]),
                            ),
                            &mut entries,
                        );
                    }
                    {
                        <XcmOrigin<
                            T,
                        > as frame_support::storage::StorageEntryMetadataBuilder>::build_metadata(
                            <[_]>::into_vec(
                                #[rustc_box]
                                ::alloc::boxed::Box::new([
                                    " Xcm origin for the current transaction.",
                                    "",
                                    " XcmOrigin: Option<Vec<AccountId>>",
                                ]),
                            ),
                            &mut entries,
                        );
                    }
                    entries
                },
            }
        }
    }
    impl<T: Config> Pallet<T> {
        ///An auto-generated getter for [`ChainId`].
        pub fn chain_id() -> u64 {
            <ChainId<T> as frame_support::storage::StorageValue<u64>>::get()
        }
    }
    impl<T: Config> Pallet<T> {
        ///An auto-generated getter for [`Accounts`].
        pub fn accounts<KArg>(k: KArg) -> Option<AccountInfo<T::Index>>
        where
            KArg: frame_support::codec::EncodeLike<EvmAddress>,
        {
            <Accounts<
                T,
            > as frame_support::storage::StorageMap<
                EvmAddress,
                AccountInfo<T::Index>,
            >>::get(k)
        }
    }
    impl<T: Config> Pallet<T> {
        ///An auto-generated getter for [`ContractStorageSizes`].
        pub fn contract_storage_sizes<KArg>(k: KArg) -> u32
        where
            KArg: frame_support::codec::EncodeLike<EvmAddress>,
        {
            <ContractStorageSizes<
                T,
            > as frame_support::storage::StorageMap<EvmAddress, u32>>::get(k)
        }
    }
    impl<T: Config> Pallet<T> {
        ///An auto-generated getter for [`AccountStorages`].
        pub fn account_storages<KArg1, KArg2>(k1: KArg1, k2: KArg2) -> H256
        where
            KArg1: frame_support::codec::EncodeLike<EvmAddress>,
            KArg2: frame_support::codec::EncodeLike<H256>,
        {
            <AccountStorages<
                T,
            > as frame_support::storage::StorageDoubleMap<
                EvmAddress,
                H256,
                H256,
            >>::get(k1, k2)
        }
    }
    impl<T: Config> Pallet<T> {
        ///An auto-generated getter for [`Codes`].
        pub fn codes<KArg>(k: KArg) -> BoundedVec<u8, MaxCodeSize>
        where
            KArg: frame_support::codec::EncodeLike<H256>,
        {
            <Codes<
                T,
            > as frame_support::storage::StorageMap<
                H256,
                BoundedVec<u8, MaxCodeSize>,
            >>::get(k)
        }
    }
    impl<T: Config> Pallet<T> {
        ///An auto-generated getter for [`CodeInfos`].
        pub fn code_infos<KArg>(k: KArg) -> Option<CodeInfo>
        where
            KArg: frame_support::codec::EncodeLike<H256>,
        {
            <CodeInfos<T> as frame_support::storage::StorageMap<H256, CodeInfo>>::get(k)
        }
    }
    impl<T: Config> Pallet<T> {
        ///An auto-generated getter for [`NetworkContractIndex`].
        pub fn network_contract_index() -> u64 {
            <NetworkContractIndex<T> as frame_support::storage::StorageValue<u64>>::get()
        }
    }
    impl<T: Config> Pallet<T> {
        ///An auto-generated getter for [`ExtrinsicOrigin`].
        pub fn extrinsic_origin() -> Option<T::AccountId> {
            <ExtrinsicOrigin<
                T,
            > as frame_support::storage::StorageValue<T::AccountId>>::get()
        }
    }
    impl<T: Config> Pallet<T> {
        ///An auto-generated getter for [`XcmOrigin`].
        pub fn xcm_origin() -> Option<Vec<T::AccountId>> {
            <XcmOrigin<
                T,
            > as frame_support::storage::StorageValue<Vec<T::AccountId>>>::get()
        }
    }
    #[doc(hidden)]
    pub struct _GeneratedPrefixForStorageChainId<T>(core::marker::PhantomData<(T,)>);
    impl<T: Config> frame_support::traits::StorageInstance
    for _GeneratedPrefixForStorageChainId<T> {
        fn pallet_prefix() -> &'static str {
            <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::name::<
                Pallet<T>,
            >()
                .expect(
                    "No name found for the pallet in the runtime! This usually means that the pallet wasn't added to `construct_runtime!`.",
                )
        }
        const STORAGE_PREFIX: &'static str = "ChainId";
    }
    #[doc(hidden)]
    pub struct _GeneratedPrefixForStorageAccounts<T>(core::marker::PhantomData<(T,)>);
    impl<T: Config> frame_support::traits::StorageInstance
    for _GeneratedPrefixForStorageAccounts<T> {
        fn pallet_prefix() -> &'static str {
            <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::name::<
                Pallet<T>,
            >()
                .expect(
                    "No name found for the pallet in the runtime! This usually means that the pallet wasn't added to `construct_runtime!`.",
                )
        }
        const STORAGE_PREFIX: &'static str = "Accounts";
    }
    #[doc(hidden)]
    pub struct _GeneratedPrefixForStorageContractStorageSizes<T>(
        core::marker::PhantomData<(T,)>,
    );
    impl<T: Config> frame_support::traits::StorageInstance
    for _GeneratedPrefixForStorageContractStorageSizes<T> {
        fn pallet_prefix() -> &'static str {
            <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::name::<
                Pallet<T>,
            >()
                .expect(
                    "No name found for the pallet in the runtime! This usually means that the pallet wasn't added to `construct_runtime!`.",
                )
        }
        const STORAGE_PREFIX: &'static str = "ContractStorageSizes";
    }
    #[doc(hidden)]
    pub struct _GeneratedPrefixForStorageAccountStorages<T>(
        core::marker::PhantomData<(T,)>,
    );
    impl<T: Config> frame_support::traits::StorageInstance
    for _GeneratedPrefixForStorageAccountStorages<T> {
        fn pallet_prefix() -> &'static str {
            <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::name::<
                Pallet<T>,
            >()
                .expect(
                    "No name found for the pallet in the runtime! This usually means that the pallet wasn't added to `construct_runtime!`.",
                )
        }
        const STORAGE_PREFIX: &'static str = "AccountStorages";
    }
    #[doc(hidden)]
    pub struct _GeneratedPrefixForStorageCodes<T>(core::marker::PhantomData<(T,)>);
    impl<T: Config> frame_support::traits::StorageInstance
    for _GeneratedPrefixForStorageCodes<T> {
        fn pallet_prefix() -> &'static str {
            <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::name::<
                Pallet<T>,
            >()
                .expect(
                    "No name found for the pallet in the runtime! This usually means that the pallet wasn't added to `construct_runtime!`.",
                )
        }
        const STORAGE_PREFIX: &'static str = "Codes";
    }
    #[doc(hidden)]
    pub struct _GeneratedPrefixForStorageCodeInfos<T>(core::marker::PhantomData<(T,)>);
    impl<T: Config> frame_support::traits::StorageInstance
    for _GeneratedPrefixForStorageCodeInfos<T> {
        fn pallet_prefix() -> &'static str {
            <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::name::<
                Pallet<T>,
            >()
                .expect(
                    "No name found for the pallet in the runtime! This usually means that the pallet wasn't added to `construct_runtime!`.",
                )
        }
        const STORAGE_PREFIX: &'static str = "CodeInfos";
    }
    #[doc(hidden)]
    pub struct _GeneratedPrefixForStorageNetworkContractIndex<T>(
        core::marker::PhantomData<(T,)>,
    );
    impl<T: Config> frame_support::traits::StorageInstance
    for _GeneratedPrefixForStorageNetworkContractIndex<T> {
        fn pallet_prefix() -> &'static str {
            <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::name::<
                Pallet<T>,
            >()
                .expect(
                    "No name found for the pallet in the runtime! This usually means that the pallet wasn't added to `construct_runtime!`.",
                )
        }
        const STORAGE_PREFIX: &'static str = "NetworkContractIndex";
    }
    #[doc(hidden)]
    pub struct _GeneratedPrefixForStorageExtrinsicOrigin<T>(
        core::marker::PhantomData<(T,)>,
    );
    impl<T: Config> frame_support::traits::StorageInstance
    for _GeneratedPrefixForStorageExtrinsicOrigin<T> {
        fn pallet_prefix() -> &'static str {
            <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::name::<
                Pallet<T>,
            >()
                .expect(
                    "No name found for the pallet in the runtime! This usually means that the pallet wasn't added to `construct_runtime!`.",
                )
        }
        const STORAGE_PREFIX: &'static str = "ExtrinsicOrigin";
    }
    #[doc(hidden)]
    pub struct _GeneratedPrefixForStorageXcmOrigin<T>(core::marker::PhantomData<(T,)>);
    impl<T: Config> frame_support::traits::StorageInstance
    for _GeneratedPrefixForStorageXcmOrigin<T> {
        fn pallet_prefix() -> &'static str {
            <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::name::<
                Pallet<T>,
            >()
                .expect(
                    "No name found for the pallet in the runtime! This usually means that the pallet wasn't added to `construct_runtime!`.",
                )
        }
        const STORAGE_PREFIX: &'static str = "XcmOrigin";
    }
    #[doc(hidden)]
    pub mod __substrate_inherent_check {
        #[doc(hidden)]
        pub use __is_inherent_part_defined_3 as is_inherent_part_defined;
    }
    /// Hidden instance generated to be internally used when module is used without
    /// instance.
    #[doc(hidden)]
    pub type __InherentHiddenInstance = ();
    impl<
        T: Config,
    > frame_support::traits::OnFinalize<<T as frame_system::Config>::BlockNumber>
    for Pallet<T> {
        fn on_finalize(n: <T as frame_system::Config>::BlockNumber) {
            <Self as frame_support::traits::Hooks<
                <T as frame_system::Config>::BlockNumber,
            >>::on_finalize(n)
        }
    }
    impl<
        T: Config,
    > frame_support::traits::OnIdle<<T as frame_system::Config>::BlockNumber>
    for Pallet<T> {
        fn on_idle(
            n: <T as frame_system::Config>::BlockNumber,
            remaining_weight: frame_support::weights::Weight,
        ) -> frame_support::weights::Weight {
            <Self as frame_support::traits::Hooks<
                <T as frame_system::Config>::BlockNumber,
            >>::on_idle(n, remaining_weight)
        }
    }
    impl<
        T: Config,
    > frame_support::traits::OnInitialize<<T as frame_system::Config>::BlockNumber>
    for Pallet<T> {
        fn on_initialize(
            n: <T as frame_system::Config>::BlockNumber,
        ) -> frame_support::weights::Weight {
            <Self as frame_support::traits::Hooks<
                <T as frame_system::Config>::BlockNumber,
            >>::on_initialize(n)
        }
    }
    impl<T: Config> frame_support::traits::OnRuntimeUpgrade for Pallet<T> {
        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            let pallet_name = <<T as frame_system::Config>::PalletInfo as frame_support::traits::PalletInfo>::name::<
                Self,
            >()
                .unwrap_or("<unknown pallet name>");
            {
                let lvl = ::log::Level::Debug;
                if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                    ::log::__private_api_log(
                        format_args!("✅ no migration for {0}", pallet_name),
                        lvl,
                        &(
                            frame_support::LOG_TARGET,
                            "module_evm::module",
                            "modules/evm/src/lib.rs",
                            160u32,
                        ),
                        ::log::__private_api::Option::None,
                    );
                }
            };
            <Self as frame_support::traits::Hooks<
                <T as frame_system::Config>::BlockNumber,
            >>::on_runtime_upgrade()
        }
    }
    impl<
        T: Config,
    > frame_support::traits::OffchainWorker<<T as frame_system::Config>::BlockNumber>
    for Pallet<T> {
        fn offchain_worker(n: <T as frame_system::Config>::BlockNumber) {
            <Self as frame_support::traits::Hooks<
                <T as frame_system::Config>::BlockNumber,
            >>::offchain_worker(n)
        }
    }
    #[doc(hidden)]
    pub mod __substrate_genesis_config_check {
        #[doc(hidden)]
        pub use __is_genesis_config_defined_4 as is_genesis_config_defined;
        #[doc(hidden)]
        pub use __is_std_macro_defined_for_genesis_4 as is_std_enabled_for_genesis;
    }
    #[doc(hidden)]
    pub mod __substrate_origin_check {
        #[doc(hidden)]
        pub use __is_origin_part_defined_5 as is_origin_part_defined;
    }
    #[doc(hidden)]
    pub mod __substrate_validate_unsigned_check {
        #[doc(hidden)]
        pub use __is_validate_unsigned_part_defined_6 as is_validate_unsigned_part_defined;
    }
    pub use __tt_default_parts_7 as tt_default_parts;
    pub use __tt_extra_parts_7 as tt_extra_parts;
}
impl<T: Config> Pallet<T> {
    /// Get StorageDepositPerByte of actual decimals
    pub fn get_storage_deposit_per_byte() -> BalanceOf<T> {
        convert_decimals_from_evm(T::StorageDepositPerByte::get())
            .expect("checked in integrity_test; qed")
    }
    /// Check whether an account is empty.
    pub fn is_account_empty(address: &H160) -> bool {
        let account_id = T::AddressMapping::get_account_id(address);
        let balance = T::Currency::total_balance(&account_id);
        if !balance.is_zero() {
            return false;
        }
        Self::accounts(address)
            .map_or(
                true,
                |account_info| {
                    account_info.contract_info.is_none() && account_info.nonce.is_zero()
                },
            )
    }
    /// Remove an account if its empty.
    /// NOTE: If the nonce is non-zero, it cannot be deleted to prevent the user from failing to
    /// create a contract due to nonce reset
    pub fn remove_account_if_empty(address: &H160) {
        if Self::is_account_empty(address) {
            Self::remove_account(address);
        }
    }
    pub fn remove_contract(
        caller: &EvmAddress,
        contract: &EvmAddress,
    ) -> DispatchResult {
        use frame_support::storage::{with_transaction, TransactionOutcome};
        with_transaction(|| {
            let r = (|| {
                {
                    let contract_account = T::AddressMapping::get_account_id(contract);
                    let task_id = Accounts::<
                        T,
                    >::try_mutate_exists(
                        contract,
                        |maybe_account_info| -> Result<Nonce, DispatchError> {
                            let account_info = maybe_account_info
                                .as_mut()
                                .ok_or(Error::<T>::ContractNotFound)?;
                            let contract_info = account_info
                                .contract_info
                                .take()
                                .ok_or(Error::<T>::ContractNotFound)?;
                            let mut code_size: u32 = 0;
                            CodeInfos::<
                                T,
                            >::mutate_exists(
                                contract_info.code_hash,
                                |maybe_code_info| {
                                    if let Some(code_info) = maybe_code_info.as_mut() {
                                        code_size = code_info.code_size;
                                        code_info.ref_count = code_info.ref_count.saturating_sub(1);
                                        if code_info.ref_count == 0 {
                                            Codes::<T>::remove(contract_info.code_hash);
                                            *maybe_code_info = None;
                                        }
                                    } else {
                                        if true {
                                            if !false {
                                                ::core::panicking::panic("assertion failed: false")
                                            }
                                        }
                                    }
                                },
                            );
                            let _total_size = ContractStorageSizes::<T>::take(contract);
                            T::IdleScheduler::schedule(
                                EvmTask::Remove {
                                    caller: *caller,
                                    contract: *contract,
                                    maintainer: contract_info.maintainer,
                                }
                                    .into(),
                            )
                        },
                    )?;
                    let weight_limit = Weight::from_parts(
                        <T as frame_system::Config>::DbWeight::get()
                            .write
                            .saturating_mul(IMMEDIATE_REMOVE_LIMIT.into()),
                        0,
                    );
                    let _weight_remaining = T::IdleScheduler::dispatch(
                        task_id,
                        weight_limit,
                    );
                    frame_system::Pallet::<T>::dec_providers(&contract_account)?;
                    Ok(())
                }
            })();
            if r.is_ok() {
                TransactionOutcome::Commit(r)
            } else {
                TransactionOutcome::Rollback(r)
            }
        })
    }
    /// Removes an account from Accounts and AccountStorages.
    /// NOTE: It will reset account nonce.
    fn remove_account(address: &EvmAddress) {
        Accounts::<
            T,
        >::mutate_exists(
            address,
            |maybe_account| {
                if let Some(account) = maybe_account {
                    if let Some(ContractInfo { code_hash, .. }) = account.contract_info {
                        CodeInfos::<
                            T,
                        >::mutate_exists(
                            code_hash,
                            |maybe_code_info| {
                                if let Some(code_info) = maybe_code_info {
                                    code_info.ref_count = code_info.ref_count.saturating_sub(1);
                                    if code_info.ref_count == 0 {
                                        Codes::<T>::remove(code_hash);
                                        *maybe_code_info = None;
                                    }
                                }
                            },
                        );
                        {
                            let lvl = ::log::Level::Warn;
                            if lvl <= ::log::STATIC_MAX_LEVEL
                                && lvl <= ::log::max_level()
                            {
                                ::log::__private_api_log(
                                    format_args!(
                                        "remove_account: removed account {0:?} while is still linked to contract info",
                                        address
                                    ),
                                    lvl,
                                    &("evm", "module_evm", "modules/evm/src/lib.rs", 1423u32),
                                    ::log::__private_api::Option::None,
                                );
                            }
                        };
                        if true {
                            if !false {
                                ::core::panicking::panic_fmt(
                                    format_args!(
                                        "removed account while is still linked to contract info"
                                    ),
                                )
                            }
                        }
                    }
                    *maybe_account = None;
                }
            },
        );
    }
    /// Create an account.
    /// - Create new account for the contract.
    /// - Update codes info.
    /// - Update maintainer of the contract.
    /// - Save `code` if not saved yet.
    pub fn create_contract(source: H160, address: H160, publish: bool, code: Vec<u8>) {
        let bounded_code: BoundedVec<u8, MaxCodeSize> = code
            .try_into()
            .expect("checked by create_contract_limit in ACALA_CONFIG; qed");
        if bounded_code.is_empty() {
            return;
        }
        let maintainer = source;
        let code_hash = code_hash(bounded_code.as_slice());
        let code_size = bounded_code.len() as u32;
        let contract_info = ContractInfo {
            code_hash,
            maintainer,
            #[cfg(not(feature = "with-ethereum-compatibility"))]
            published: publish,
        };
        CodeInfos::<
            T,
        >::mutate_exists(
            code_hash,
            |maybe_code_info| {
                if let Some(code_info) = maybe_code_info.as_mut() {
                    code_info.ref_count = code_info.ref_count.saturating_add(1);
                } else {
                    let new = CodeInfo {
                        code_size,
                        ref_count: 1,
                    };
                    *maybe_code_info = Some(new);
                    Codes::<T>::insert(code_hash, bounded_code);
                }
            },
        );
        Accounts::<
            T,
        >::mutate(
            address,
            |maybe_account_info| {
                if let Some(account_info) = maybe_account_info.as_mut() {
                    account_info.contract_info = Some(contract_info.clone());
                } else {
                    let account_info = AccountInfo::<
                        T::Index,
                    >::new(Default::default(), Some(contract_info.clone()));
                    *maybe_account_info = Some(account_info);
                }
            },
        );
        let contract_account = T::AddressMapping::get_account_id(&address);
        frame_system::Pallet::<T>::inc_providers(&contract_account);
    }
    /// Get the account basic in EVM format.
    pub fn account_basic(address: &EvmAddress) -> Account {
        let account_id = T::AddressMapping::get_account_id(address);
        let nonce = Self::accounts(address)
            .map_or(Default::default(), |account_info| account_info.nonce);
        let balance = T::Currency::free_balance(&account_id);
        Account {
            nonce: U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(nonce)),
            balance: U256::from(
                UniqueSaturatedInto::<
                    u128,
                >::unique_saturated_into(convert_decimals_to_evm(balance)),
            ),
        }
    }
    /// Get the author using the FindAuthor trait.
    pub fn find_author() -> H160 {
        let digest = <frame_system::Pallet<T>>::digest();
        let pre_runtime_digests = digest.logs.iter().filter_map(|d| d.as_pre_runtime());
        if let Some(author) = T::FindAuthor::find_author(pre_runtime_digests) {
            T::AddressMapping::get_default_evm_address(&author)
        } else {
            H160::default()
        }
    }
    /// Get code hash at given address.
    pub fn code_hash_at_address(address: &EvmAddress) -> H256 {
        if let Some(AccountInfo { contract_info: Some(contract_info), .. })
            = Self::accounts(address) {
            contract_info.code_hash
        } else {
            H256::from_slice(
                &[
                    197u8,
                    210u8,
                    70u8,
                    1u8,
                    134u8,
                    247u8,
                    35u8,
                    60u8,
                    146u8,
                    126u8,
                    125u8,
                    178u8,
                    220u8,
                    199u8,
                    3u8,
                    192u8,
                    229u8,
                    0u8,
                    182u8,
                    83u8,
                    202u8,
                    130u8,
                    39u8,
                    59u8,
                    123u8,
                    250u8,
                    216u8,
                    4u8,
                    93u8,
                    133u8,
                    164u8,
                    112u8,
                ],
            )
        }
    }
    /// Get code size at given address.
    pub fn code_size_at_address(address: &EvmAddress) -> U256 {
        Self::code_infos(Self::code_hash_at_address(address))
            .map_or(U256::zero(), |code_info| U256::from(code_info.code_size))
    }
    /// Get code at given address.
    pub fn code_at_address(address: &EvmAddress) -> BoundedVec<u8, MaxCodeSize> {
        Self::codes(Self::code_hash_at_address(address))
    }
    pub fn is_contract(address: &EvmAddress) -> bool {
        match Self::accounts(address) {
            Some(AccountInfo { contract_info: Some(_), .. }) => true,
            _ => false,
        }
    }
    pub fn update_contract_storage_size(address: &EvmAddress, change: i32) {
        if change == 0 {
            return;
        }
        ContractStorageSizes::<
            T,
        >::mutate(
            address,
            |val| {
                if change > 0 {
                    *val = val.saturating_add(change as u32);
                } else {
                    *val = val.saturating_sub(change.unsigned_abs());
                }
            },
        );
    }
    /// Sets a given contract's contract info to a new maintainer.
    fn do_transfer_maintainer(
        who: T::AccountId,
        contract: EvmAddress,
        new_maintainer: EvmAddress,
    ) -> DispatchResult {
        Accounts::<
            T,
        >::mutate(
            contract,
            |maybe_account_info| -> DispatchResult {
                let account_info = maybe_account_info
                    .as_mut()
                    .ok_or(Error::<T>::ContractNotFound)?;
                let contract_info = account_info
                    .contract_info
                    .as_mut()
                    .ok_or(Error::<T>::ContractNotFound)?;
                let maintainer = T::AddressMapping::get_evm_address(&who)
                    .ok_or(Error::<T>::AddressNotMapped)?;
                {
                    if !(contract_info.maintainer == maintainer) {
                        { return Err(Error::<T>::NoPermission.into()) };
                    }
                };
                contract_info.maintainer = new_maintainer;
                Ok(())
            },
        )?;
        Ok(())
    }
    /// Puts a deposit down to allow account to interact with non-published contracts
    fn do_enable_contract_development(who: &T::AccountId) -> DispatchResult {
        {
            if !T::Currency::reserved_balance_named(&RESERVE_ID_DEVELOPER_DEPOSIT, who)
                .is_zero()
            {
                { return Err(Error::<T>::ContractDevelopmentAlreadyEnabled.into()) };
            }
        };
        T::Currency::ensure_reserved_named(
            &RESERVE_ID_DEVELOPER_DEPOSIT,
            who,
            T::DeveloperDeposit::get(),
        )?;
        Ok(())
    }
    /// Returns deposit and disables account for contract development
    fn do_disable_contract_development(who: &T::AccountId) -> DispatchResult {
        {
            if !!T::Currency::reserved_balance_named(&RESERVE_ID_DEVELOPER_DEPOSIT, who)
                .is_zero()
            {
                { return Err(Error::<T>::ContractDevelopmentNotEnabled.into()) };
            }
        };
        T::Currency::unreserve_all_named(&RESERVE_ID_DEVELOPER_DEPOSIT, who);
        Ok(())
    }
    /// Publishes the Contract
    ///
    /// Checks that `who` is the contract maintainer and takes the publication fee
    fn do_publish_contract(who: T::AccountId, contract: EvmAddress) -> DispatchResult {
        let address = T::AddressMapping::get_evm_address(&who)
            .ok_or(Error::<T>::AddressNotMapped)?;
        T::Currency::transfer(
            &who,
            &T::TreasuryAccount::get(),
            T::PublicationFee::get(),
            ExistenceRequirement::AllowDeath,
        )?;
        Self::mark_published(contract, Some(address))?;
        Ok(())
    }
    /// Mark contract as published
    ///
    /// If maintainer is provider then it will check maintainer
    fn mark_published(
        contract: EvmAddress,
        maintainer: Option<EvmAddress>,
    ) -> DispatchResult {
        Accounts::<
            T,
        >::mutate(
            contract,
            |maybe_account_info| -> DispatchResult {
                if let Some(AccountInfo { contract_info: Some(contract_info), .. })
                    = maybe_account_info.as_mut()
                {
                    if let Some(maintainer) = maintainer {
                        {
                            if !(contract_info.maintainer == maintainer) {
                                { return Err(Error::<T>::NoPermission.into()) };
                            }
                        };
                    }
                    {
                        if !!contract_info.published {
                            { return Err(Error::<T>::ContractAlreadyPublished.into()) };
                        }
                    };
                    contract_info.published = true;
                    Ok(())
                } else {
                    Err(Error::<T>::ContractNotFound.into())
                }
            },
        )
    }
    /// Set the code of a contract at a given address.
    ///
    /// - Ensures signer is maintainer or root.
    /// - Update codes info.
    /// - Save `code` if not saved yet.
    fn do_set_code(
        root_or_signed: Either<(), T::AccountId>,
        contract: EvmAddress,
        code: Vec<u8>,
    ) -> DispatchResult {
        Accounts::<
            T,
        >::mutate(
            contract,
            |maybe_account_info| -> DispatchResult {
                let account_info = maybe_account_info
                    .as_mut()
                    .ok_or(Error::<T>::ContractNotFound)?;
                let contract_info = account_info
                    .contract_info
                    .as_mut()
                    .ok_or(Error::<T>::ContractNotFound)?;
                let source = if let Either::Right(signer) = root_or_signed {
                    let maintainer = T::AddressMapping::get_evm_address(&signer)
                        .ok_or(Error::<T>::AddressNotMapped)?;
                    {
                        if !(contract_info.maintainer == maintainer) {
                            { return Err(Error::<T>::NoPermission.into()) };
                        }
                    };
                    {
                        if !!contract_info.published {
                            { return Err(Error::<T>::ContractAlreadyPublished.into()) };
                        }
                    };
                    maintainer
                } else {
                    T::NetworkContractSource::get()
                };
                let old_code_info = Self::code_infos(contract_info.code_hash)
                    .ok_or(Error::<T>::ContractNotFound)?;
                let bounded_code: BoundedVec<u8, MaxCodeSize> = code
                    .try_into()
                    .map_err(|_| Error::<T>::ContractExceedsMaxCodeSize)?;
                let code_hash = code_hash(bounded_code.as_slice());
                let code_size = bounded_code.len() as u32;
                if code_hash == contract_info.code_hash {
                    return Ok(());
                }
                let storage_size_changed: i32 = code_size
                    .saturating_add(T::NewContractExtraBytes::get()) as i32
                    - old_code_info.code_size as i32;
                if storage_size_changed.is_positive() {
                    Self::reserve_storage(&source, storage_size_changed as u32)?;
                }
                Self::charge_storage(&source, &contract, storage_size_changed)?;
                Self::update_contract_storage_size(&contract, storage_size_changed);
                CodeInfos::<
                    T,
                >::mutate_exists(
                    contract_info.code_hash,
                    |maybe_code_info| -> DispatchResult {
                        let code_info = maybe_code_info
                            .as_mut()
                            .ok_or(Error::<T>::ContractNotFound)?;
                        code_info.ref_count = code_info.ref_count.saturating_sub(1);
                        if code_info.ref_count == 0 {
                            Codes::<T>::remove(contract_info.code_hash);
                            *maybe_code_info = None;
                        }
                        Ok(())
                    },
                )?;
                CodeInfos::<
                    T,
                >::mutate_exists(
                    code_hash,
                    |maybe_code_info| {
                        if let Some(code_info) = maybe_code_info.as_mut() {
                            code_info.ref_count = code_info.ref_count.saturating_add(1);
                        } else {
                            let new = CodeInfo {
                                code_size,
                                ref_count: 1,
                            };
                            *maybe_code_info = Some(new);
                            Codes::<T>::insert(code_hash, bounded_code);
                        }
                    },
                );
                contract_info.code_hash = code_hash;
                Ok(())
            },
        )
    }
    /// Selfdestruct a contract at a given address.
    fn do_selfdestruct(caller: &EvmAddress, contract: &EvmAddress) -> DispatchResult {
        let account_info = Self::accounts(contract).ok_or(Error::<T>::ContractNotFound)?;
        let contract_info = account_info
            .contract_info
            .as_ref()
            .ok_or(Error::<T>::ContractNotFound)?;
        {
            if !(contract_info.maintainer == *caller) {
                { return Err(Error::<T>::NoPermission.into()) };
            }
        };
        {
            if !!contract_info.published {
                { return Err(Error::<T>::ContractAlreadyPublished.into()) };
            }
        };
        Self::remove_contract(caller, contract)
    }
    fn ensure_root_or_signed(
        o: T::RuntimeOrigin,
    ) -> Result<Either<(), T::AccountId>, BadOrigin> {
        EitherOfDiverse::<
            EnsureRoot<T::AccountId>,
            EnsureSigned<T::AccountId>,
        >::try_origin(o)
            .map_or(Err(BadOrigin), Ok)
    }
    fn can_call_contract(address: &H160, caller: &H160) -> bool {
        if let Some(
            AccountInfo {
                contract_info: Some(ContractInfo { published, maintainer, .. }),
                ..
            },
        ) = Accounts::<T>::get(address) {
            published || maintainer == *caller || *caller == H160::default()
                || Self::is_developer_or_contract(caller)
        } else {
            true
        }
    }
    fn is_developer_or_contract(caller: &H160) -> bool {
        let account_id = T::AddressMapping::get_account_id(caller);
        Self::query_developer_status(account_id) || Self::is_contract(caller)
    }
    fn reserve_storage(caller: &H160, limit: u32) -> DispatchResult {
        if limit.is_zero() {
            return Ok(());
        }
        let user = T::AddressMapping::get_account_id(caller);
        let amount = Self::get_storage_deposit_per_byte().saturating_mul(limit.into());
        {
            let lvl = ::log::Level::Debug;
            if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                ::log::__private_api_log(
                    format_args!(
                        "reserve_storage: [from: {0:?}, account: {1:?}, limit: {2:?}, amount: {3:?}]",
                        caller, user, limit, amount
                    ),
                    lvl,
                    &("evm", "module_evm", "modules/evm/src/lib.rs", 1775u32),
                    ::log::__private_api::Option::None,
                );
            }
        };
        T::ChargeTransactionPayment::reserve_fee(
            &user,
            amount,
            Some(RESERVE_ID_STORAGE_DEPOSIT),
        )?;
        Ok(())
    }
    fn unreserve_storage(
        caller: &H160,
        limit: u32,
        used: u32,
        refunded: u32,
    ) -> DispatchResult {
        let total = limit.saturating_add(refunded);
        let unused = total.saturating_sub(used);
        if unused.is_zero() {
            return Ok(());
        }
        let user = T::AddressMapping::get_account_id(caller);
        let amount = Self::get_storage_deposit_per_byte().saturating_mul(unused.into());
        {
            let lvl = ::log::Level::Debug;
            if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                ::log::__private_api_log(
                    format_args!(
                        "unreserve_storage: [from: {0:?}, account: {1:?}, used: {2:?}, refunded: {3:?}, unused: {4:?}, amount: {5:?}]",
                        caller, user, used, refunded, unused, amount
                    ),
                    lvl,
                    &("evm", "module_evm", "modules/evm/src/lib.rs", 1795u32),
                    ::log::__private_api::Option::None,
                );
            }
        };
        let err_amount = T::ChargeTransactionPayment::unreserve_fee(
            &user,
            amount,
            Some(RESERVE_ID_STORAGE_DEPOSIT),
        );
        if true {
            if !err_amount.is_zero() {
                ::core::panicking::panic("assertion failed: err_amount.is_zero()")
            }
        }
        Ok(())
    }
    fn charge_storage(caller: &H160, contract: &H160, storage: i32) -> DispatchResult {
        if storage.is_zero() {
            return Ok(());
        }
        let user = T::AddressMapping::get_account_id(caller);
        let contract_acc = T::AddressMapping::get_account_id(contract);
        let amount = Self::get_storage_deposit_per_byte()
            .saturating_mul(storage.unsigned_abs().into());
        {
            let lvl = ::log::Level::Debug;
            if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                ::log::__private_api_log(
                    format_args!(
                        "charge_storage: [from: {0:?}, account: {1:?}, contract: {2:?}, contract_acc: {3:?}, storage: {4:?}, amount: {5:?}]",
                        caller, user, contract, contract_acc, storage, amount
                    ),
                    lvl,
                    &("evm", "module_evm", "modules/evm/src/lib.rs", 1817u32),
                    ::log::__private_api::Option::None,
                );
            }
        };
        if storage.is_positive() {
            let err_amount = T::Currency::repatriate_reserved_named(
                &RESERVE_ID_STORAGE_DEPOSIT,
                &user,
                &contract_acc,
                amount,
                BalanceStatus::Reserved,
            )?;
            if true {
                if !err_amount.is_zero() {
                    ::core::panicking::panic("assertion failed: err_amount.is_zero()")
                }
            }
        } else {
            let val = T::Currency::repatriate_reserved_named(
                &RESERVE_ID_STORAGE_DEPOSIT,
                &contract_acc,
                &user,
                amount,
                BalanceStatus::Reserved,
            )?;
            if true {
                if !val.is_zero() {
                    ::core::panicking::panic("assertion failed: val.is_zero()")
                }
            }
        };
        Ok(())
    }
    fn refund_storage(
        caller: &H160,
        contract: &H160,
        maintainer: &H160,
    ) -> DispatchResult {
        let user = T::AddressMapping::get_account_id(caller);
        let contract_acc = T::AddressMapping::get_account_id(contract);
        let maintainer_acc = T::AddressMapping::get_account_id(maintainer);
        let amount = T::Currency::reserved_balance_named(
            &RESERVE_ID_STORAGE_DEPOSIT,
            &contract_acc,
        );
        {
            let lvl = ::log::Level::Debug;
            if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                ::log::__private_api_log(
                    format_args!(
                        "refund_storage: [from: {0:?}, account: {1:?}, contract: {2:?}, contract_acc: {3:?}, maintainer: {4:?}, maintainer_acc: {5:?}, amount: {6:?}]",
                        caller, user, contract, contract_acc, maintainer, maintainer_acc,
                        amount
                    ),
                    lvl,
                    &("evm", "module_evm", "modules/evm/src/lib.rs", 1856u32),
                    ::log::__private_api::Option::None,
                );
            }
        };
        let val = T::Currency::repatriate_reserved_named(
            &RESERVE_ID_STORAGE_DEPOSIT,
            &contract_acc,
            &user,
            amount,
            BalanceStatus::Free,
        )?;
        if true {
            if !val.is_zero() {
                ::core::panicking::panic("assertion failed: val.is_zero()")
            }
        }
        let dest = if contract_acc == maintainer_acc {
            T::TreasuryAccount::get()
        } else {
            maintainer_acc
        };
        T::TransferAll::transfer_all(&contract_acc, &dest)?;
        Ok(())
    }
    fn inc_nonce_if_needed<Output>(
        origin: &H160,
        outcome: &Result<ExecutionInfo<Output>, DispatchError>,
    ) {
        if match outcome {
            Ok(ExecutionInfo { exit_reason: ExitReason::Succeed(_), .. }) => true,
            _ => false,
        } {
            return;
        }
        Accounts::<
            T,
        >::mutate(
            origin,
            |account| {
                if let Some(info) = account.as_mut() {
                    info.nonce = info.nonce.saturating_add(T::Index::one());
                } else {
                    *account = Some(AccountInfo {
                        nonce: T::Index::one(),
                        contract_info: None,
                    });
                }
            },
        );
    }
}
impl<T: Config> EVMTrait<T::AccountId> for Pallet<T> {
    type Balance = BalanceOf<T>;
    fn execute(
        context: InvokeContext,
        input: Vec<u8>,
        value: BalanceOf<T>,
        gas_limit: u64,
        storage_limit: u32,
        mode: ExecutionMode,
    ) -> Result<CallInfo, DispatchError> {
        let mut config = T::config().clone();
        if let ExecutionMode::EstimateGas = mode {
            config.estimate = true;
        }
        frame_support::storage::with_transaction(|| {
            let result = T::Runner::call(
                context.sender,
                context.origin,
                context.contract,
                input,
                value,
                gas_limit,
                storage_limit,
                ::alloc::vec::Vec::new(),
                &config,
            );
            match result {
                Ok(info) => {
                    match mode {
                        ExecutionMode::Execute => {
                            if info.exit_reason.is_succeed() {
                                Pallet::<
                                    T,
                                >::deposit_event(Event::<T>::Executed {
                                    from: context.sender,
                                    contract: context.contract,
                                    logs: info.logs.clone(),
                                    used_gas: info.used_gas.unique_saturated_into(),
                                    used_storage: info.used_storage,
                                });
                                TransactionOutcome::Commit(Ok(info))
                            } else {
                                Pallet::<
                                    T,
                                >::deposit_event(Event::<T>::ExecutedFailed {
                                    from: context.sender,
                                    contract: context.contract,
                                    exit_reason: info.exit_reason.clone(),
                                    output: info.value.clone(),
                                    logs: info.logs.clone(),
                                    used_gas: info.used_gas.unique_saturated_into(),
                                    used_storage: Default::default(),
                                });
                                TransactionOutcome::Rollback(Ok(info))
                            }
                        }
                        ExecutionMode::View | ExecutionMode::EstimateGas => {
                            TransactionOutcome::Rollback(Ok(info))
                        }
                    }
                }
                Err(e) => TransactionOutcome::Rollback(Err(e)),
            }
        })
    }
    /// Get the real origin account and charge storage rent from the origin.
    fn get_origin() -> Option<T::AccountId> {
        ExtrinsicOrigin::<T>::get()
    }
    /// Set the EVM origin
    fn set_origin(origin: T::AccountId) {
        ExtrinsicOrigin::<T>::set(Some(origin));
    }
    fn kill_origin() {
        ExtrinsicOrigin::<T>::kill();
    }
    fn push_xcm_origin(origin: T::AccountId) {
        XcmOrigin::<
            T,
        >::mutate(|o| {
            if let Some(o) = o {
                o.push(origin);
            } else {
                *o = Some(
                    <[_]>::into_vec(#[rustc_box] ::alloc::boxed::Box::new([origin])),
                );
            }
        });
    }
    fn pop_xcm_origin() {
        XcmOrigin::<
            T,
        >::mutate(|o| {
            if let Some(arr) = o {
                arr.pop();
                if arr.is_empty() {
                    *o = None;
                }
            }
        });
    }
    fn kill_xcm_origin() {
        XcmOrigin::<T>::kill();
    }
    fn get_real_or_xcm_origin() -> Option<T::AccountId> {
        ExtrinsicOrigin::<T>::get()
            .or_else(|| XcmOrigin::<T>::get().and_then(|o| o.last().cloned()))
    }
}
pub struct EvmChainId<T>(PhantomData<T>);
impl<T: Config> Get<u64> for EvmChainId<T> {
    fn get() -> u64 {
        Pallet::<T>::chain_id()
    }
}
impl<T: Config> EVMManager<T::AccountId, BalanceOf<T>> for Pallet<T> {
    fn query_new_contract_extra_bytes() -> u32 {
        T::NewContractExtraBytes::get()
    }
    fn query_storage_deposit_per_byte() -> BalanceOf<T> {
        T::StorageDepositPerByte::get()
    }
    fn query_maintainer(contract: EvmAddress) -> Result<EvmAddress, DispatchError> {
        Accounts::<T>::get(contract)
            .map_or(
                Err(Error::<T>::ContractNotFound.into()),
                |account_info| {
                    account_info
                        .contract_info
                        .map_or(
                            Err(Error::<T>::ContractNotFound.into()),
                            |v| Ok(v.maintainer),
                        )
                },
            )
    }
    fn query_developer_deposit() -> BalanceOf<T> {
        convert_decimals_to_evm(T::DeveloperDeposit::get())
    }
    fn query_publication_fee() -> BalanceOf<T> {
        convert_decimals_to_evm(T::PublicationFee::get())
    }
    fn transfer_maintainer(
        from: T::AccountId,
        contract: EvmAddress,
        new_maintainer: EvmAddress,
    ) -> DispatchResult {
        Pallet::<T>::do_transfer_maintainer(from, contract, new_maintainer)
    }
    fn publish_contract_precompile(who: T::AccountId, contract: H160) -> DispatchResult {
        Pallet::<T>::do_publish_contract(who, contract)
    }
    fn query_developer_status(who: T::AccountId) -> bool {
        !T::Currency::reserved_balance_named(&RESERVE_ID_DEVELOPER_DEPOSIT, &who)
            .is_zero()
    }
    fn enable_account_contract_development(who: T::AccountId) -> DispatchResult {
        Pallet::<T>::do_enable_contract_development(&who)
    }
    fn disable_account_contract_development(
        who: T::AccountId,
    ) -> sp_runtime::DispatchResult {
        Pallet::<T>::do_disable_contract_development(&who)
    }
}
pub struct CallKillAccount<T>(PhantomData<T>);
impl<T: Config> OnKilledAccount<T::AccountId> for CallKillAccount<T> {
    fn on_killed_account(who: &T::AccountId) {
        if let Some(address) = T::AddressMapping::get_evm_address(who) {
            Pallet::<T>::remove_account_if_empty(&address);
        }
    }
}
pub fn code_hash(code: &[u8]) -> H256 {
    H256::from_slice(Keccak256::digest(code).as_slice())
}
#[allow(dead_code)]
fn encode_revert_message(msg: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(68 + msg.len());
    data.extend_from_slice(&[0u8; 68]);
    U256::from(msg.len()).to_big_endian(&mut data[36..68]);
    data.extend_from_slice(msg);
    data
}
#[scale_info(skip_type_params(T))]
pub struct SetEvmOrigin<T: Config + Send + Sync>(PhantomData<T>);
#[allow(deprecated)]
const _: () = {
    #[automatically_derived]
    impl<T: Config + Send + Sync> ::codec::Encode for SetEvmOrigin<T>
    where
        PhantomData<T>: ::codec::Encode,
        PhantomData<T>: ::codec::Encode,
    {
        fn size_hint(&self) -> usize {
            ::codec::Encode::size_hint(&&self.0)
        }
        fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
            &self,
            __codec_dest_edqy: &mut __CodecOutputEdqy,
        ) {
            ::codec::Encode::encode_to(&&self.0, __codec_dest_edqy)
        }
        fn encode(&self) -> ::codec::alloc::vec::Vec<::core::primitive::u8> {
            ::codec::Encode::encode(&&self.0)
        }
        fn using_encoded<R, F: ::core::ops::FnOnce(&[::core::primitive::u8]) -> R>(
            &self,
            f: F,
        ) -> R {
            ::codec::Encode::using_encoded(&&self.0, f)
        }
    }
    #[automatically_derived]
    impl<T: Config + Send + Sync> ::codec::EncodeLike for SetEvmOrigin<T>
    where
        PhantomData<T>: ::codec::Encode,
        PhantomData<T>: ::codec::Encode,
    {}
};
#[allow(deprecated)]
const _: () = {
    #[automatically_derived]
    impl<T: Config + Send + Sync> ::codec::Decode for SetEvmOrigin<T>
    where
        PhantomData<T>: ::codec::Decode,
        PhantomData<T>: ::codec::Decode,
    {
        fn decode<__CodecInputEdqy: ::codec::Input>(
            __codec_input_edqy: &mut __CodecInputEdqy,
        ) -> ::core::result::Result<Self, ::codec::Error> {
            ::core::result::Result::Ok(
                SetEvmOrigin::<
                    T,
                >({
                    let __codec_res_edqy = <PhantomData<
                        T,
                    > as ::codec::Decode>::decode(__codec_input_edqy);
                    match __codec_res_edqy {
                        ::core::result::Result::Err(e) => {
                            return ::core::result::Result::Err(
                                e.chain("Could not decode `SetEvmOrigin.0`"),
                            );
                        }
                        ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                    }
                }),
            )
        }
    }
};
#[automatically_derived]
impl<T: ::core::clone::Clone + Config + Send + Sync> ::core::clone::Clone
for SetEvmOrigin<T> {
    #[inline]
    fn clone(&self) -> SetEvmOrigin<T> {
        SetEvmOrigin(::core::clone::Clone::clone(&self.0))
    }
}
#[automatically_derived]
impl<T: Config + Send + Sync> ::core::marker::StructuralEq for SetEvmOrigin<T> {}
#[automatically_derived]
impl<T: ::core::cmp::Eq + Config + Send + Sync> ::core::cmp::Eq for SetEvmOrigin<T> {
    #[inline]
    #[doc(hidden)]
    #[no_coverage]
    fn assert_receiver_is_total_eq(&self) -> () {
        let _: ::core::cmp::AssertParamIsEq<PhantomData<T>>;
    }
}
#[automatically_derived]
impl<T: Config + Send + Sync> ::core::marker::StructuralPartialEq for SetEvmOrigin<T> {}
#[automatically_derived]
impl<T: ::core::cmp::PartialEq + Config + Send + Sync> ::core::cmp::PartialEq
for SetEvmOrigin<T> {
    #[inline]
    fn eq(&self, other: &SetEvmOrigin<T>) -> bool {
        self.0 == other.0
    }
}
#[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
const _: () = {
    impl<T: Config + Send + Sync> ::scale_info::TypeInfo for SetEvmOrigin<T>
    where
        PhantomData<T>: ::scale_info::TypeInfo + 'static,
        T: Config + Send + Sync + 'static,
    {
        type Identity = Self;
        fn type_info() -> ::scale_info::Type {
            ::scale_info::Type::builder()
                .path(::scale_info::Path::new("SetEvmOrigin", "module_evm"))
                .type_params(
                    <[_]>::into_vec(
                        #[rustc_box]
                        ::alloc::boxed::Box::new([
                            ::scale_info::TypeParameter::new(
                                "T",
                                ::core::option::Option::None,
                            ),
                        ]),
                    ),
                )
                .composite(
                    ::scale_info::build::Fields::unnamed()
                        .field(|f| f.ty::<PhantomData<T>>().type_name("PhantomData<T>")),
                )
        }
    }
};
impl<T: Config + Send + Sync> sp_std::fmt::Debug for SetEvmOrigin<T> {
    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        Ok(())
    }
}
impl<T: Config + Send + Sync> SetEvmOrigin<T> {
    pub fn new() -> Self {
        Self(sp_std::marker::PhantomData)
    }
}
impl<T: Config + Send + Sync> Default for SetEvmOrigin<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T: Config + Send + Sync> SignedExtension for SetEvmOrigin<T> {
    const IDENTIFIER: &'static str = "SetEvmOrigin";
    type AccountId = T::AccountId;
    type Call = T::RuntimeCall;
    type AdditionalSigned = ();
    type Pre = ();
    fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> {
        Ok(())
    }
    fn validate(
        &self,
        who: &Self::AccountId,
        _call: &Self::Call,
        _info: &DispatchInfoOf<Self::Call>,
        _len: usize,
    ) -> TransactionValidity {
        ExtrinsicOrigin::<T>::set(Some(who.clone()));
        Ok(ValidTransaction::default())
    }
    fn pre_dispatch(
        self,
        who: &Self::AccountId,
        _call: &Self::Call,
        _info: &DispatchInfoOf<Self::Call>,
        _len: usize,
    ) -> Result<(), TransactionValidityError> {
        ExtrinsicOrigin::<T>::set(Some(who.clone()));
        Ok(())
    }
    fn post_dispatch(
        _pre: Option<Self::Pre>,
        _info: &DispatchInfoOf<Self::Call>,
        _post_info: &PostDispatchInfoOf<Self::Call>,
        _len: usize,
        _result: &DispatchResult,
    ) -> Result<(), TransactionValidityError> {
        ExtrinsicOrigin::<T>::kill();
        XcmOrigin::<T>::kill();
        Ok(())
    }
}
pub enum EvmTask<T: Config> {
    Schedule {
        from: EvmAddress,
        target: EvmAddress,
        input: Vec<u8>,
        value: BalanceOf<T>,
        gas_limit: u64,
        storage_limit: u32,
    },
    Remove { caller: EvmAddress, contract: EvmAddress, maintainer: EvmAddress },
}
#[automatically_derived]
impl<T: ::core::clone::Clone + Config> ::core::clone::Clone for EvmTask<T> {
    #[inline]
    fn clone(&self) -> EvmTask<T> {
        match self {
            EvmTask::Schedule {
                from: __self_0,
                target: __self_1,
                input: __self_2,
                value: __self_3,
                gas_limit: __self_4,
                storage_limit: __self_5,
            } => {
                EvmTask::Schedule {
                    from: ::core::clone::Clone::clone(__self_0),
                    target: ::core::clone::Clone::clone(__self_1),
                    input: ::core::clone::Clone::clone(__self_2),
                    value: ::core::clone::Clone::clone(__self_3),
                    gas_limit: ::core::clone::Clone::clone(__self_4),
                    storage_limit: ::core::clone::Clone::clone(__self_5),
                }
            }
            EvmTask::Remove {
                caller: __self_0,
                contract: __self_1,
                maintainer: __self_2,
            } => {
                EvmTask::Remove {
                    caller: ::core::clone::Clone::clone(__self_0),
                    contract: ::core::clone::Clone::clone(__self_1),
                    maintainer: ::core::clone::Clone::clone(__self_2),
                }
            }
        }
    }
}
impl<T: Config> core::fmt::Debug for EvmTask<T>
where
    T: core::fmt::Debug,
{
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        fmt.write_str("<wasm:stripped>")
    }
}
#[automatically_derived]
impl<T: Config> ::core::marker::StructuralPartialEq for EvmTask<T> {}
#[automatically_derived]
impl<T: ::core::cmp::PartialEq + Config> ::core::cmp::PartialEq for EvmTask<T> {
    #[inline]
    fn eq(&self, other: &EvmTask<T>) -> bool {
        let __self_tag = ::core::intrinsics::discriminant_value(self);
        let __arg1_tag = ::core::intrinsics::discriminant_value(other);
        __self_tag == __arg1_tag
            && match (self, other) {
                (
                    EvmTask::Schedule {
                        from: __self_0,
                        target: __self_1,
                        input: __self_2,
                        value: __self_3,
                        gas_limit: __self_4,
                        storage_limit: __self_5,
                    },
                    EvmTask::Schedule {
                        from: __arg1_0,
                        target: __arg1_1,
                        input: __arg1_2,
                        value: __arg1_3,
                        gas_limit: __arg1_4,
                        storage_limit: __arg1_5,
                    },
                ) => {
                    *__self_0 == *__arg1_0 && *__self_1 == *__arg1_1
                        && *__self_2 == *__arg1_2 && *__self_3 == *__arg1_3
                        && *__self_4 == *__arg1_4 && *__self_5 == *__arg1_5
                }
                (
                    EvmTask::Remove {
                        caller: __self_0,
                        contract: __self_1,
                        maintainer: __self_2,
                    },
                    EvmTask::Remove {
                        caller: __arg1_0,
                        contract: __arg1_1,
                        maintainer: __arg1_2,
                    },
                ) => {
                    *__self_0 == *__arg1_0 && *__self_1 == *__arg1_1
                        && *__self_2 == *__arg1_2
                }
                _ => unsafe { ::core::intrinsics::unreachable() }
            }
    }
}
#[allow(deprecated)]
const _: () = {
    #[automatically_derived]
    impl<T: Config> ::codec::Encode for EvmTask<T>
    where
        BalanceOf<T>: ::codec::Encode,
        BalanceOf<T>: ::codec::Encode,
    {
        fn size_hint(&self) -> usize {
            1_usize
                + match *self {
                    EvmTask::Schedule {
                        ref from,
                        ref target,
                        ref input,
                        ref value,
                        ref gas_limit,
                        ref storage_limit,
                    } => {
                        0_usize
                            .saturating_add(::codec::Encode::size_hint(from))
                            .saturating_add(::codec::Encode::size_hint(target))
                            .saturating_add(::codec::Encode::size_hint(input))
                            .saturating_add(::codec::Encode::size_hint(value))
                            .saturating_add(::codec::Encode::size_hint(gas_limit))
                            .saturating_add(::codec::Encode::size_hint(storage_limit))
                    }
                    EvmTask::Remove { ref caller, ref contract, ref maintainer } => {
                        0_usize
                            .saturating_add(::codec::Encode::size_hint(caller))
                            .saturating_add(::codec::Encode::size_hint(contract))
                            .saturating_add(::codec::Encode::size_hint(maintainer))
                    }
                    _ => 0_usize,
                }
        }
        fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
            &self,
            __codec_dest_edqy: &mut __CodecOutputEdqy,
        ) {
            match *self {
                EvmTask::Schedule {
                    ref from,
                    ref target,
                    ref input,
                    ref value,
                    ref gas_limit,
                    ref storage_limit,
                } => {
                    __codec_dest_edqy.push_byte(0usize as ::core::primitive::u8);
                    ::codec::Encode::encode_to(from, __codec_dest_edqy);
                    ::codec::Encode::encode_to(target, __codec_dest_edqy);
                    ::codec::Encode::encode_to(input, __codec_dest_edqy);
                    ::codec::Encode::encode_to(value, __codec_dest_edqy);
                    ::codec::Encode::encode_to(gas_limit, __codec_dest_edqy);
                    ::codec::Encode::encode_to(storage_limit, __codec_dest_edqy);
                }
                EvmTask::Remove { ref caller, ref contract, ref maintainer } => {
                    __codec_dest_edqy.push_byte(1usize as ::core::primitive::u8);
                    ::codec::Encode::encode_to(caller, __codec_dest_edqy);
                    ::codec::Encode::encode_to(contract, __codec_dest_edqy);
                    ::codec::Encode::encode_to(maintainer, __codec_dest_edqy);
                }
                _ => {}
            }
        }
    }
    #[automatically_derived]
    impl<T: Config> ::codec::EncodeLike for EvmTask<T>
    where
        BalanceOf<T>: ::codec::Encode,
        BalanceOf<T>: ::codec::Encode,
    {}
};
#[allow(deprecated)]
const _: () = {
    #[automatically_derived]
    impl<T: Config> ::codec::Decode for EvmTask<T>
    where
        BalanceOf<T>: ::codec::Decode,
        BalanceOf<T>: ::codec::Decode,
    {
        fn decode<__CodecInputEdqy: ::codec::Input>(
            __codec_input_edqy: &mut __CodecInputEdqy,
        ) -> ::core::result::Result<Self, ::codec::Error> {
            match __codec_input_edqy
                .read_byte()
                .map_err(|e| {
                    e.chain("Could not decode `EvmTask`, failed to read variant byte")
                })?
            {
                #[allow(clippy::unnecessary_cast)]
                __codec_x_edqy if __codec_x_edqy == 0usize as ::core::primitive::u8 => {
                    #[allow(clippy::redundant_closure_call)]
                    return (move || {
                        ::core::result::Result::Ok(EvmTask::<T>::Schedule {
                            from: {
                                let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                    __codec_input_edqy,
                                );
                                match __codec_res_edqy {
                                    ::core::result::Result::Err(e) => {
                                        return ::core::result::Result::Err(
                                            e.chain("Could not decode `EvmTask::Schedule::from`"),
                                        );
                                    }
                                    ::core::result::Result::Ok(__codec_res_edqy) => {
                                        __codec_res_edqy
                                    }
                                }
                            },
                            target: {
                                let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                    __codec_input_edqy,
                                );
                                match __codec_res_edqy {
                                    ::core::result::Result::Err(e) => {
                                        return ::core::result::Result::Err(
                                            e.chain("Could not decode `EvmTask::Schedule::target`"),
                                        );
                                    }
                                    ::core::result::Result::Ok(__codec_res_edqy) => {
                                        __codec_res_edqy
                                    }
                                }
                            },
                            input: {
                                let __codec_res_edqy = <Vec<
                                    u8,
                                > as ::codec::Decode>::decode(__codec_input_edqy);
                                match __codec_res_edqy {
                                    ::core::result::Result::Err(e) => {
                                        return ::core::result::Result::Err(
                                            e.chain("Could not decode `EvmTask::Schedule::input`"),
                                        );
                                    }
                                    ::core::result::Result::Ok(__codec_res_edqy) => {
                                        __codec_res_edqy
                                    }
                                }
                            },
                            value: {
                                let __codec_res_edqy = <BalanceOf<
                                    T,
                                > as ::codec::Decode>::decode(__codec_input_edqy);
                                match __codec_res_edqy {
                                    ::core::result::Result::Err(e) => {
                                        return ::core::result::Result::Err(
                                            e.chain("Could not decode `EvmTask::Schedule::value`"),
                                        );
                                    }
                                    ::core::result::Result::Ok(__codec_res_edqy) => {
                                        __codec_res_edqy
                                    }
                                }
                            },
                            gas_limit: {
                                let __codec_res_edqy = <u64 as ::codec::Decode>::decode(
                                    __codec_input_edqy,
                                );
                                match __codec_res_edqy {
                                    ::core::result::Result::Err(e) => {
                                        return ::core::result::Result::Err(
                                            e.chain("Could not decode `EvmTask::Schedule::gas_limit`"),
                                        );
                                    }
                                    ::core::result::Result::Ok(__codec_res_edqy) => {
                                        __codec_res_edqy
                                    }
                                }
                            },
                            storage_limit: {
                                let __codec_res_edqy = <u32 as ::codec::Decode>::decode(
                                    __codec_input_edqy,
                                );
                                match __codec_res_edqy {
                                    ::core::result::Result::Err(e) => {
                                        return ::core::result::Result::Err(
                                            e
                                                .chain(
                                                    "Could not decode `EvmTask::Schedule::storage_limit`",
                                                ),
                                        );
                                    }
                                    ::core::result::Result::Ok(__codec_res_edqy) => {
                                        __codec_res_edqy
                                    }
                                }
                            },
                        })
                    })();
                }
                #[allow(clippy::unnecessary_cast)]
                __codec_x_edqy if __codec_x_edqy == 1usize as ::core::primitive::u8 => {
                    #[allow(clippy::redundant_closure_call)]
                    return (move || {
                        ::core::result::Result::Ok(EvmTask::<T>::Remove {
                            caller: {
                                let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                    __codec_input_edqy,
                                );
                                match __codec_res_edqy {
                                    ::core::result::Result::Err(e) => {
                                        return ::core::result::Result::Err(
                                            e.chain("Could not decode `EvmTask::Remove::caller`"),
                                        );
                                    }
                                    ::core::result::Result::Ok(__codec_res_edqy) => {
                                        __codec_res_edqy
                                    }
                                }
                            },
                            contract: {
                                let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                    __codec_input_edqy,
                                );
                                match __codec_res_edqy {
                                    ::core::result::Result::Err(e) => {
                                        return ::core::result::Result::Err(
                                            e.chain("Could not decode `EvmTask::Remove::contract`"),
                                        );
                                    }
                                    ::core::result::Result::Ok(__codec_res_edqy) => {
                                        __codec_res_edqy
                                    }
                                }
                            },
                            maintainer: {
                                let __codec_res_edqy = <EvmAddress as ::codec::Decode>::decode(
                                    __codec_input_edqy,
                                );
                                match __codec_res_edqy {
                                    ::core::result::Result::Err(e) => {
                                        return ::core::result::Result::Err(
                                            e.chain("Could not decode `EvmTask::Remove::maintainer`"),
                                        );
                                    }
                                    ::core::result::Result::Ok(__codec_res_edqy) => {
                                        __codec_res_edqy
                                    }
                                }
                            },
                        })
                    })();
                }
                _ => {
                    #[allow(clippy::redundant_closure_call)]
                    return (move || {
                        ::core::result::Result::Err(
                            <_ as ::core::convert::Into<
                                _,
                            >>::into("Could not decode `EvmTask`, variant doesn't exist"),
                        )
                    })();
                }
            }
        }
    }
};
#[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
const _: () = {
    impl<T: Config> ::scale_info::TypeInfo for EvmTask<T>
    where
        BalanceOf<T>: ::scale_info::TypeInfo + 'static,
        T: Config + ::scale_info::TypeInfo + 'static,
    {
        type Identity = Self;
        fn type_info() -> ::scale_info::Type {
            ::scale_info::Type::builder()
                .path(::scale_info::Path::new("EvmTask", "module_evm"))
                .type_params(
                    <[_]>::into_vec(
                        #[rustc_box]
                        ::alloc::boxed::Box::new([
                            ::scale_info::TypeParameter::new(
                                "T",
                                ::core::option::Option::Some(::scale_info::meta_type::<T>()),
                            ),
                        ]),
                    ),
                )
                .variant(
                    ::scale_info::build::Variants::new()
                        .variant(
                            "Schedule",
                            |v| {
                                v
                                    .index(0usize as ::core::primitive::u8)
                                    .fields(
                                        ::scale_info::build::Fields::named()
                                            .field(|f| {
                                                f.ty::<EvmAddress>().name("from").type_name("EvmAddress")
                                            })
                                            .field(|f| {
                                                f.ty::<EvmAddress>().name("target").type_name("EvmAddress")
                                            })
                                            .field(|f| {
                                                f.ty::<Vec<u8>>().name("input").type_name("Vec<u8>")
                                            })
                                            .field(|f| {
                                                f
                                                    .ty::<BalanceOf<T>>()
                                                    .name("value")
                                                    .type_name("BalanceOf<T>")
                                            })
                                            .field(|f| f.ty::<u64>().name("gas_limit").type_name("u64"))
                                            .field(|f| {
                                                f.ty::<u32>().name("storage_limit").type_name("u32")
                                            }),
                                    )
                            },
                        )
                        .variant(
                            "Remove",
                            |v| {
                                v
                                    .index(1usize as ::core::primitive::u8)
                                    .fields(
                                        ::scale_info::build::Fields::named()
                                            .field(|f| {
                                                f.ty::<EvmAddress>().name("caller").type_name("EvmAddress")
                                            })
                                            .field(|f| {
                                                f
                                                    .ty::<EvmAddress>()
                                                    .name("contract")
                                                    .type_name("EvmAddress")
                                            })
                                            .field(|f| {
                                                f
                                                    .ty::<EvmAddress>()
                                                    .name("maintainer")
                                                    .type_name("EvmAddress")
                                            }),
                                    )
                            },
                        ),
                )
        }
    }
};
impl<T: Config> DispatchableTask for EvmTask<T> {
    fn dispatch(self, weight: Weight) -> TaskResult {
        match self {
            EvmTask::Schedule { .. } => {
                TaskResult {
                    result: Ok(()),
                    used_weight: Weight::zero(),
                    finished: false,
                }
            }
            EvmTask::Remove { caller, contract, maintainer } => {
                let limit: u32 = cmp::min(
                    weight
                        .ref_time()
                        .checked_div(<T as frame_system::Config>::DbWeight::get().write)
                        .unwrap_or(REMOVE_LIMIT.into())
                        .saturated_into(),
                    REMOVE_LIMIT,
                );
                let r = <AccountStorages<T>>::clear_prefix(contract, limit, None);
                let count = r.backend;
                let used_weight = Weight::from_parts(
                    <T as frame_system::Config>::DbWeight::get()
                        .write
                        .saturating_mul(count.into()),
                    0,
                );
                {
                    let lvl = ::log::Level::Debug;
                    if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                        ::log::__private_api_log(
                            format_args!(
                                "EvmTask remove: [from: {0:?}, contract: {1:?}, maintainer: {2:?}, count: {3:?}]",
                                caller, contract, maintainer, count
                            ),
                            lvl,
                            &("evm", "module_evm", "modules/evm/src/lib.rs", 2224u32),
                            ::log::__private_api::Option::None,
                        );
                    }
                };
                if r.maybe_cursor.is_none() {
                    let result = Pallet::<
                        T,
                    >::refund_storage(&caller, &contract, &maintainer);
                    if true {
                        if !result.is_ok() {
                            ::core::panicking::panic("assertion failed: result.is_ok()")
                        }
                    }
                    {
                        let lvl = ::log::Level::Debug;
                        if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                            ::log::__private_api_log(
                                format_args!(
                                    "EvmTask refund_storage: [from: {0:?}, contract: {1:?}, maintainer: {2:?}, result: {3:?}]",
                                    caller, contract, maintainer, result
                                ),
                                lvl,
                                &("evm", "module_evm", "modules/evm/src/lib.rs", 2234u32),
                                ::log::__private_api::Option::None,
                            );
                        }
                    };
                    Pallet::<T>::remove_account(&contract);
                    TaskResult {
                        result,
                        used_weight,
                        finished: true,
                    }
                } else {
                    TaskResult {
                        result: Ok(()),
                        used_weight,
                        finished: false,
                    }
                }
            }
        }
    }
}
