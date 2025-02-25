// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT
#![allow(clippy::unused_async)]

use crate::blocks::TipsetKeys;
use crate::cid_collections::CidHashSet;
use crate::libp2p::NetworkMessage;
use crate::lotus_json::LotusJson;
use crate::rpc_api::data_types::{
    ApiActorState, ApiDeadline, ApiInvocResult, CirculatingSupply, MarketDeal, MessageLookup,
    RPCState, SectorOnChainInfo,
};
use crate::shim::{
    address::Address, clock::ChainEpoch, econ::TokenAmount, executor::Receipt, message::Message,
    state_tree::ActorState, version::NetworkVersion,
};
use crate::state_manager::chain_rand::ChainRand;
use crate::state_manager::vm_circ_supply::GenesisInfo;
use crate::state_manager::{InvocResult, MarketBalance};
use crate::utils::db::car_stream::{CarBlock, CarWriter};
use ahash::{HashMap, HashMapExt};
use anyhow::Context as _;
use cid::Cid;
use fil_actor_interface::miner::DeadlineInfo;
use fil_actor_interface::{
    market, miner,
    miner::{MinerInfo, MinerPower},
};
use fil_actors_shared::fvm_ipld_bitfield::BitField;
use futures::StreamExt;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::{CborStore, DAG_CBOR};
use jsonrpc_v2::{Data, Error as JsonRpcError, Params};
use libipld_core::ipld::Ipld;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::{sync::Arc, time::Duration};
use tokio::task::JoinSet;

type RandomnessParams = (i64, ChainEpoch, Vec<u8>, TipsetKeys);

/// runs the given message and returns its result without any persisted changes.
pub(in crate::rpc) async fn state_call<DB: Blockstore + Send + Sync + 'static>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((message, key))): Params<LotusJson<(Message, TipsetKeys)>>,
) -> Result<ApiInvocResult, JsonRpcError> {
    let state_manager = &data.state_manager;
    let tipset = data
        .state_manager
        .chain_store()
        .load_required_tipset(&key)?;
    // Handle expensive fork error?
    // TODO(elmattic): https://github.com/ChainSafe/forest/issues/3733
    Ok(state_manager.call(&message, Some(tipset))?)
}

/// returns the result of executing the indicated message, assuming it was
/// executed in the indicated tipset.
pub(in crate::rpc) async fn state_replay<DB: Blockstore + Send + Sync + 'static>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((cid, key))): Params<LotusJson<(Cid, TipsetKeys)>>,
) -> Result<InvocResult, JsonRpcError> {
    let state_manager = &data.state_manager;
    let tipset = data
        .state_manager
        .chain_store()
        .load_required_tipset(&key)?;
    let (msg, ret) = state_manager.replay(&tipset, cid).await?;

    Ok(InvocResult {
        msg,
        msg_rct: Some(ret.msg_receipt()),
        error: ret.failure_info(),
    })
}

/// gets network name from state manager
pub(in crate::rpc) async fn state_network_name<DB: Blockstore>(
    data: Data<RPCState<DB>>,
) -> Result<String, JsonRpcError> {
    let state_manager = &data.state_manager;
    let heaviest_tipset = state_manager.chain_store().heaviest_tipset();

    state_manager
        .get_network_name(heaviest_tipset.parent_state())
        .map_err(|e| e.into())
}

pub(in crate::rpc) async fn state_get_network_version<DB: Blockstore>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((tsk,))): Params<LotusJson<(TipsetKeys,)>>,
) -> Result<NetworkVersion, JsonRpcError> {
    let ts = data.chain_store.load_required_tipset(&tsk)?;
    Ok(data.state_manager.get_network_version(ts.epoch()))
}

/// gets the public key address of the given ID address
/// See <https://github.com/filecoin-project/lotus/blob/master/documentation/en/api-v0-methods.md#StateAccountKey>
pub(in crate::rpc) async fn state_account_key<DB: Blockstore>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((address, tipset_keys))): Params<LotusJson<(Address, TipsetKeys)>>,
) -> Result<LotusJson<Address>, JsonRpcError>
where
    DB: Blockstore + Send + Sync + 'static,
{
    let ts_opt = data.chain_store.load_tipset(&tipset_keys)?;
    Ok(LotusJson(
        data.state_manager
            .resolve_to_deterministic_address(address, ts_opt)
            .await?,
    ))
}

