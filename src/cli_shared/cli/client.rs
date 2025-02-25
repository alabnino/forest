// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    str::FromStr,
};

use crate::rpc_client::DEFAULT_PORT;
use chrono::Duration;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationSeconds};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
#[cfg_attr(test, derive(derive_quickcheck_arbitrary::Arbitrary))]
pub struct ChunkSize(pub u32);
impl Default for ChunkSize {
    fn default() -> Self {
        ChunkSize(500_000)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
#[cfg_attr(test, derive(derive_quickcheck_arbitrary::Arbitrary))]
pub struct BufferSize(pub u32);
impl Default for BufferSize {
    fn default() -> Self {
        BufferSize(1)
    }
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
#[cfg_attr(test, derive(derive_quickcheck_arbitrary::Arbitrary))]
pub struct Client {
    pub data_dir: PathBuf,
    pub genesis_file: Option<String>,
    pub enable_rpc: bool,
    pub enable_metrics_endpoint: bool,
    pub rpc_token: Option<String>,
    /// If this is true, then we do not validate the imported snapshot.
    /// Otherwise, we validate and compute the states.
    pub snapshot: bool,
    /// If this is true, delete the snapshot at `snapshot_path` if it's a local file.
    pub consume_snapshot: bool,
    pub snapshot_height: Option<i64>,
    pub snapshot_head: Option<i64>,
    pub snapshot_path: Option<PathBuf>,
    /// Skips loading import CAR file and assumes it's already been loaded.
    /// Will use the CIDs in the header of the file to index the chain.
    pub skip_load: bool,
    /// When importing CAR files, chunk key-value pairs before committing them
    /// to the database.
    pub chunk_size: ChunkSize,
    /// When importing CAR files, maintain a read-ahead buffer measured in
    /// number of chunks.
    pub buffer_size: BufferSize,
    pub encrypt_keystore: bool,
    /// Metrics bind, e.g. 127.0.0.1:6116
    pub metrics_address: SocketAddr,
    /// RPC bind, e.g. 127.0.0.1:1234
    pub rpc_address: SocketAddr,
    // Period of validity for JWT in seconds. Defaults to 60 days.
    #[serde_as(as = "DurationSeconds<i64>")]
    #[cfg_attr(test, arbitrary(gen(
        |g| Duration::milliseconds(i64::arbitrary(g))
    )))]
    pub token_exp: Duration,
    /// Load actors from the bundle file (possibly generating it if it doesn't exist)
    pub load_actors: bool,
}

impl Default for Client {
    fn default() -> Self {
        let dir = ProjectDirs::from("com", "ChainSafe", "Forest").expect("failed to find project directories, please set FOREST_CONFIG_PATH environment variable manually.");
        Self {
            data_dir: dir.data_dir().to_path_buf(),
            genesis_file: None,
            enable_rpc: true,
            enable_metrics_endpoint: true,
            rpc_token: None,
            snapshot_path: None,
            snapshot: false,
            consume_snapshot: false,
            snapshot_height: None,
            snapshot_head: None,
            skip_load: false,
            chunk_size: ChunkSize::default(),
            buffer_size: BufferSize::default(),
            encrypt_keystore: true,
            metrics_address: FromStr::from_str("0.0.0.0:6116").unwrap(),
            rpc_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DEFAULT_PORT),
            token_exp: Duration::seconds(5184000), // 60 Days = 5184000 Seconds
            load_actors: true,
        }
    }
}
