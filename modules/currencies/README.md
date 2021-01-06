# Currencies Module

## Overview

The currencies module provides a mixed currencies system, by configuring a native currency which implements `BasicCurrencyExtended`, and a multi-currency which implements `MultiCurrency`.

It also provides an adapter, to adapt `frame_support::traits::Currency` implementations into `BasicCurrencyExtended`.

The currencies module provides functionality of both `MultiCurrencyExtended` and `BasicCurrencyExtended`, via unified interfaces, and all calls would be delegated to the underlying multi-currency and base currency system. A native currency ID could be set by `Trait::GetNativeCurrencyId`, to identify the native currency.
