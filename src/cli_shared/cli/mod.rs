// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

mod client;
mod config;

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};

use crate::cli_shared::read_config;
use crate::networks::NetworkChain;
use crate::utils::io::read_file_to_string;
use crate::utils::misc::LoggingColor;
use ahash::HashSet;
use clap::Parser;
use directories::ProjectDirs;
use tracing::error;

pub use self::{client::*, config::*};

pub static HELP_MESSAGE: &str = "\
{name} {version}
{author}
{about}

USAGE:
  {usage}

SUBCOMMANDS:
{subcommands}

OPTIONS:
{options}
";

/// CLI options
#[derive(Default, Debug, Parser)]
pub struct CliOpts {
    /// A TOML file containing relevant configurations
    #[arg(short, long)]
    pub config: Option<String>,
    /// The genesis CAR file
    #[arg(short, long)]
    pub genesis: Option<String>,
    /// Allow RPC to be active or not (default: true)
    #[arg(short, long)]
    pub rpc: Option<bool>,
    /// Disable Metrics endpoint
    #[arg(short, long)]
    pub no_metrics: bool,
    /// Client JWT token to use for JSON-RPC authentication
    #[arg(short, long)]
    pub token: Option<String>,
    /// Address used for metrics collection server. By defaults binds on
    /// localhost on port 6116.
    #[arg(long)]
    pub metrics_address: Option<SocketAddr>,
    /// Address used for RPC. By defaults binds on localhost on port 2345.
    #[arg(long)]
    pub rpc_address: Option<SocketAddr>,
    /// Allow Kademlia (default: true)
    #[arg(short, long)]
    pub kademlia: Option<bool>,
    /// Allow MDNS (default: false)
    #[arg(long)]
    pub mdns: Option<bool>,
    /// Validate snapshot at given EPOCH, use a negative value -N to validate
    /// the last N EPOCH(s) starting at HEAD.
    #[arg(long)]
    pub height: Option<i64>,
    /// Sets the current HEAD epoch to validate to. Useful to specify a
    /// smaller range in conjunction with `height`, ignored if `height`
    /// is unspecified.
    #[arg(long)]
    pub head: Option<u64>,
    /// Import a snapshot from a local CAR file or URL
    #[arg(long)]
    pub import_snapshot: Option<String>,
    /// Import a snapshot from a local CAR file and delete it, or from a URL
    #[arg(long)]
    pub consume_snapshot: Option<String>,
    /// Halt with exit code 0 after successfully importing a snapshot
    #[arg(long)]
    pub halt_after_import: bool,
    /// Import a chain from a local CAR file or URL
    #[arg(long)]
    pub import_chain: Option<String>,
    /// Skips loading CAR file and uses header to index chain. Assumes a
    /// pre-loaded database
    #[arg(long)]
    pub skip_load: Option<bool>,
    /// Number of tipsets requested over one chain exchange (default is 8)
    #[arg(long)]
    pub req_window: Option<usize>,
    /// Number of tipsets to include in the sample that determines what the
    /// network head is (default is 5)
    #[arg(long)]
    pub tipset_sample_size: Option<u8>,
    /// Amount of Peers we want to be connected to (default is 75)
    #[arg(long)]
    pub target_peer_count: Option<u32>,
    /// Encrypt the key-store (default: true)
    #[arg(long)]
    pub encrypt_keystore: Option<bool>,
    /// Choose network chain to sync to
    #[arg(long)]
    pub chain: Option<NetworkChain>,
    /// Daemonize Forest process
    #[arg(long)]
    pub detach: bool,
    /// Automatically download a chain specific snapshot to sync with the
    /// Filecoin network if needed.
    #[arg(long)]
    pub auto_download_snapshot: bool,
    /// Enable or disable colored logging in `stdout`
    #[arg(long, default_value = "auto")]
    pub color: LoggingColor,
    /// Turn on tokio-console support for debugging
    #[arg(long)]
    pub tokio_console: bool,
    /// Send telemetry to `grafana loki`
    #[arg(long)]
    pub loki: bool,
    /// Endpoint of `grafana loki`
    #[arg(long, default_value = "http://127.0.0.1:3100")]
    pub loki_endpoint: String,
    /// Specify a directory into which rolling log files should be appended
    #[arg(long)]
    pub log_dir: Option<PathBuf>,
    /// Exit after basic daemon initialization
    #[arg(long)]
    pub exit_after_init: bool,
    /// If provided, indicates the file to which to save the admin token.
    #[arg(long)]
    pub save_token: Option<PathBuf>,
    /// Track peak physical memory usage and print on exit
    #[arg(long)]
    pub track_peak_rss: bool,
    /// Disable the automatic database garbage collection.
    #[arg(long)]
    pub no_gc: bool,
    /// Check your command-line options and configuration file if one is used
    #[arg(long)]
    pub dry_run: bool,
    /// Skip loading actors from the actors bundle.
    #[arg(long)]
    pub skip_load_actors: bool,
}

