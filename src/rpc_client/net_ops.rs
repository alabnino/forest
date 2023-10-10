// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use crate::rpc_api::net_api::*;
use jsonrpc_v2::Error;

use crate::rpc_client::call;

pub async fn net_addrs_listen(
    (): NetAddrsListenParams,
    auth_token: &Option<String>,
) -> Result<NetAddrsListenResult, Error> {
    call(NET_ADDRS_LISTEN, (), auth_token).await
}

pub async fn net_peers(
    (): NetPeersParams,
    auth_token: &Option<String>,
) -> Result<NetPeersResult, Error> {
    call(NET_PEERS, (), auth_token).await
}

pub async fn net_info(
    (): NetInfoParams,
    auth_token: &Option<String>,
) -> Result<NetInfoResult, Error> {
    call(NET_INFO, (), auth_token).await
}

pub async fn net_connect(
    params: NetConnectParams,
    auth_token: &Option<String>,
) -> Result<NetConnectResult, Error> {
    call(NET_CONNECT, params, auth_token).await
}

pub async fn net_disconnect(
    params: NetDisconnectParams,
    auth_token: &Option<String>,
) -> Result<NetDisconnectResult, Error> {
    call(NET_DISCONNECT, params, auth_token).await
}
