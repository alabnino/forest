// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::str::FromStr;

use crate::libp2p::{NetRPCMethods, NetworkMessage, PeerId};
use crate::rpc_api::{
    data_types::{AddrInfo, RPCState},
    net_api::*,
};
use cid::multibase;
use futures::channel::oneshot;
use fvm_ipld_blockstore::Blockstore;
use jsonrpc_v2::{Data, Error as JsonRpcError, Params};
use tracing::error;

pub(in crate::rpc) async fn net_addrs_listen<DB: Blockstore>(
    data: Data<RPCState<DB>>,
) -> Result<AddrInfo, JsonRpcError> {
    let (tx, rx) = oneshot::channel();
    let req = NetworkMessage::JSONRPCRequest {
        method: NetRPCMethods::AddrsListen(tx),
    };

    data.network_send.send_async(req).await?;
    let (id, addrs) = rx.await?;

    Ok(AddrInfo {
        id: id.to_string(),
        addrs,
    })
}

pub(in crate::rpc) async fn net_peers<DB: Blockstore>(
    data: Data<RPCState<DB>>,
) -> Result<Vec<AddrInfo>, JsonRpcError> {
    let (tx, rx) = oneshot::channel();
    let req = NetworkMessage::JSONRPCRequest {
        method: NetRPCMethods::Peers(tx),
    };

    data.network_send.send_async(req).await?;
    let peer_addresses = rx.await?;

    let connections = peer_addresses
        .into_iter()
        .map(|(id, addrs)| AddrInfo {
            id: id.to_string(),
            addrs,
        })
        .collect();

    Ok(connections)
}

pub(in crate::rpc) async fn net_info<DB: Blockstore>(
    data: Data<RPCState<DB>>,
) -> Result<NetInfoResult, JsonRpcError> {
    let (tx, rx) = oneshot::channel();
    let req = NetworkMessage::JSONRPCRequest {
        method: NetRPCMethods::Info(tx),
    };

    data.network_send.send_async(req).await?;
    Ok(rx.await?)
}

pub(in crate::rpc) async fn net_connect<DB: Blockstore>(
    data: Data<RPCState<DB>>,
    Params((AddrInfo { id, addrs },)): Params<(AddrInfo,)>,
) -> Result<(), JsonRpcError> {
    let (_, id) = multibase::decode(format!("{}{}", "z", id))?;
    let peer_id = PeerId::from_bytes(&id)?;

    let (tx, rx) = oneshot::channel();
    let req = NetworkMessage::JSONRPCRequest {
        method: NetRPCMethods::Connect(tx, peer_id, addrs),
    };

    data.network_send.send_async(req).await?;
    let success = rx.await?;

    if success {
        Ok(())
    } else {
        error!("Peer could not be dialed from any address provided");
        Err(JsonRpcError::INTERNAL_ERROR)
    }
}

pub(in crate::rpc) async fn net_disconnect<DB: Blockstore>(
    data: Data<RPCState<DB>>,
    Params((id,)): Params<(String,)>,
) -> Result<(), JsonRpcError> {
    let peer_id = PeerId::from_str(&id)?;

    let (tx, rx) = oneshot::channel();
    let req = NetworkMessage::JSONRPCRequest {
        method: NetRPCMethods::Disconnect(tx, peer_id),
    };

    data.network_send.send_async(req).await?;
    rx.await?;

    Ok(())
}
