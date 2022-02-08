
#![feature(prelude_import)]
//! # Module RelayChain
//!
//! This module is in charge of handling relaychain related utilities and business logic.
#![allow(clippy::unused_unit)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
use codec::{Decode, Encode, FullCodec};
use sp_runtime::traits::StaticLookup;
use frame_support::{traits::Get, weights::Weight, RuntimeDebug};
use module_support::CallBuilder;
use primitives::Balance;
use sp_std::{boxed::Box, marker::PhantomData, prelude::*};
pub use cumulus_primitives_core::ParaId;
use xcm::latest::prelude::*;
use frame_system::Config;
pub enum ProxyType {
    #[codec(index = 0)]
    Any,
}
const _: () = {
    impl ::codec::Encode for ProxyType {
        fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
            &self,
            __codec_dest_edqy: &mut __CodecOutputEdqy,
        ) {
            match *self {
                ProxyType::Any => {
                    __codec_dest_edqy.push_byte(0u8 as ::core::primitive::u8);
                }
                _ => (),
            }
        }
    }
    impl ::codec::EncodeLike for ProxyType {}
};
const _: () = {
    impl ::codec::Decode for ProxyType {
        fn decode<__CodecInputEdqy: ::codec::Input>(
            __codec_input_edqy: &mut __CodecInputEdqy,
        ) -> ::core::result::Result<Self, ::codec::Error> {
            match __codec_input_edqy
                .read_byte()
                .map_err(|e| e.chain("Could not decode `ProxyType`, failed to read variant byte"))?
            {
                __codec_x_edqy if __codec_x_edqy == 0u8 as ::core::primitive::u8 => {
                    ::core::result::Result::Ok(ProxyType::Any)
                }
                _ => ::core::result::Result::Err(<_ as ::core::convert::Into<_>>::into(
                    "Could not decode `ProxyType`, variant doesn\'t exist",
                )),
            }
        }
    }
};
impl core::fmt::Debug for ProxyType {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::Any => fmt.debug_tuple("ProxyType::Any").finish(),
            _ => Ok(()),
        }
    }
}
pub enum BalancesCall<T: Config> {
    #[codec(index = 3)]
    TransferKeepAlive(
        <T::Lookup as StaticLookup>::Source,
        #[codec(compact)] Balance,
    ),
}
const _: () = {
    impl<T: Config> ::codec::Encode for BalancesCall<T>
    where
        <T::Lookup as StaticLookup>::Source: ::codec::Encode,
        <T::Lookup as StaticLookup>::Source: ::codec::Encode,
    {
        fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
            &self,
            __codec_dest_edqy: &mut __CodecOutputEdqy,
        ) {
            match *self {
                BalancesCall::TransferKeepAlive(ref aa, ref ba) => {
                    __codec_dest_edqy.push_byte(3u8 as ::core::primitive::u8);
                    ::codec::Encode::encode_to(aa, __codec_dest_edqy);
                    {
                        ::codec::Encode::encode_to(
                            &<<Balance as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                '_,
                                Balance,
                            >>::RefType::from(ba),
                            __codec_dest_edqy,
                        );
                    }
                }
                _ => (),
            }
        }
    }
    impl<T: Config> ::codec::EncodeLike for BalancesCall<T>
    where
        <T::Lookup as StaticLookup>::Source: ::codec::Encode,
        <T::Lookup as StaticLookup>::Source: ::codec::Encode,
    {
    }
};
const _: () = {
    impl<T: Config> ::codec::Decode for BalancesCall<T>
    where
        <T::Lookup as StaticLookup>::Source: ::codec::Decode,
        <T::Lookup as StaticLookup>::Source: ::codec::Decode,
    {
        fn decode<__CodecInputEdqy: ::codec::Input>(
            __codec_input_edqy: &mut __CodecInputEdqy,
        ) -> ::core::result::Result<Self, ::codec::Error> {
            match __codec_input_edqy.read_byte().map_err(|e| {
                e.chain("Could not decode `BalancesCall`, failed to read variant byte")
            })? {
                __codec_x_edqy if __codec_x_edqy == 3u8 as ::core::primitive::u8 => {
                    ::core::result::Result::Ok(BalancesCall::<T>::TransferKeepAlive(
                        {
                            let __codec_res_edqy =
                                <<T::Lookup as StaticLookup>::Source as ::codec::Decode>::decode(
                                    __codec_input_edqy,
                                );
                            match __codec_res_edqy {
                                ::core::result::Result::Err(e) => {
                                    return ::core::result::Result::Err(e.chain(
                                        "Could not decode `BalancesCall::TransferKeepAlive.0`",
                                    ))
                                }
                                ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                            }
                        },
                        {
                            let __codec_res_edqy =
                                <<Balance as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                    __codec_input_edqy,
                                );
                            match __codec_res_edqy {
                                ::core::result::Result::Err(e) => {
                                    return ::core::result::Result::Err(e.chain(
                                        "Could not decode `BalancesCall::TransferKeepAlive.1`",
                                    ))
                                }
                                ::core::result::Result::Ok(__codec_res_edqy) => {
                                    __codec_res_edqy.into()
                                }
                            }
                        },
                    ))
                }
                _ => ::core::result::Result::Err(<_ as ::core::convert::Into<_>>::into(
                    "Could not decode `BalancesCall`, variant doesn\'t exist",
                )),
            }
        }
    }
};
impl<T: Config> core::fmt::Debug for BalancesCall<T>
where
    T: core::fmt::Debug,
{
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::TransferKeepAlive(ref a0, ref a1) => fmt
                .debug_tuple("BalancesCall::TransferKeepAlive")
                .field(a0)
                .field(a1)
                .finish(),
            _ => Ok(()),
        }
    }
}
pub enum UtilityCall<RelayChainCall> {
    #[codec(index = 1)]
    AsDerivative(u16, RelayChainCall),
    #[codec(index = 2)]
    BatchAll(Vec<RelayChainCall>),
}
const _: () = {
    impl<RelayChainCall> ::codec::Encode for UtilityCall<RelayChainCall>
    where
        RelayChainCall: ::codec::Encode,
        RelayChainCall: ::codec::Encode,
        Vec<RelayChainCall>: ::codec::Encode,
        Vec<RelayChainCall>: ::codec::Encode,
    {
        fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
            &self,
            __codec_dest_edqy: &mut __CodecOutputEdqy,
        ) {
            match *self {
                UtilityCall::AsDerivative(ref aa, ref ba) => {
                    __codec_dest_edqy.push_byte(1u8 as ::core::primitive::u8);
                    ::codec::Encode::encode_to(aa, __codec_dest_edqy);
                    ::codec::Encode::encode_to(ba, __codec_dest_edqy);
                }
                UtilityCall::BatchAll(ref aa) => {
                    __codec_dest_edqy.push_byte(2u8 as ::core::primitive::u8);
                    ::codec::Encode::encode_to(aa, __codec_dest_edqy);
                }
                _ => (),
            }
        }
    }
    impl<RelayChainCall> ::codec::EncodeLike for UtilityCall<RelayChainCall>
    where
        RelayChainCall: ::codec::Encode,
        RelayChainCall: ::codec::Encode,
        Vec<RelayChainCall>: ::codec::Encode,
        Vec<RelayChainCall>: ::codec::Encode,
    {
    }
};
const _: () = {
    impl<RelayChainCall> ::codec::Decode for UtilityCall<RelayChainCall>
    where
        RelayChainCall: ::codec::Decode,
        RelayChainCall: ::codec::Decode,
        Vec<RelayChainCall>: ::codec::Decode,
        Vec<RelayChainCall>: ::codec::Decode,
    {
        fn decode<__CodecInputEdqy: ::codec::Input>(
            __codec_input_edqy: &mut __CodecInputEdqy,
        ) -> ::core::result::Result<Self, ::codec::Error> {
            match __codec_input_edqy.read_byte().map_err(|e| {
                e.chain("Could not decode `UtilityCall`, failed to read variant byte")
            })? {
                __codec_x_edqy if __codec_x_edqy == 1u8 as ::core::primitive::u8 => {
                    ::core::result::Result::Ok(UtilityCall::<RelayChainCall>::AsDerivative(
                        {
                            let __codec_res_edqy =
                                <u16 as ::codec::Decode>::decode(__codec_input_edqy);
                            match __codec_res_edqy {
                                ::core::result::Result::Err(e) => {
                                    return ::core::result::Result::Err(
                                        e.chain("Could not decode `UtilityCall::AsDerivative.0`"),
                                    )
                                }
                                ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                            }
                        },
                        {
                            let __codec_res_edqy =
                                <RelayChainCall as ::codec::Decode>::decode(__codec_input_edqy);
                            match __codec_res_edqy {
                                ::core::result::Result::Err(e) => {
                                    return ::core::result::Result::Err(
                                        e.chain("Could not decode `UtilityCall::AsDerivative.1`"),
                                    )
                                }
                                ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                            }
                        },
                    ))
                }
                __codec_x_edqy if __codec_x_edqy == 2u8 as ::core::primitive::u8 => {
                    ::core::result::Result::Ok(UtilityCall::<RelayChainCall>::BatchAll({
                        let __codec_res_edqy =
                            <Vec<RelayChainCall> as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `UtilityCall::BatchAll.0`"),
                                )
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                        }
                    }))
                }
                _ => ::core::result::Result::Err(<_ as ::core::convert::Into<_>>::into(
                    "Could not decode `UtilityCall`, variant doesn\'t exist",
                )),
            }
        }
    }
};
impl<RelayChainCall> core::fmt::Debug for UtilityCall<RelayChainCall>
where
    RelayChainCall: core::fmt::Debug,
{
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::AsDerivative(ref a0, ref a1) => fmt
                .debug_tuple("UtilityCall::AsDerivative")
                .field(a0)
                .field(a1)
                .finish(),
            Self::BatchAll(ref a0) => fmt.debug_tuple("UtilityCall::BatchAll").field(a0).finish(),
            _ => Ok(()),
        }
    }
}
pub enum StakingCall {
    #[codec(index = 1)]
    BondExtra(#[codec(compact)] Balance),
    #[codec(index = 2)]
    Unbond(#[codec(compact)] Balance),
    #[codec(index = 3)]
    WithdrawUnbonded(u32),
}
const _: () = {
    impl ::codec::Encode for StakingCall {
        fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
            &self,
            __codec_dest_edqy: &mut __CodecOutputEdqy,
        ) {
            match *self {
                StakingCall::BondExtra(ref aa) => {
                    __codec_dest_edqy.push_byte(1u8 as ::core::primitive::u8);
                    {
                        ::codec::Encode::encode_to(
                            &<<Balance as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                '_,
                                Balance,
                            >>::RefType::from(aa),
                            __codec_dest_edqy,
                        );
                    }
                }
                StakingCall::Unbond(ref aa) => {
                    __codec_dest_edqy.push_byte(2u8 as ::core::primitive::u8);
                    {
                        ::codec::Encode::encode_to(
                            &<<Balance as ::codec::HasCompact>::Type as ::codec::EncodeAsRef<
                                '_,
                                Balance,
                            >>::RefType::from(aa),
                            __codec_dest_edqy,
                        );
                    }
                }
                StakingCall::WithdrawUnbonded(ref aa) => {
                    __codec_dest_edqy.push_byte(3u8 as ::core::primitive::u8);
                    ::codec::Encode::encode_to(aa, __codec_dest_edqy);
                }
                _ => (),
            }
        }
    }
    impl ::codec::EncodeLike for StakingCall {}
};
const _: () = {
    impl ::codec::Decode for StakingCall {
        fn decode<__CodecInputEdqy: ::codec::Input>(
            __codec_input_edqy: &mut __CodecInputEdqy,
        ) -> ::core::result::Result<Self, ::codec::Error> {
            match __codec_input_edqy.read_byte().map_err(|e| {
                e.chain("Could not decode `StakingCall`, failed to read variant byte")
            })? {
                __codec_x_edqy if __codec_x_edqy == 1u8 as ::core::primitive::u8 => {
                    ::core::result::Result::Ok(StakingCall::BondExtra({
                        let __codec_res_edqy =
                            <<Balance as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                __codec_input_edqy,
                            );
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `StakingCall::BondExtra.0`"),
                                )
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy.into(),
                        }
                    }))
                }
                __codec_x_edqy if __codec_x_edqy == 2u8 as ::core::primitive::u8 => {
                    ::core::result::Result::Ok(StakingCall::Unbond({
                        let __codec_res_edqy =
                            <<Balance as ::codec::HasCompact>::Type as ::codec::Decode>::decode(
                                __codec_input_edqy,
                            );
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `StakingCall::Unbond.0`"),
                                )
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy.into(),
                        }
                    }))
                }
                __codec_x_edqy if __codec_x_edqy == 3u8 as ::core::primitive::u8 => {
                    ::core::result::Result::Ok(StakingCall::WithdrawUnbonded({
                        let __codec_res_edqy = <u32 as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `StakingCall::WithdrawUnbonded.0`"),
                                )
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                        }
                    }))
                }
                _ => ::core::result::Result::Err(<_ as ::core::convert::Into<_>>::into(
                    "Could not decode `StakingCall`, variant doesn\'t exist",
                )),
            }
        }
    }
};
impl core::fmt::Debug for StakingCall {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::BondExtra(ref a0) => fmt.debug_tuple("StakingCall::BondExtra").field(a0).finish(),
            Self::Unbond(ref a0) => fmt.debug_tuple("StakingCall::Unbond").field(a0).finish(),
            Self::WithdrawUnbonded(ref a0) => fmt
                .debug_tuple("StakingCall::WithdrawUnbonded")
                .field(a0)
                .finish(),
            _ => Ok(()),
        }
    }
}
pub enum ProxyCall<T: Config, RelayChainCall> {
    Proxy(T::AccountId, Option<ProxyType>, Box<RelayChainCall>),
    AddProxy(T::AccountId, ProxyType, T::BlockNumber),
    RemoveProxy(T::AccountId, ProxyType, T::BlockNumber),
}
impl<T: Config, RelayChainCall> core::fmt::Debug for ProxyCall<T, RelayChainCall>
where
    T: core::fmt::Debug,
    RelayChainCall: core::fmt::Debug,
{
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::Proxy(ref a0, ref a1, ref a2) => fmt
                .debug_tuple("ProxyCall::Proxy")
                .field(a0)
                .field(a1)
                .field(a2)
                .finish(),
            Self::AddProxy(ref a0, ref a1, ref a2) => fmt
                .debug_tuple("ProxyCall::AddProxy")
                .field(a0)
                .field(a1)
                .field(a2)
                .finish(),
            Self::RemoveProxy(ref a0, ref a1, ref a2) => fmt
                .debug_tuple("ProxyCall::RemoveProxy")
                .field(a0)
                .field(a1)
                .field(a2)
                .finish(),
            _ => Ok(()),
        }
    }
}
#[cfg(feature = "kusama")]
mod kusama {
    use crate::*;
    /// The encoded index corresponds to Kusama's Runtime module configuration.
    /// https://github.com/paritytech/polkadot/blob/master/runtime/kusama/src/lib.rs#L1361
    pub enum RelayChainCall<T: Config> {
        #[codec(index = 4)]
        Balances(BalancesCall<T>),
        #[codec(index = 6)]
        Staking(StakingCall),
        #[codec(index = 24)]
        Utility(Box<UtilityCall<Self>>),
        #[codec(index = 30)]
        Proxy(Box<ProxyCall<T, Self>>),
    }
    const _: () = {
        impl<T: Config> ::codec::Encode for RelayChainCall<T>
        where
            BalancesCall<T>: ::codec::Encode,
            BalancesCall<T>: ::codec::Encode,
            Box<ProxyCall<T, Self>>: ::codec::Encode,
            Box<ProxyCall<T, Self>>: ::codec::Encode,
        {
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                match *self {
                    RelayChainCall::Balances(ref aa) => {
                        __codec_dest_edqy.push_byte(4u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(aa, __codec_dest_edqy);
                    }
                    RelayChainCall::Staking(ref aa) => {
                        __codec_dest_edqy.push_byte(6u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(aa, __codec_dest_edqy);
                    }
                    RelayChainCall::Utility(ref aa) => {
                        __codec_dest_edqy.push_byte(24u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(aa, __codec_dest_edqy);
                    }
                    RelayChainCall::Proxy(ref aa) => {
                        __codec_dest_edqy.push_byte(30u8 as ::core::primitive::u8);
                        ::codec::Encode::encode_to(aa, __codec_dest_edqy);
                    }
                    _ => (),
                }
            }
        }
        impl<T: Config> ::codec::EncodeLike for RelayChainCall<T>
        where
            BalancesCall<T>: ::codec::Encode,
            BalancesCall<T>: ::codec::Encode,
            Box<ProxyCall<T, Self>>: ::codec::Encode,
            Box<ProxyCall<T, Self>>: ::codec::Encode,
        {
        }
    };
    const _: () = {
        impl<T: Config> ::codec::Decode for RelayChainCall<T>
        where
            BalancesCall<T>: ::codec::Decode,
            BalancesCall<T>: ::codec::Decode,
            Box<ProxyCall<T, Self>>: ::codec::Decode,
            Box<ProxyCall<T, Self>>: ::codec::Decode,
        {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                match __codec_input_edqy.read_byte().map_err(|e| {
                    e.chain("Could not decode `RelayChainCall`, failed to read variant byte")
                })? {
                    __codec_x_edqy if __codec_x_edqy == 4u8 as ::core::primitive::u8 => {
                        ::core::result::Result::Ok(RelayChainCall::<T>::Balances({
                            let __codec_res_edqy =
                                <BalancesCall<T> as ::codec::Decode>::decode(__codec_input_edqy);
                            match __codec_res_edqy {
                                ::core::result::Result::Err(e) => {
                                    return ::core::result::Result::Err(
                                        e.chain("Could not decode `RelayChainCall::Balances.0`"),
                                    )
                                }
                                ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                            }
                        }))
                    }
                    __codec_x_edqy if __codec_x_edqy == 6u8 as ::core::primitive::u8 => {
                        ::core::result::Result::Ok(RelayChainCall::<T>::Staking({
                            let __codec_res_edqy =
                                <StakingCall as ::codec::Decode>::decode(__codec_input_edqy);
                            match __codec_res_edqy {
                                ::core::result::Result::Err(e) => {
                                    return ::core::result::Result::Err(
                                        e.chain("Could not decode `RelayChainCall::Staking.0`"),
                                    )
                                }
                                ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                            }
                        }))
                    }
                    __codec_x_edqy if __codec_x_edqy == 24u8 as ::core::primitive::u8 => {
                        ::core::result::Result::Ok(RelayChainCall::<T>::Utility({
                            let __codec_res_edqy =
                                <Box<UtilityCall<Self>> as ::codec::Decode>::decode(
                                    __codec_input_edqy,
                                );
                            match __codec_res_edqy {
                                ::core::result::Result::Err(e) => {
                                    return ::core::result::Result::Err(
                                        e.chain("Could not decode `RelayChainCall::Utility.0`"),
                                    )
                                }
                                ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                            }
                        }))
                    }
                    __codec_x_edqy if __codec_x_edqy == 30u8 as ::core::primitive::u8 => {
                        ::core::result::Result::Ok(RelayChainCall::<T>::Proxy({
                            let __codec_res_edqy =
                                <Box<ProxyCall<T, Self>> as ::codec::Decode>::decode(
                                    __codec_input_edqy,
                                );
                            match __codec_res_edqy {
                                ::core::result::Result::Err(e) => {
                                    return ::core::result::Result::Err(
                                        e.chain("Could not decode `RelayChainCall::Proxy.0`"),
                                    )
                                }
                                ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                            }
                        }))
                    }
                    _ => ::core::result::Result::Err(<_ as ::core::convert::Into<_>>::into(
                        "Could not decode `RelayChainCall`, variant doesn\'t exist",
                    )),
                }
            }
        }
    };
    impl<T: Config> core::fmt::Debug for RelayChainCall<T>
    where
        T: core::fmt::Debug,
    {
        fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
            match self {
                Self::Balances(ref a0) => fmt
                    .debug_tuple("RelayChainCall::Balances")
                    .field(a0)
                    .finish(),
                Self::Staking(ref a0) => fmt
                    .debug_tuple("RelayChainCall::Staking")
                    .field(a0)
                    .finish(),
                Self::Utility(ref a0) => fmt
                    .debug_tuple("RelayChainCall::Utility")
                    .field(a0)
                    .finish(),
                Self::Proxy(ref a0) => fmt.debug_tuple("RelayChainCall::Proxy").field(a0).finish(),
                _ => Ok(()),
            }
        }
    }
}
#[cfg(feature = "kusama")]
pub use kusama::*;
pub struct RelayChainCallBuilder<T: Config, ParachainId: Get<ParaId>>(
    PhantomData<(T, ParachainId)>,
);
impl<T: Config, ParachainId: Get<ParaId>> CallBuilder for RelayChainCallBuilder<T, ParachainId>
where
    T::AccountId: FullCodec,
    RelayChainCall<T>: FullCodec,
{
    type AccountId = T::AccountId;
    type Balance = Balance;
    type RelayChainCall = RelayChainCall<T>;
    fn utility_batch_call(calls: Vec<Self::RelayChainCall>) -> Self::RelayChainCall {
        RelayChainCall::Utility(Box::new(UtilityCall::BatchAll(calls)))
    }
    fn utility_as_derivative_call(call: Self::RelayChainCall, index: u16) -> Self::RelayChainCall {
        RelayChainCall::Utility(Box::new(UtilityCall::AsDerivative(index, call)))
    }
    fn staking_bond_extra(amount: Self::Balance) -> Self::RelayChainCall {
        RelayChainCall::Staking(StakingCall::BondExtra(amount))
    }
    fn staking_unbond(amount: Self::Balance) -> Self::RelayChainCall {
        RelayChainCall::Staking(StakingCall::Unbond(amount))
    }
    fn staking_withdraw_unbonded(num_slashing_spans: u32) -> Self::RelayChainCall {
        RelayChainCall::Staking(StakingCall::WithdrawUnbonded(num_slashing_spans))
    }
    fn balances_transfer_keep_alive(
        to: Self::AccountId,
        amount: Self::Balance,
    ) -> Self::RelayChainCall {
        RelayChainCall::Balances(BalancesCall::TransferKeepAlive(
            T::Lookup::unlookup(to),
            amount,
        ))
    }
    fn proxy_add_proxy(delegate: Self::AccountId) -> Self::RelayChainCall {
        RelayChainCall::Proxy(Box::new(ProxyCall::AddProxy(
            delegate,
            ProxyType::Any,
            Default::default(),
        )))
    }
    fn proxy_remove_proxy(delegate: Self::AccountId) -> Self::RelayChainCall {
        RelayChainCall::Proxy(Box::new(ProxyCall::RemoveProxy(
            delegate,
            ProxyType::Any,
            Default::default(),
        )))
    }
    fn proxy_call_via_proxy(
        real: Self::AccountId,
        call: Self::RelayChainCall,
    ) -> Self::RelayChainCall {
        RelayChainCall::Proxy(Box::new(ProxyCall::Proxy(
            real,
            Some(ProxyType::Any),
            Box::new(call),
        )))
    }
    fn finalize_call_into_xcm_message(
        call: Self::RelayChainCall,
        extra_fee: Self::Balance,
        weight: Weight,
    ) -> Xcm<()> {
        let asset = MultiAsset {
            id: Concrete(MultiLocation::here()),
            fun: Fungibility::Fungible(extra_fee),
        };
        Xcm(<[_]>::into_vec(box [
            WithdrawAsset(asset.clone().into()),
            BuyExecution {
                fees: asset,
                weight_limit: Unlimited,
            },
            Transact {
                origin_type: OriginKind::SovereignAccount,
                require_weight_at_most: weight,
                call: call.encode().into(),
            },
            DepositAsset {
                assets: All.into(),
                max_assets: u32::max_value(),
                beneficiary: MultiLocation {
                    parents: 0,
                    interior: X1(Parachain(ParachainId::get().into())),
                },
            },
        ]))
    }
}
royyang@Roys-MacBook-Pro Acala % 