impl CliOpts {
    pub fn to_config(&self) -> Result<(Config, Option<ConfigPath>), anyhow::Error> {
        let (path, mut cfg) = read_config(&self.config, &self.chain)?;

        if let Some(genesis_file) = &self.genesis {
            cfg.client.genesis_file = Some(genesis_file.to_owned());
        }
        if self.rpc.unwrap_or(cfg.client.enable_rpc) {
            cfg.client.enable_rpc = true;
            if let Some(rpc_address) = self.rpc_address {
                cfg.client.rpc_address = rpc_address;
            }

            if self.token.is_some() {
                cfg.client.rpc_token = self.token.to_owned();
            }
        } else {
            cfg.client.enable_rpc = false;
        }

        if self.no_metrics {
            cfg.client.enable_metrics_endpoint = false;
        } else {
            cfg.client.enable_metrics_endpoint = true;
            if let Some(metrics_address) = self.metrics_address {
                cfg.client.metrics_address = metrics_address;
            }
        }

        if self.import_snapshot.is_some() && self.import_chain.is_some() {
            anyhow::bail!("Can't set import_snapshot and import_chain at the same time!")
        } else if self.import_snapshot.is_some() && self.consume_snapshot.is_some() {
            anyhow::bail!("Can't set import_snapshot and consume_snapshot at the same time!")
        } else if self.consume_snapshot.is_some() && self.import_chain.is_some() {
            anyhow::bail!("Can't set consume_snapshot and import_chain at the same time!")
        }

        if let Some(snapshot_path) = &self.import_snapshot {
            cfg.client.snapshot_path = Some(snapshot_path.into());
            cfg.client.snapshot = true;
        }
        if let Some(snapshot_path) = &self.consume_snapshot {
            cfg.client.snapshot_path = Some(snapshot_path.into());
            cfg.client.snapshot = true;
            cfg.client.consume_snapshot = true;
        }
        if let Some(snapshot_path) = &self.import_chain {
            cfg.client.snapshot_path = Some(snapshot_path.into());
            cfg.client.snapshot = false;
        }
        cfg.client.snapshot_height = self.height;
        cfg.client.snapshot_head = self.head.map(|head| head as i64);
        if let Some(skip_load) = self.skip_load {
            cfg.client.skip_load = skip_load;
        }

        cfg.network.kademlia = self.kademlia.unwrap_or(cfg.network.kademlia);
        cfg.network.mdns = self.mdns.unwrap_or(cfg.network.mdns);
        if let Some(target_peer_count) = self.target_peer_count {
            cfg.network.target_peer_count = target_peer_count;
        }
        // (where to find these flags, should be easy to do with structops)

        // check and set syncing configurations
        if let Some(req_window) = self.req_window {
            cfg.sync.request_window = req_window;
        }
        if let Some(tipset_sample_size) = self.tipset_sample_size {
            cfg.sync.tipset_sample_size = tipset_sample_size.into();
        }
        if let Some(encrypt_keystore) = self.encrypt_keystore {
            cfg.client.encrypt_keystore = encrypt_keystore;
        }

        cfg.client.load_actors = !self.skip_load_actors;

        Ok((cfg, path))
    }
}