/// retrieves the ID address of the given address
/// See <https://github.com/filecoin-project/lotus/blob/master/documentation/en/api-v0-methods.md#StateLookupID>
pub(in crate::rpc) async fn state_lookup_id<DB: Blockstore>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((address, tipset_keys))): Params<LotusJson<(Address, TipsetKeys)>>,
) -> Result<LotusJson<Address>, JsonRpcError>
where
    DB: Blockstore + Send + Sync + 'static,
{
    let ts = data.chain_store.load_required_tipset(&tipset_keys)?;
    let ret = data
        .state_manager
        .lookup_id(&address, ts.as_ref())?
        .with_context(|| {
            format!("Failed to lookup the id address for address: {address} and tipset keys: {tipset_keys}")
        })?;
    Ok(LotusJson(ret))
}

pub(crate) async fn state_get_actor<DB: Blockstore>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((addr, tsk))): Params<LotusJson<(Address, TipsetKeys)>>,
) -> Result<LotusJson<Option<ActorState>>, JsonRpcError> {
    let ts = data.chain_store.load_required_tipset(&tsk)?;
    let state = data.state_manager.get_actor(&addr, *ts.parent_state());
    state.map(Into::into).map_err(|e| e.into())
}

/// looks up the Escrow and Locked balances of the given address in the Storage
/// Market
pub(in crate::rpc) async fn state_market_balance<DB: Blockstore + Send + Sync + 'static>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((address, key))): Params<LotusJson<(Address, TipsetKeys)>>,
) -> Result<MarketBalance, JsonRpcError> {
    let tipset = data
        .state_manager
        .chain_store()
        .load_required_tipset(&key)?;
    data.state_manager
        .market_balance(&address, &tipset)
        .map_err(|e| e.into())
}

pub(in crate::rpc) async fn state_market_deals<DB: Blockstore>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((tsk,))): Params<LotusJson<(TipsetKeys,)>>,
) -> Result<HashMap<String, MarketDeal>, JsonRpcError> {
    let ts = data.chain_store.load_required_tipset(&tsk)?;
    let actor = data
        .state_manager
        .get_actor(&Address::MARKET_ACTOR, *ts.parent_state())?
        .ok_or("Market actor address could not be resolved")?;
    let market_state =
        market::State::load(data.state_manager.blockstore(), actor.code, actor.state)?;

    let da = market_state.proposals(data.state_manager.blockstore())?;
    let sa = market_state.states(data.state_manager.blockstore())?;

    let mut out = HashMap::new();
    da.for_each(|deal_id, d| {
        let s = sa.get(deal_id)?.unwrap_or(market::DealState {
            sector_start_epoch: -1,
            last_updated_epoch: -1,
            slash_epoch: -1,
        });
        out.insert(
            deal_id.to_string(),
            MarketDeal {
                proposal: d,
                state: s,
            },
        );
        Ok(())
    })?;
    Ok(out)
}

/// looks up the miner info of the given address.
pub(in crate::rpc) async fn state_miner_info<DB: Blockstore + Send + Sync + 'static>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((address, key))): Params<LotusJson<(Address, TipsetKeys)>>,
) -> Result<LotusJson<MinerInfo>, JsonRpcError> {
    let tipset = data
        .state_manager
        .chain_store()
        .load_required_tipset(&key)?;
    Ok(LotusJson(data.state_manager.miner_info(&address, &tipset)?))
}

pub(in crate::rpc) async fn state_miner_active_sectors<DB: Blockstore>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((miner, tsk))): Params<LotusJson<(Address, TipsetKeys)>>,
) -> Result<LotusJson<Vec<SectorOnChainInfo>>, JsonRpcError> {
    let bs = data.state_manager.blockstore();
    let ts = data.chain_store.load_required_tipset(&tsk)?;
    let policy = &data.state_manager.chain_config().policy;
    let actor = data
        .state_manager
        .get_actor(&miner, *ts.parent_state())?
        .ok_or("Miner actor address could not be resolved")?;
    let miner_state = miner::State::load(bs, actor.code, actor.state)?;

    // Collect active sectors from each partition in each deadline.
    let mut active_sectors = vec![];
    miner_state.for_each_deadline(policy, bs, |_dlidx, deadline| {
        deadline.for_each(bs, |_partidx, partition| {
            active_sectors.push(partition.active_sectors());
            Ok(())
        })
    })?;

    let sectors = miner_state
        .load_sectors(bs, Some(&BitField::union(&active_sectors)))?
        .into_iter()
        .map(SectorOnChainInfo::from)
        .collect::<Vec<_>>();

    Ok(LotusJson(sectors))
}

