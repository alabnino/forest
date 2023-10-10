// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use crate::libp2p_bitswap::BitswapBehaviour;
use crate::utils::{encoding::blake2b_256, version::FOREST_VERSION_STRING};
use ahash::{HashMap, HashSet};
use libp2p::{
    allow_block_list, connection_limits,
    gossipsub::{
        self, IdentTopic as Topic, MessageAuthenticity, MessageId, PublishError, SubscriptionError,
        ValidationMode,
    },
    identify,
    identity::{Keypair, PeerId},
    kad::QueryId,
    metrics::{Metrics, Recorder},
    ping,
    swarm::NetworkBehaviour,
    Multiaddr,
};
use tracing::warn;

use crate::libp2p::{
    chain_exchange::ChainExchangeBehaviour,
    config::Libp2pConfig,
    discovery::{DiscoveryBehaviour, DiscoveryConfig},
    gossip_params::{build_peer_score_params, build_peer_score_threshold},
    hello::HelloBehaviour,
};

/// Libp2p behavior for the Forest node. This handles all sub protocols needed
/// for a Filecoin node.
#[derive(NetworkBehaviour)]
pub(in crate::libp2p) struct ForestBehaviour {
    gossipsub: gossipsub::Behaviour,
    discovery: DiscoveryBehaviour,
    ping: ping::Behaviour,
    identify: identify::Behaviour,
    connection_limits: connection_limits::Behaviour,
    pub(super) blocked_peers: allow_block_list::Behaviour<allow_block_list::BlockedPeers>,
    pub(super) hello: HelloBehaviour,
    pub(super) chain_exchange: ChainExchangeBehaviour,
    pub(super) bitswap: BitswapBehaviour,
}

impl Recorder<ForestBehaviourEvent> for Metrics {
    fn record(&self, event: &ForestBehaviourEvent) {
        match event {
            ForestBehaviourEvent::Gossipsub(e) => self.record(e),
            ForestBehaviourEvent::Ping(ping_event) => self.record(ping_event),
            ForestBehaviourEvent::Identify(id_event) => self.record(id_event),
            _ => {}
        }
    }
}

impl ForestBehaviour {
    pub fn new(
        local_key: &Keypair,
        config: &Libp2pConfig,
        network_name: &str,
    ) -> anyhow::Result<Self> {
        let mut gs_config_builder = gossipsub::ConfigBuilder::default();
        gs_config_builder.max_transmit_size(1 << 20);
        gs_config_builder.validation_mode(ValidationMode::Strict);
        gs_config_builder.message_id_fn(|msg: &gossipsub::Message| {
            let s = blake2b_256(&msg.data);
            MessageId::from(s)
        });

        let gossipsub_config = gs_config_builder.build().unwrap();
        let mut gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        )
        .unwrap();

        gossipsub
            .with_peer_score(
                build_peer_score_params(network_name),
                build_peer_score_threshold(),
            )
            .unwrap();

        let bitswap = BitswapBehaviour::new(
            &[
                "/chain/ipfs/bitswap/1.2.0",
                "/chain/ipfs/bitswap/1.1.0",
                "/chain/ipfs/bitswap/1.0.0",
                "/chain/ipfs/bitswap",
            ],
            Default::default(),
        );
        if let Err(err) = crate::libp2p_bitswap::register_metrics(prometheus::default_registry()) {
            warn!("Fail to register prometheus metrics for libp2p_bitswap: {err}");
        }

        let mut discovery_config = DiscoveryConfig::new(local_key.public(), network_name);
        discovery_config
            .with_mdns(config.mdns)
            .with_kademlia(config.kademlia)
            .with_user_defined(config.bootstrap_peers.clone())
            .target_peer_count(config.target_peer_count as u64);

        let connection_limits = connection_limits::Behaviour::new(
            connection_limits::ConnectionLimits::default()
                .with_max_pending_incoming(Some(4096))
                .with_max_pending_outgoing(Some(8192))
                .with_max_established_incoming(Some(8192))
                .with_max_established_outgoing(Some(8192))
                .with_max_established_per_peer(Some(5)),
        );

        warn!("libp2p Forest version: {}", FOREST_VERSION_STRING.as_str());
        Ok(ForestBehaviour {
            gossipsub,
            discovery: discovery_config.finish()?,
            ping: Default::default(),
            identify: identify::Behaviour::new(
                identify::Config::new("ipfs/0.1.0".into(), local_key.public())
                    .with_agent_version(format!("forest-{}", FOREST_VERSION_STRING.as_str())),
            ),
            connection_limits,
            blocked_peers: Default::default(),
            bitswap,
            hello: HelloBehaviour::default(),
            chain_exchange: ChainExchangeBehaviour::default(),
        })
    }

    /// Bootstrap Kademlia network
    pub fn bootstrap(&mut self) -> Result<QueryId, String> {
        self.discovery.bootstrap()
    }

    /// Publish data over the gossip network.
    pub fn publish(
        &mut self,
        topic: Topic,
        data: impl Into<Vec<u8>>,
    ) -> Result<MessageId, PublishError> {
        self.gossipsub.publish(topic, data)
    }

    /// Subscribe to a gossip topic.
    pub fn subscribe(&mut self, topic: &Topic) -> Result<bool, SubscriptionError> {
        self.gossipsub.subscribe(topic)
    }

    /// Returns a set of peer ids
    pub fn peers(&self) -> &HashSet<PeerId> {
        self.discovery.peers()
    }

    /// Returns a map of peer ids and their multi-addresses
    pub fn peer_addresses(&mut self) -> &HashMap<PeerId, HashSet<Multiaddr>> {
        self.discovery.peer_addresses()
    }
}
