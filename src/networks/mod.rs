// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::{fmt::Display, str::FromStr};

use cid::Cid;
use fil_actors_shared::v10::runtime::Policy;
use libp2p::Multiaddr;
use serde::{Deserialize, Serialize};
use strum_macros::Display;

use crate::beacon::{BeaconPoint, BeaconSchedule, DrandBeacon, DrandConfig};
use crate::db::SettingsStore;
use crate::make_butterfly_policy;
use crate::shim::clock::{ChainEpoch, EPOCH_DURATION_SECONDS};
use crate::shim::sector::{RegisteredPoStProofV3, RegisteredSealProofV3};
use crate::shim::version::NetworkVersion;

mod actors_bundle;
pub use actors_bundle::{generate_actor_bundle, ActorBundleInfo, ACTOR_BUNDLES};

mod drand;

pub mod butterflynet;
pub mod calibnet;
pub mod devnet;
pub mod mainnet;

/// Newest network version for all networks
pub const NEWEST_NETWORK_VERSION: NetworkVersion = NetworkVersion::V17;

/// Forest builtin `filecoin` network chains. In general only `mainnet` and its
/// chain information should be considered stable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(test, derive(derive_quickcheck_arbitrary::Arbitrary))]
#[serde(tag = "type", content = "name", rename_all = "lowercase")]
pub enum NetworkChain {
    #[default]
    Mainnet,
    Calibnet,
    Butterflynet,
    Devnet(String),
}

impl FromStr for NetworkChain {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mainnet" => Ok(NetworkChain::Mainnet),
            "calibnet" | "calibrationnet" => Ok(NetworkChain::Calibnet),
            "butterflynet" => Ok(NetworkChain::Butterflynet),
            name => Ok(NetworkChain::Devnet(name.to_owned())),
        }
    }
}

impl Display for NetworkChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkChain::Mainnet => write!(f, "mainnet"),
            NetworkChain::Calibnet => write!(f, "calibnet"),
            NetworkChain::Butterflynet => write!(f, "Butterflynet"),
            NetworkChain::Devnet(name) => write!(f, "{name}"),
        }
    }
}

impl NetworkChain {
    /// Returns [`NetworkChain::Calibnet`] or [`NetworkChain::Mainnet`] if `cid`
    /// is the hard-coded genesis CID for either of those networks.
    pub fn from_genesis(cid: &Cid) -> Option<Self> {
        if cid == &*mainnet::GENESIS_CID {
            Some(Self::Mainnet)
        } else if cid == &*calibnet::GENESIS_CID {
            Some(Self::Calibnet)
        } else if cid == &*butterflynet::GENESIS_CID {
            Some(Self::Butterflynet)
        } else {
            None
        }
    }

    /// Returns [`NetworkChain::Calibnet`] or [`NetworkChain::Mainnet`] if `cid`
    /// is the hard-coded genesis CID for either of those networks.
    ///
    /// Else returns a [`NetworkChain::Devnet`] with a placeholder name.
    pub fn from_genesis_or_devnet_placeholder(cid: &Cid) -> Self {
        Self::from_genesis(cid).unwrap_or(Self::Devnet(String::from("devnet")))
    }

    pub fn is_testnet(&self) -> bool {
        !matches!(self, NetworkChain::Mainnet)
    }
}

/// Defines the meaningful heights of the protocol.
#[derive(Debug, Display, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(derive_quickcheck_arbitrary::Arbitrary))]
pub enum Height {
    Breeze,
    Smoke,
    Ignition,
    ActorsV2,
    Tape,
    Liftoff,
    Kumquat,
    Calico,
    Persian,
    Orange,
    Trust,
    Norwegian,
    Turbo,
    Hyperdrive,
    Chocolate,
    OhSnap,
    Skyr,
    Shark,
    Hygge,
    Lightning,
    Thunder,
    Watermelon,
    WatermelonFix,
    WatermelonFix2,
}

impl Default for Height {
    fn default() -> Height {
        Self::Breeze
    }
}