/// looks up the miner power of the given address.
pub(in crate::rpc) async fn state_miner_power<DB: Blockstore + Send + Sync + 'static>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((address, key))): Params<LotusJson<(Address, TipsetKeys)>>,
) -> Result<LotusJson<MinerPower>, JsonRpcError> {
    let tipset = data
        .state_manager
        .chain_store()
        .load_required_tipset(&key)?;

    data.state_manager
        .miner_power(&address, &tipset)
        .map(|res| res.into())
        .map_err(|e| e.into())
}

pub(in crate::rpc) async fn state_miner_deadlines<DB: Blockstore + Send + Sync + 'static>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((addr, tsk))): Params<LotusJson<(Address, TipsetKeys)>>,
) -> Result<LotusJson<Vec<ApiDeadline>>, JsonRpcError> {
    let ts = data.chain_store.load_required_tipset(&tsk)?;
    let policy = &data.state_manager.chain_config().policy;
    let actor = data
        .state_manager
        .get_actor(&addr, *ts.parent_state())?
        .ok_or("Miner actor address could not be resolved")?;
    let store = data.state_manager.blockstore();
    let state = miner::State::load(store, actor.code, actor.state)?;
    let mut res = Vec::new();
    state.for_each_deadline(policy, store, |_idx, deadline| {
        res.push(ApiDeadline {
            post_submissions: deadline.partitions_posted(),
            disputable_proof_count: deadline.disputable_proof_count(store)?,
        });
        Ok(())
    })?;
    Ok(LotusJson(res))
}

pub(in crate::rpc) async fn state_miner_proving_deadline<DB: Blockstore + Send + Sync + 'static>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((addr, tsk))): Params<LotusJson<(Address, TipsetKeys)>>,
) -> Result<LotusJson<DeadlineInfo>, JsonRpcError> {
    let ts = data.chain_store.load_required_tipset(&tsk)?;
    let policy = &data.state_manager.chain_config().policy;
    let actor = data
        .state_manager
        .get_actor(&addr, *ts.parent_state())?
        .ok_or("Miner actor address could not be resolved")?;
    let store = data.state_manager.blockstore();
    let state = miner::State::load(store, actor.code, actor.state)?;
    Ok(LotusJson(state.deadline_info(policy, ts.epoch())))
}

/// looks up the miner power of the given address.
pub(in crate::rpc) async fn state_miner_faults<DB: Blockstore + Send + Sync + 'static>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((address, key))): Params<LotusJson<(Address, TipsetKeys)>>,
) -> Result<LotusJson<BitField>, JsonRpcError> {
    let ts = data
        .state_manager
        .chain_store()
        .load_required_tipset(&key)?;

    data.state_manager
        .miner_faults(&address, &ts)
        .map_err(|e| e.into())
        .map(|r| r.into())
}

pub(in crate::rpc) async fn state_miner_recoveries<DB: Blockstore + Send + Sync + 'static>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((miner, tsk))): Params<LotusJson<(Address, TipsetKeys)>>,
) -> Result<LotusJson<BitField>, JsonRpcError> {
    let ts = data
        .state_manager
        .chain_store()
        .load_required_tipset(&tsk)?;

    data.state_manager
        .miner_recoveries(&miner, &ts)
        .map_err(|e| e.into())
        .map(|r| r.into())
}

/// returns the message receipt for the given message
pub(in crate::rpc) async fn state_get_receipt<DB: Blockstore + Send + Sync + 'static>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((cid, key))): Params<LotusJson<(Cid, TipsetKeys)>>,
) -> Result<LotusJson<Receipt>, JsonRpcError> {
    let state_manager = &data.state_manager;
    let tipset = data
        .state_manager
        .chain_store()
        .load_required_tipset(&key)?;
    state_manager
        .get_receipt(tipset, cid)
        .map(|s| s.into())
        .map_err(|e| e.into())
}
/// looks back in the chain for a message. If not found, it blocks until the
/// message arrives on chain, and gets to the indicated confidence depth.
pub(in crate::rpc) async fn state_wait_msg<DB: Blockstore + Send + Sync + 'static>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((cid, confidence))): Params<LotusJson<(Cid, i64)>>,
) -> Result<MessageLookup, JsonRpcError> {
    let state_manager = &data.state_manager;
    let (tipset, receipt) = state_manager.wait_for_message(cid, confidence).await?;
    let tipset = tipset.ok_or("wait for msg returned empty tuple")?;
    let receipt = receipt.ok_or("wait for msg returned empty receipt")?;
    let ipld: Ipld = if receipt.return_data().bytes().is_empty() {
        Ipld::Null
    } else {
        receipt.return_data().deserialize()?
    };
    Ok(MessageLookup {
        receipt,
        tipset: tipset.key().clone(),
        height: tipset.epoch(),
        message: cid,
        return_dec: ipld,
    })
}