/// CLI RPC options
#[derive(Default, Debug, Parser)]
pub struct CliRpcOpts {
    /// Admin token to interact with the node
    #[arg(long)]
    pub token: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum ConfigPath {
    Cli(PathBuf),
    Env(PathBuf),
    Project(PathBuf),
}

impl ConfigPath {
    pub fn to_path_buf(&self) -> &PathBuf {
        match self {
            ConfigPath::Cli(path) => path,
            ConfigPath::Env(path) => path,
            ConfigPath::Project(path) => path,
        }
    }
}

pub fn find_config_path(config: &Option<String>) -> Option<ConfigPath> {
    if let Some(s) = config {
        return Some(ConfigPath::Cli(PathBuf::from(s)));
    }
    if let Ok(s) = std::env::var("FOREST_CONFIG_PATH") {
        return Some(ConfigPath::Env(PathBuf::from(s)));
    }
    if let Some(dir) = ProjectDirs::from("com", "ChainSafe", "Forest") {
        let path = dir.config_dir().join("config.toml");
        if path.exists() {
            return Some(ConfigPath::Project(path));
        }
    }
    None
}

fn find_unknown_keys<'a>(
    tables: Vec<&'a str>,
    x: &'a toml::Value,
    y: &'a toml::Value,
    result: &mut Vec<(Vec<&'a str>, &'a str)>,
) {
    if let (toml::Value::Table(x_map), toml::Value::Table(y_map)) = (x, y) {
        let x_set: HashSet<_> = x_map.keys().collect();
        let y_set: HashSet<_> = y_map.keys().collect();
        for k in x_set.difference(&y_set) {
            result.push((tables.clone(), k));
        }
        for (x_key, x_value) in x_map.iter() {
            if let Some(y_value) = y_map.get(x_key) {
                let mut copy = tables.clone();
                copy.push(x_key);
                find_unknown_keys(copy, x_value, y_value, result);
            }
        }
    }
    if let (toml::Value::Array(x_vec), toml::Value::Array(y_vec)) = (x, y) {
        for (x_value, y_value) in x_vec.iter().zip(y_vec.iter()) {
            find_unknown_keys(tables.clone(), x_value, y_value, result);
        }
    }
}

pub fn check_for_unknown_keys(path: &Path, config: &Config) {
    // `config` has been loaded successfully from toml file in `path` so we can
    // always serialize it back to a valid TOML value or get the TOML value from
    // `path`
    let file = read_file_to_string(path).unwrap();
    let value = file.parse::<toml::Value>().unwrap();

    let config_file = toml::to_string(config).unwrap();
    let config_value = config_file.parse::<toml::Value>().unwrap();

    let mut result = vec![];
    find_unknown_keys(vec![], &value, &config_value, &mut result);
    for (tables, k) in result.iter() {
        if tables.is_empty() {
            error!("Unknown key `{k}` in top-level table");
        } else {
            error!("Unknown key `{k}` in [{}]", tables.join("."));
        }
    }
    if !result.is_empty() {
        let path = path.display();
        cli_error_and_die(
            format!("Error checking {path}. Verify that all keys are valid"),
            1,
        )
    }
}

/// Print an error message and exit the program with an error code
/// Used for handling high level errors such as invalid parameters
pub fn cli_error_and_die(msg: impl AsRef<str>, code: i32) -> ! {
    error!("{}", msg.as_ref());
    std::process::exit(code);
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn find_unknown_keys_must_work() {
        let x: toml::Value = toml::from_str(
            r#"
            folklore = true
            foo = "foo"
            [myth]
            author = 'H. P. Lovecraft'
            entities = [
                { name = 'Cthulhu' },
                { name = 'Azathoth' },
                { baz = 'Dagon' },
            ]
            bar = "bar"
        "#,
        )
        .unwrap();

        let y: toml::Value = toml::from_str(
            r#"
            folklore = true
            [myth]
            author = 'H. P. Lovecraft'
            entities = [
                { name = 'Cthulhu' },
                { name = 'Azathoth' },
                { name = 'Dagon' },
            ]
        "#,
        )
        .unwrap();

        // No differences
        let mut result = vec![];
        find_unknown_keys(vec![], &y, &y, &mut result);
        assert!(result.is_empty());

        // 3 unknown keys
        let mut result = vec![];
        find_unknown_keys(vec![], &x, &y, &mut result);
        assert_eq!(
            result,
            vec![
                (vec![], "foo"),
                (vec!["myth"], "bar"),
                (vec!["myth", "entities"], "baz"),
            ]
        );
    }

    #[test]
    fn combination_of_import_snapshot_and_import_chain_should_fail() {
        // Creating a config with default cli options should succeed
        let options = CliOpts::default();
        assert!(options.to_config().is_ok());

        // Creating a config with both --import_snapshot and --import_chain should fail
        let options = CliOpts {
            import_snapshot: Some("snapshot.car".into()),
            import_chain: Some("snapshot.car".into()),
            ..Default::default()
        };
        assert!(options.to_config().is_err());

        // Creating a config with only --import_snapshot should succeed
        let options = CliOpts {
            import_snapshot: Some("snapshot.car".into()),
            ..Default::default()
        };
        assert!(options.to_config().is_ok());

        // Creating a config with only --import_chain should succeed
        let options = CliOpts {
            import_chain: Some("snapshot.car".into()),
            ..Default::default()
        };
        assert!(options.to_config().is_ok());
    }
}