impl From<Height> for NetworkVersion {
    fn from(height: Height) -> NetworkVersion {
        match height {
            Height::Breeze => NetworkVersion::V1,
            Height::Smoke => NetworkVersion::V2,
            Height::Ignition => NetworkVersion::V3,
            Height::ActorsV2 => NetworkVersion::V4,
            Height::Tape => NetworkVersion::V5,
            Height::Liftoff => NetworkVersion::V5,
            Height::Kumquat => NetworkVersion::V6,
            Height::Calico => NetworkVersion::V7,
            Height::Persian => NetworkVersion::V8,
            Height::Orange => NetworkVersion::V9,
            Height::Trust => NetworkVersion::V10,
            Height::Norwegian => NetworkVersion::V11,
            Height::Turbo => NetworkVersion::V12,
            Height::Hyperdrive => NetworkVersion::V13,
            Height::Chocolate => NetworkVersion::V14,
            Height::OhSnap => NetworkVersion::V15,
            Height::Skyr => NetworkVersion::V16,
            Height::Shark => NetworkVersion::V17,
            Height::Hygge => NetworkVersion::V18,
            Height::Lightning => NetworkVersion::V19,
            Height::Thunder => NetworkVersion::V20,
            Height::Watermelon => NetworkVersion::V21,
            Height::WatermelonFix => NetworkVersion::V21,
            Height::WatermelonFix2 => NetworkVersion::V21,
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(derive_quickcheck_arbitrary::Arbitrary))]
pub struct HeightInfo {
    pub height: Height,
    pub epoch: ChainEpoch,
    pub bundle: Option<Cid>,
}

pub fn sort_by_epoch(height_info_slice: &[HeightInfo]) -> Vec<HeightInfo> {
    let mut height_info_vec = height_info_slice.to_vec();
    height_info_vec.sort_by(|a, b| a.epoch.cmp(&b.epoch));
    height_info_vec
}

#[derive(Clone)]
struct DrandPoint<'a> {
    pub height: ChainEpoch,
    pub config: &'a DrandConfig<'a>,
}

/// Defines all network configuration parameters.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[cfg_attr(test, derive(derive_quickcheck_arbitrary::Arbitrary))]
#[serde(default)]
pub struct ChainConfig {
    pub network: NetworkChain,
    pub genesis_cid: Option<String>,
    #[cfg_attr(test, arbitrary(gen(
        |g: &mut quickcheck::Gen| {
            let addr = std::net::Ipv4Addr::arbitrary(&mut *g);
            let n = u8::arbitrary(g) as usize;
            vec![addr.into(); n]
        }
    )))]
    pub bootstrap_peers: Vec<Multiaddr>,
    pub block_delay_secs: u32,
    pub propagation_delay_secs: u32,
    pub height_infos: Vec<HeightInfo>,
    #[cfg_attr(test, arbitrary(gen(|_g| Policy::mainnet())))]
    #[serde(default = "default_policy")]
    pub policy: Policy,
    pub eth_chain_id: u32,
}

impl ChainConfig {
    pub fn mainnet() -> Self {
        use mainnet::*;
        Self {
            network: NetworkChain::Mainnet,
            genesis_cid: Some(GENESIS_CID.to_string()),
            bootstrap_peers: DEFAULT_BOOTSTRAP.clone(),
            block_delay_secs: EPOCH_DURATION_SECONDS as u32,
            propagation_delay_secs: 10,
            height_infos: HEIGHT_INFOS.to_vec(),
            policy: Policy::mainnet(),
            eth_chain_id: ETH_CHAIN_ID as u32,
        }
    }

    pub fn calibnet() -> Self {
        use calibnet::*;
        Self {
            network: NetworkChain::Calibnet,
            genesis_cid: Some(GENESIS_CID.to_string()),
            bootstrap_peers: DEFAULT_BOOTSTRAP.clone(),
            block_delay_secs: EPOCH_DURATION_SECONDS as u32,
            propagation_delay_secs: 10,
            height_infos: HEIGHT_INFOS.to_vec(),
            policy: Policy::calibnet(),
            eth_chain_id: ETH_CHAIN_ID as u32,
        }
    }