// Sample CIDs (useful for testing):
//   Mainnet:
//     1,594,681 bafy2bzaceaclaz3jvmbjg3piazaq5dcesoyv26cdpoozlkzdiwnsvdvm2qoqm OhSnap upgrade
//     1_960_320 bafy2bzacec43okhmihmnwmgqspyrkuivqtxv75rpymsdbulq6lgsdq2vkwkcg Skyr upgrade
//     2,833,266 bafy2bzacecaydufxqo5vtouuysmg3tqik6onyuezm6lyviycriohgfnzfslm2
//     2,933,266 bafy2bzacebyp6cmbshtzzuogzk7icf24pt6s5veyq5zkkqbn3sbbvswtptuuu
//   Calibnet:
//     242,150 bafy2bzaceb522vvt3wo7xhleo2dvb7wb7pyydmzlahc4aqd7lmvg3afreejiw
//     630,932 bafy2bzacedidwdsd7ds73t3z76hcjfsaisoxrangkxsqlzih67ulqgtxnypqk
//
/// Traverse an IPLD directed acyclic graph and use libp2p-bitswap to request any missing nodes.
/// This function has two primary uses: (1) Downloading specific state-roots when Forest deviates
/// from the mainline blockchain, (2) fetching historical state-trees to verify past versions of the
/// consensus rules.
pub(in crate::rpc) async fn state_fetch_root<DB: Blockstore + Sync + Send + 'static>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((root_cid, save_to_file))): Params<LotusJson<(Cid, Option<PathBuf>)>>,
) -> Result<String, JsonRpcError> {
    let network_send = data.network_send.clone();
    let db = data.chain_store.db.clone();
    drop(data);

    let (car_tx, car_handle) = if let Some(save_to_file) = save_to_file {
        let (car_tx, car_rx) = flume::bounded(100);
        let roots = vec![root_cid];
        let file = tokio::fs::File::create(save_to_file).await?;

        let car_handle = tokio::spawn(async move {
            car_rx
                .stream()
                .map(Ok)
                .forward(CarWriter::new_carv1(roots, file)?)
                .await
        });

        (Some(car_tx), Some(car_handle))
    } else {
        (None, None)
    };

    const MAX_CONCURRENT_REQUESTS: usize = 64;
    const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

    let mut seen: CidHashSet = CidHashSet::default();
    let mut counter: usize = 0;
    let mut fetched: usize = 0;
    let mut failures: usize = 0;
    let mut task_set = JoinSet::new();

    fn handle_worker(fetched: &mut usize, failures: &mut usize, ret: anyhow::Result<()>) {
        match ret {
            Ok(()) => *fetched += 1,
            Err(msg) => {
                *failures += 1;
                tracing::debug!("Request failed: {msg}");
            }
        }
    }

    // When walking an Ipld graph, we're only interested in the DAG_CBOR encoded nodes.
    let mut get_ipld_link = |ipld: &Ipld| match ipld {
        &Ipld::Link(cid) if cid.codec() == DAG_CBOR && seen.insert(cid) => Some(cid),
        _ => None,
    };

    // Do a depth-first-search of the IPLD graph (DAG). Nodes that are _not_ present in our database
    // are fetched in background tasks. If the number of tasks reaches MAX_CONCURRENT_REQUESTS, the
    // depth-first-search pauses until one of the work tasks returns. The memory usage of this
    // algorithm is dominated by the set of seen CIDs and the 'dfs' stack is not expected to grow to
    // more than 1000 elements (even when walking tens of millions of nodes).
    let dfs = Arc::new(Mutex::new(vec![Ipld::Link(root_cid)]));
    let mut to_be_fetched = vec![];

    // Loop until: No more items in `dfs` AND no running worker tasks.
    loop {
        while let Some(ipld) = lock_pop(&dfs) {
            {
                let mut dfs_guard = dfs.lock();
                // Scan for unseen CIDs. Available IPLD nodes are pushed to the depth-first-search
                // stack, unavailable nodes will be requested in worker tasks.
                for new_cid in ipld.iter().filter_map(&mut get_ipld_link) {
                    counter += 1;
                    if counter % 1_000 == 0 {
                        // set RUST_LOG=forest_filecoin::rpc::state_api=debug to enable these printouts.
                        tracing::debug!(
                                "Graph walk: CIDs: {counter}, Fetched: {fetched}, Failures: {failures}, dfs: {}, Concurrent: {}",
                                dfs_guard.len(), task_set.len()
                            );
                    }

                    if let Some(next_ipld) = db.get_cbor(&new_cid)? {
                        dfs_guard.push(next_ipld);
                        if let Some(car_tx) = &car_tx {
                            car_tx.send(CarBlock {
                                cid: new_cid,
                                data: db.get(&new_cid)?.with_context(|| {
                                    format!("Failed to get cid {new_cid} from block store")
                                })?,
                            })?;
                        }
                    } else {
                        to_be_fetched.push(new_cid);
                    }
                }
            }

            while let Some(cid) = to_be_fetched.pop() {
                if task_set.len() == MAX_CONCURRENT_REQUESTS {
                    if let Some(ret) = task_set.join_next().await {
                        handle_worker(&mut fetched, &mut failures, ret?)
                    }
                }
                task_set.spawn_blocking({
                    let network_send = network_send.clone();
                    let db = db.clone();
                    let dfs_vec = Arc::clone(&dfs);
                    let car_tx = car_tx.clone();
                    move || {
                        let (tx, rx) = flume::bounded(1);
                        network_send.send(NetworkMessage::BitswapRequest {
                            cid,
                            response_channel: tx,
                            epoch: None,
                        })?;
                        // Bitswap requests do not fail. They are just ignored if no-one has
                        // the requested data. Here we arbitrary decide to only wait for
                        // REQUEST_TIMEOUT before judging that the data is unavailable.
                        let _ignore = rx.recv_timeout(REQUEST_TIMEOUT);

                        let new_ipld = db
                            .get_cbor::<Ipld>(&cid)?
                            .with_context(|| format!("Request failed: {cid}"))?;
                        dfs_vec.lock().push(new_ipld);
                        if let Some(car_tx) = &car_tx {
                            car_tx.send(CarBlock {
                                cid,
                                data: db.get(&cid)?.with_context(|| {
                                    format!("Failed to get cid {cid} from block store")
                                })?,
                            })?;
                        }

                        Ok(())
                    }
                });
            }
            tokio::task::yield_now().await;
        }
        if let Some(ret) = task_set.join_next().await {
            handle_worker(&mut fetched, &mut failures, ret?)
        } else {
            // We are out of work items (dfs) and all worker threads have finished, this means
            // the entire graph has been walked and fetched.
            break;
        }
    }

    drop(car_tx);
    if let Some(car_handle) = car_handle {
        car_handle.await??;
    }

    Ok(format!(
        "IPLD graph traversed! CIDs: {counter}, fetched: {fetched}, failures: {failures}."
    ))
}

