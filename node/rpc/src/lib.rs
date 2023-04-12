// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

mod auth_api;
mod beacon_api;
mod chain_api;
mod common_api;
mod db_api;
mod gas_api;
mod mpool_api;
mod net_api;
mod rpc_http_handler;
mod rpc_util;
mod rpc_ws_handler;
mod state_api;
mod sync_api;
mod wallet_api;

use std::{net::TcpListener, sync::Arc};

use axum::routing::{get, post};
use forest_beacon::Beacon;
use forest_chain::Scale;
use forest_rpc_api::{
    auth_api::*, beacon_api::*, chain_api::*, common_api::*, data_types::RPCState, db_api::*,
    gas_api::*, mpool_api::*, net_api::*, state_api::*, sync_api::*, wallet_api::*,
};
use fvm_ipld_blockstore::Blockstore;
use jsonrpc_v2::{Data, Error as JSONRPCError, Server};
use log::info;
use tokio::sync::mpsc::Sender;

use crate::{
    beacon_api::beacon_get_entry,
    common_api::{shutdown, version},
    rpc_http_handler::rpc_http_handler,
    rpc_ws_handler::rpc_ws_handler,
    state_api::*,
};

pub async fn start_rpc<DB, B, S>(
    state: Arc<RPCState<DB, B>>,
    rpc_endpoint: TcpListener,
    forest_version: &'static str,
    shutdown_send: Sender<()>,
) -> Result<(), JSONRPCError>
where
    DB: Blockstore + Clone + Send + Sync + 'static,
    B: Beacon,
    S: Scale + 'static,
{
    use auth_api::*;
    use chain_api::*;
    use gas_api::*;
    use mpool_api::*;
    use sync_api::*;
    use wallet_api::*;

    let block_delay = state.state_manager.chain_config().block_delay_secs;
    let rpc_server = Arc::new(
        Server::new()
            .with_data(Data(state))
            // Auth API
            .with_method(AUTH_NEW, auth_new::<DB, B>)
            .with_method(AUTH_VERIFY, auth_verify::<DB, B>)
            // Beacon API
            .with_method(BEACON_GET_ENTRY, beacon_get_entry::<DB, B>)
            // Chain API
            .with_method(CHAIN_GET_MESSAGE, chain_api::chain_get_message::<DB, B>)
            .with_method(CHAIN_EXPORT, chain_api::chain_export::<DB, B>)
            .with_method(CHAIN_READ_OBJ, chain_read_obj::<DB, B>)
            .with_method(CHAIN_HAS_OBJ, chain_has_obj::<DB, B>)
            .with_method(CHAIN_GET_BLOCK_MESSAGES, chain_get_block_messages::<DB, B>)
            .with_method(
                CHAIN_GET_TIPSET_BY_HEIGHT,
                chain_get_tipset_by_height::<DB, B>,
            )
            .with_method(CHAIN_GET_GENESIS, chain_get_genesis::<DB, B>)
            .with_method(CHAIN_GET_TIPSET, chain_get_tipset::<DB, B>)
            .with_method(CHAIN_GET_TIPSET_HASH, chain_get_tipset_hash::<DB, B>)
            .with_method(
                CHAIN_VALIDATE_TIPSET_CHECKPOINTS,
                chain_validate_tipset_checkpoints::<DB, B>,
            )
            .with_method(CHAIN_HEAD, chain_head::<DB, B>)
            .with_method(CHAIN_GET_BLOCK, chain_api::chain_get_block::<DB, B>)
            .with_method(CHAIN_GET_NAME, chain_api::chain_get_name::<DB, B>)
            // Message Pool API
            .with_method(MPOOL_PENDING, mpool_pending::<DB, B>)
            .with_method(MPOOL_PUSH, mpool_push::<DB, B>)
            .with_method(MPOOL_PUSH_MESSAGE, mpool_push_message::<DB, B>)
            // Sync API
            .with_method(SYNC_CHECK_BAD, sync_check_bad::<DB, B>)
            .with_method(SYNC_MARK_BAD, sync_mark_bad::<DB, B>)
            .with_method(SYNC_STATE, sync_state::<DB, B>)
            // Wallet API
            .with_method(WALLET_BALANCE, wallet_balance::<DB, B>)
            .with_method(WALLET_DEFAULT_ADDRESS, wallet_default_address::<DB, B>)
            .with_method(WALLET_EXPORT, wallet_export::<DB, B>)
            .with_method(WALLET_HAS, wallet_has::<DB, B>)
            .with_method(WALLET_IMPORT, wallet_import::<DB, B>)
            .with_method(WALLET_LIST, wallet_list::<DB, B>)
            .with_method(WALLET_NEW, wallet_new::<DB, B>)
            .with_method(WALLET_SET_DEFAULT, wallet_set_default::<DB, B>)
            .with_method(WALLET_SIGN, wallet_sign::<DB, B>)
            .with_method(WALLET_VERIFY, wallet_verify::<DB, B>)
            // State API
            .with_method(STATE_CALL, state_call::<DB, B>)
            .with_method(STATE_REPLAY, state_replay::<DB, B>)
            .with_method(STATE_NETWORK_NAME, state_network_name::<DB, B>)
            .with_method(STATE_NETWORK_VERSION, state_get_network_version::<DB, B>)
            .with_method(STATE_GET_ACTOR, state_get_actor::<DB, B>)
            .with_method(STATE_MARKET_BALANCE, state_market_balance::<DB, B>)
            .with_method(STATE_MARKET_DEALS, state_market_deals::<DB, B>)
            .with_method(STATE_GET_RECEIPT, state_get_receipt::<DB, B>)
            .with_method(STATE_WAIT_MSG, state_wait_msg::<DB, B>)
            // Gas API
            .with_method(GAS_ESTIMATE_FEE_CAP, gas_estimate_fee_cap::<DB, B>)
            .with_method(GAS_ESTIMATE_GAS_LIMIT, gas_estimate_gas_limit::<DB, B>)
            .with_method(GAS_ESTIMATE_GAS_PREMIUM, gas_estimate_gas_premium::<DB, B>)
            .with_method(GAS_ESTIMATE_MESSAGE_GAS, gas_estimate_message_gas::<DB, B>)
            // Common API
            .with_method(VERSION, move || version(block_delay, forest_version))
            .with_method(SHUTDOWN, move || shutdown(shutdown_send.clone()))
            // Net API
            .with_method(NET_ADDRS_LISTEN, net_api::net_addrs_listen::<DB, B>)
            .with_method(NET_PEERS, net_api::net_peers::<DB, B>)
            .with_method(NET_CONNECT, net_api::net_connect::<DB, B>)
            .with_method(NET_DISCONNECT, net_api::net_disconnect::<DB, B>)
            // DB API
            .with_method(DB_GC, db_api::db_gc::<DB, B>)
            .finish_unwrapped(),
    );

    let app = axum::Router::new()
        .route("/rpc/v0", get(rpc_ws_handler))
        .route("/rpc/v0", post(rpc_http_handler))
        .with_state(rpc_server);

    info!("Ready for RPC connections");
    let server = axum::Server::from_tcp(rpc_endpoint)?.serve(app.into_make_service());
    server.await?;

    info!("Stopped accepting RPC connections");

    Ok(())
}