    pub fn devnet() -> Self {
        use devnet::*;
        let mut policy = Policy::mainnet();
        policy.minimum_consensus_power = 2048.into();
        policy.minimum_verified_allocation_size = 256.into();
        policy.pre_commit_challenge_delay = 10;

        #[allow(clippy::disallowed_types)]
        let allowed_proof_types = std::collections::HashSet::from_iter(vec![
            RegisteredSealProofV3::StackedDRG2KiBV1,
            RegisteredSealProofV3::StackedDRG8MiBV1,
        ]);
        policy.valid_pre_commit_proof_type = allowed_proof_types;
        #[allow(clippy::disallowed_types)]
        let allowed_proof_types = std::collections::HashSet::from_iter(vec![
            RegisteredPoStProofV3::StackedDRGWindow2KiBV1,
            RegisteredPoStProofV3::StackedDRGWindow8MiBV1,
        ]);
        policy.valid_post_proof_type = allowed_proof_types;

        Self {
            network: NetworkChain::Devnet("devnet".to_string()),
            genesis_cid: None,
            bootstrap_peers: Vec::new(),
            block_delay_secs: 4,
            propagation_delay_secs: 1,
            height_infos: HEIGHT_INFOS.to_vec(),
            policy,
            eth_chain_id: ETH_CHAIN_ID as u32,
        }
    }

    pub fn butterflynet() -> Self {
        use butterflynet::*;

        Self {
            network: NetworkChain::Butterflynet,
            genesis_cid: Some(GENESIS_CID.to_string()),
            bootstrap_peers: DEFAULT_BOOTSTRAP.clone(),
            block_delay_secs: EPOCH_DURATION_SECONDS as u32,
            propagation_delay_secs: 6,
            height_infos: HEIGHT_INFOS.to_vec(),
            policy: make_butterfly_policy!(v10),
            eth_chain_id: ETH_CHAIN_ID as u32,
        }
    }

    pub fn from_chain(network_chain: &NetworkChain) -> Self {
        match network_chain {
            NetworkChain::Mainnet => Self::mainnet(),
            NetworkChain::Calibnet => Self::calibnet(),
            NetworkChain::Butterflynet => Self::butterflynet(),
            NetworkChain::Devnet(name) => Self {
                network: NetworkChain::Devnet(name.clone()),
                ..Self::devnet()
            },
        }
    }

    pub fn network_version(&self, epoch: ChainEpoch) -> NetworkVersion {
        let height = sort_by_epoch(&self.height_infos)
            .iter()
            .rev()
            .find(|info| epoch > info.epoch)
            .map(|info| info.height)
            .unwrap_or(Height::Breeze);

        From::from(height)
    }

    pub fn get_beacon_schedule(&self, genesis_ts: u64) -> BeaconSchedule {
        let ds_iter = match self.network {
            NetworkChain::Mainnet => mainnet::DRAND_SCHEDULE.iter(),
            NetworkChain::Calibnet => calibnet::DRAND_SCHEDULE.iter(),
            NetworkChain::Butterflynet => butterflynet::DRAND_SCHEDULE.iter(),
            NetworkChain::Devnet(_) => devnet::DRAND_SCHEDULE.iter(),
        };

        BeaconSchedule(
            ds_iter
                .map(|dc| BeaconPoint {
                    height: dc.height,
                    beacon: Box::new(DrandBeacon::new(
                        genesis_ts,
                        self.block_delay_secs as u64,
                        dc.config,
                    )),
                })
                .collect(),
        )
    }

    pub fn epoch(&self, height: Height) -> ChainEpoch {
        sort_by_epoch(&self.height_infos)
            .iter()
            .find(|info| height == info.height)
            .map(|info| info.epoch)
            .unwrap_or(0)
    }

    pub async fn genesis_bytes<DB: SettingsStore>(
        &self,
        db: &DB,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(match self.network {
            NetworkChain::Mainnet => Some(mainnet::DEFAULT_GENESIS.to_vec()),
            NetworkChain::Calibnet => Some(calibnet::DEFAULT_GENESIS.to_vec()),
            // Butterflynet genesis is not hardcoded in the binary, for size reasons.
            NetworkChain::Butterflynet => Some(butterflynet::fetch_genesis(db).await?),
            NetworkChain::Devnet(_) => None,
        })
    }

    pub fn is_testnet(&self) -> bool {
        self.network.is_testnet()
    }
}

impl Default for ChainConfig {
    fn default() -> Self {
        ChainConfig::mainnet()
    }
}

/// Dummy default. Will be overwritten later.
// Wish we could get rid of this
fn default_policy() -> Policy {
    Policy::mainnet()
}

pub(crate) fn parse_bootstrap_peers(bootstrap_peer_list: &str) -> Vec<Multiaddr> {
    bootstrap_peer_list
        .split('\n')
        .filter(|s| !s.is_empty())
        .map(|s| {
            Multiaddr::from_str(s).unwrap_or_else(|e| panic!("invalid bootstrap peer {s}: {e}"))
        })
        .collect()
}