// Convenience function for locking and popping a value out of a vector. If this function is
// inlined, the mutex guard isn't dropped early enough.
fn lock_pop<T>(mutex: &Mutex<Vec<T>>) -> Option<T> {
    mutex.lock().pop()
}

/// Get randomness from tickets
pub(in crate::rpc) async fn state_get_randomness_from_tickets<
    DB: Blockstore + Send + Sync + 'static,
>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((personalization, rand_epoch, entropy, tsk))): Params<
        LotusJson<RandomnessParams>,
    >,
) -> Result<LotusJson<Vec<u8>>, JsonRpcError> {
    let state_manager = &data.state_manager;
    let tipset = state_manager.chain_store().load_required_tipset(&tsk)?;
    let chain_config = state_manager.chain_config();
    let chain_index = &data.chain_store.chain_index;
    let beacon = state_manager.beacon_schedule();
    let chain_rand = ChainRand::new(chain_config.clone(), tipset, chain_index.clone(), beacon);
    let digest = chain_rand.get_chain_randomness(rand_epoch, false)?;
    let value = crate::state_manager::chain_rand::draw_randomness_from_digest(
        &digest,
        personalization,
        rand_epoch,
        &entropy,
    )?;
    Ok(LotusJson(value.to_vec()))
}

/// Get randomness from beacon
pub(in crate::rpc) async fn state_get_randomness_from_beacon<
    DB: Blockstore + Send + Sync + 'static,
