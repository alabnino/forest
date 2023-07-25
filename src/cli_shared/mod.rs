// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

pub mod cli;
pub mod logger;

use std::path::PathBuf;

#[cfg(feature = "mimalloc")]
pub use mimalloc;
#[cfg(feature = "jemalloc")]
pub use tikv_jemallocator;

/// Gets chain data directory
pub fn chain_path(config: &crate::cli_shared::cli::Config) -> PathBuf {
    let chain_path = PathBuf::from(&config.client.data_dir).join(config.chain.network.to_string());
    // Use the dev database if it exists, else use versioned database
    let dev_path = chain_path.join("dev");
    if dev_path.exists() {
        dev_path
    } else {
        chain_path.join(env!("CARGO_PKG_VERSION"))
    }
}

pub mod snapshot;