>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((personalization, rand_epoch, entropy, tsk))): Params<
        LotusJson<RandomnessParams>,
    >,
) -> Result<LotusJson<Vec<u8>>, JsonRpcError> {
    let state_manager = &data.state_manager;
    let tipset = state_manager.chain_store().load_required_tipset(&tsk)?;
    let chain_config = state_manager.chain_config();
    let chain_index = &data.chain_store.chain_index;
    let beacon = state_manager.beacon_schedule();
    let chain_rand = ChainRand::new(chain_config.clone(), tipset, chain_index.clone(), beacon);
    let digest = chain_rand.get_beacon_randomness_v3(rand_epoch)?;
    let value = crate::state_manager::chain_rand::draw_randomness_from_digest(
        &digest,
        personalization,
        rand_epoch,
        &entropy,
    )?;
    Ok(LotusJson(value.to_vec()))
}

/// Get read state
pub(in crate::rpc) async fn state_read_state<DB: Blockstore + Send + Sync + 'static>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((addr, tsk))): Params<LotusJson<(Address, TipsetKeys)>>,
) -> Result<LotusJson<ApiActorState>, JsonRpcError> {
    let ts = data.chain_store.load_required_tipset(&tsk)?;
    let actor = data
        .state_manager
        .get_actor(&addr, *ts.parent_state())?
        .ok_or("Actor address could not be resolved")?;
    let blk = data
        .state_manager
        .blockstore()
        .get(&actor.state)?
        .ok_or("Failed to get block from blockstore")?;
    let state = fvm_ipld_encoding::from_slice::<Vec<Cid>>(&blk)?[0];

    Ok(LotusJson(ApiActorState::new(
        actor.balance.clone().into(),
        actor.code,
        Ipld::Link(state),
    )))
}

pub(in crate::rpc) async fn state_circulating_supply<DB: Blockstore + Send + Sync + 'static>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((tsk,))): Params<LotusJson<(TipsetKeys,)>>,
) -> Result<LotusJson<TokenAmount>, JsonRpcError> {
    let ts = data.chain_store.load_required_tipset(&tsk)?;

    let height = ts.epoch();

    let state_manager = &data.state_manager;

    let root = ts.parent_state();

    let genesis_info = GenesisInfo::from_chain_config(state_manager.chain_config());

    let supply =
        genesis_info.get_circulating_supply(height, &state_manager.blockstore_owned(), root)?;

    Ok(LotusJson(supply))
}

/// Get state sector info using sector no
pub(in crate::rpc) async fn state_sector_get_info<DB: Blockstore + Send + Sync + 'static>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((addr, sector_no, tsk))): Params<LotusJson<(Address, u64, TipsetKeys)>>,
) -> Result<LotusJson<SectorOnChainInfo>, JsonRpcError> {
    let ts = data.chain_store.load_required_tipset(&tsk)?;

    Ok(LotusJson(
        data.state_manager
            .get_all_sectors(&addr, &ts)?
            .into_iter()
            .find(|info| info.sector_number == sector_no)
            .map(SectorOnChainInfo::from)
            .ok_or(format!("Info for sector number {sector_no} not found"))?,
    ))
}

pub(in crate::rpc) async fn state_vm_circulating_supply_internal<
    DB: Blockstore + Send + Sync + 'static,
>(
    data: Data<RPCState<DB>>,
    Params(LotusJson((tsk,))): Params<LotusJson<(TipsetKeys,)>>,
) -> Result<LotusJson<CirculatingSupply>, JsonRpcError> {
    let ts = data.chain_store.load_required_tipset(&tsk)?;

    let genesis_info = GenesisInfo::from_chain_config(data.state_manager.chain_config());

    Ok(LotusJson(genesis_info.get_vm_circulating_supply_detailed(
        ts.epoch(),
        &data.state_manager.blockstore_owned(),
        ts.parent_state(),
    )?))
}
