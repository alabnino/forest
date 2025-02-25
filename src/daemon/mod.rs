// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

pub mod bundle;
mod db_util;
pub mod main;

use crate::auth::{create_token, generate_priv_key, ADMIN, JWT_IDENTIFIER};
use crate::blocks::Tipset;
use crate::chain::ChainStore;
use crate::chain_sync::ChainMuxer;
use crate::cli_shared::snapshot;
use crate::cli_shared::{
    chain_path,
    cli::{CliOpts, Config},
};

use crate::daemon::db_util::{import_chain_as_forest_car, load_all_forest_cars};
use crate::db::car::ManyCar;
use crate::db::db_engine::{db_root, open_db};
use crate::db::MarkAndSweep;
use crate::genesis::{get_network_name_from_genesis, read_genesis_header};
use crate::key_management::{
    KeyStore, KeyStoreConfig, ENCRYPTED_KEYSTORE_NAME, FOREST_KEYSTORE_PHRASE_ENV,
};
use crate::libp2p::{Libp2pConfig, Libp2pService, PeerManager};
use crate::message_pool::{MessagePool, MpoolConfig, MpoolRpcProvider};
use crate::networks::{ChainConfig, NetworkChain};
use crate::rpc::start_rpc;
use crate::rpc_api::data_types::RPCState;
use crate::shim::address::{CurrentNetwork, Network};
use crate::shim::clock::ChainEpoch;
use crate::shim::version::NetworkVersion;
use crate::state_manager::StateManager;
use crate::utils::{
    monitoring::MemStatsTracker, proofs_api::paramfetch::ensure_params_downloaded,
    version::FOREST_VERSION_STRING,
};
use anyhow::{bail, Context as _};
use bundle::load_actor_bundles;
use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use futures::{select, Future, FutureExt};
use once_cell::sync::Lazy;
use raw_sync_2::events::{Event, EventInit as _, EventState};
use shared_memory::ShmemConf;
use std::path::Path;
use std::time::Duration;
use std::{cell::RefCell, cmp, path::PathBuf, sync::Arc};
use tempfile::{Builder, TempPath};
use tokio::{
    net::TcpListener,
    signal::{
        ctrl_c,
        unix::{signal, SignalKind},
    },
    sync::{mpsc, RwLock},
    task::JoinSet,
};
use tracing::{debug, info, warn};

static IPC_PATH: Lazy<TempPath> = Lazy::new(|| {
    Builder::new()
        .prefix("forest-ipc")
        .tempfile()
        .expect("tempfile must succeed")
        .into_temp_path()
});

// The parent process and the daemonized child communicate through an Event in
// shared memory. The identity of the shared memory object is written to a
// temporary file. The parent process is responsible for cleaning up the file
// and the shared memory object.
pub fn ipc_shmem_conf() -> ShmemConf {
    ShmemConf::new()
        .size(Event::size_of(None))
        .force_create_flink()
        .flink(IPC_PATH.as_os_str())
}

fn unblock_parent_process() -> anyhow::Result<()> {
    let shmem = ipc_shmem_conf().open()?;
    let (event, _) =
        unsafe { Event::from_existing(shmem.as_ptr()).map_err(|err| anyhow::anyhow!("{err}")) }?;

    event
        .set(EventState::Signaled)
        .map_err(|err| anyhow::anyhow!("{err}"))
}

/// Increase the file descriptor limit to a reasonable number.
/// This prevents the node from failing if the default soft limit is too low.
/// Note that the value is only increased, never decreased.
fn maybe_increase_fd_limit() -> anyhow::Result<()> {
    static DESIRED_SOFT_LIMIT: u64 = 8192;
    let (soft_before, _) = rlimit::Resource::NOFILE.get()?;

    let soft_after = rlimit::increase_nofile_limit(DESIRED_SOFT_LIMIT)?;
    if soft_before < soft_after {
        debug!("Increased file descriptor limit from {soft_before} to {soft_after}");
    }
    if soft_after < DESIRED_SOFT_LIMIT {
        warn!(
            "File descriptor limit is too low: {soft_after} < {DESIRED_SOFT_LIMIT}. \
            You may encounter 'too many open files' errors.",
        );
    }

    Ok(())
}

// Start the daemon and abort if we're interrupted by ctrl-c, SIGTERM, or `forest-cli shutdown`.
pub async fn start_interruptable(opts: CliOpts, config: Config) -> anyhow::Result<()> {
    let mut terminate = signal(SignalKind::terminate())?;
    let (shutdown_send, mut shutdown_recv) = mpsc::channel(1);

    let result = tokio::select! {
        ret = start(opts, config, shutdown_send) => ret,
        _ = ctrl_c() => {
            info!("Keyboard interrupt.");
            Ok(())
        },
        _ = terminate.recv() => {
            info!("Received SIGTERM.");
            Ok(())
        },
        _ = shutdown_recv.recv() => {
            info!("Client requested a shutdown.");
            Ok(())
        },
    };
    crate::utils::io::terminal_cleanup();
    result
}

// Garbage collection interval, currently set at 10 hours.
const GC_INTERVAL: Duration = Duration::from_secs(60 * 60 * 10);

/// Starts daemon process
pub(super) async fn start(
    opts: CliOpts,
    config: Config,
    shutdown_send: mpsc::Sender<()>,
) -> anyhow::Result<()> {
    let chain_config = Arc::new(ChainConfig::from_chain(&config.chain));
    if chain_config.is_testnet() {
        CurrentNetwork::set_global(Network::Testnet);
    }

    info!(
        "Starting Forest daemon, version {}",
        FOREST_VERSION_STRING.as_str()
    );
    maybe_increase_fd_limit()?;

    let start_time = chrono::Utc::now();
    let path: PathBuf = config.client.data_dir.join("libp2p");
    let net_keypair = crate::libp2p::keypair::get_or_create_keypair(&path)?;

    let mut keystore = load_or_create_keystore(&config).await?;

    if keystore.get(JWT_IDENTIFIER).is_err() {
        keystore.put(JWT_IDENTIFIER, generate_priv_key())?;
    }

    handle_admin_token(&opts, &config, &keystore)?;

    let keystore = Arc::new(RwLock::new(keystore));

    let chain_data_path = chain_path(&config);

    // Try to migrate the database if needed. In case the migration fails, we fallback to creating a new database
    // to avoid breaking the node.
    let db_migration = crate::db::migration::DbMigration::new(chain_data_path.clone());
    if let Err(e) = db_migration.migrate() {
        warn!("Failed to migrate database: {e}");
    }

    let db_root_dir = db_root(&chain_data_path)?;
    let db_writer = Arc::new(open_db(db_root_dir.clone(), config.db_config().clone())?);
    let db = Arc::new(ManyCar::new(db_writer.clone()));
    let forest_car_db_dir = db_root_dir.join("car_db");
    load_all_forest_cars(&db, &forest_car_db_dir)?;

    if config.client.load_actors {
        load_actor_bundles(&db, &config.chain).await?;
    }

    let mut services = JoinSet::new();

    if opts.track_peak_rss {
        let mem_stats_tracker = MemStatsTracker::default();
        services.spawn(async move {
            mem_stats_tracker.run_loop().await;
            Ok(())
        });
    }

    if config.client.enable_metrics_endpoint {
        // Start Prometheus server port
        let prometheus_listener = TcpListener::bind(config.client.metrics_address)
            .await
            .context(format!(
                "could not bind to {}",
                config.client.metrics_address
            ))?;
        info!(
            "Prometheus server started at {}",
            config.client.metrics_address
        );
        let db_directory = crate::db::db_engine::db_root(&chain_path(&config))?;
        let db = db.writer().clone();
        services.spawn(async {
            crate::metrics::init_prometheus(prometheus_listener, db_directory, db)
                .await
                .context("Failed to initiate prometheus server")
        });
    }

    // Read Genesis file
    // * When snapshot command implemented, this genesis does not need to be
    //   initialized
    let genesis_header = read_genesis_header(
        config.client.genesis_file.as_ref(),
        chain_config.genesis_bytes(&db).await?.as_deref(),
        &db,
    )
    .await?;

    // Initialize ChainStore
    let chain_store = Arc::new(ChainStore::new(
        Arc::clone(&db),
        db.writer().clone(),
        chain_config.clone(),
        genesis_header.clone(),
    )?);

    if !opts.no_gc {
        let mut db_garbage_collector = {
            let chain_store = chain_store.clone();
            let depth = cmp::max(
                chain_config.policy.chain_finality * 2,
                config.sync.recent_state_roots,
            );

            let get_heaviest_tipset = Box::new(move || chain_store.heaviest_tipset());

            MarkAndSweep::new(
                db_writer,
                get_heaviest_tipset,
                depth,
                Duration::from_secs(chain_config.block_delay_secs as u64),
            )
        };
        services.spawn(async move { db_garbage_collector.gc_loop(GC_INTERVAL).await });
    }

    let publisher = chain_store.publisher();

    // Initialize StateManager
    let sm = StateManager::new(
        Arc::clone(&chain_store),
        Arc::clone(&chain_config),
        Arc::new(config.sync.clone()),
    )?;

    let state_manager = Arc::new(sm);

    let network_name = get_network_name_from_genesis(&genesis_header, &state_manager)?;

    info!("Using network :: {}", get_actual_chain_name(&network_name));
    display_chain_logo(&config.chain);
    let (tipset_sink, tipset_stream) = flume::bounded(20);

    // if bootstrap peers are not set, set them
    let config = if config.network.bootstrap_peers.is_empty() {
        let bootstrap_peers = chain_config.bootstrap_peers.clone();

        Config {
            network: Libp2pConfig {
                bootstrap_peers,
                ..config.network
            },
            ..config
        }
    } else {
        config
    };

    if opts.exit_after_init {
        return Ok(());
    }

    let epoch = chain_store.heaviest_tipset().epoch();

    let peer_manager = Arc::new(PeerManager::default());
    services.spawn(peer_manager.clone().peer_operation_event_loop_task());
    let genesis_cid = *genesis_header.cid();
    // Libp2p service setup
    let p2p_service = Libp2pService::new(
        config.network.clone(),
        Arc::clone(&chain_store),
        peer_manager.clone(),
        net_keypair,
        &network_name,
        genesis_cid,
    )
    .await?;

    let network_rx = p2p_service.network_receiver();
    let network_send = p2p_service.network_sender();

    // Initialize mpool
    let provider = MpoolRpcProvider::new(publisher.clone(), Arc::clone(&state_manager));
    let mpool = MessagePool::new(
        provider,
        network_name.clone(),
        network_send.clone(),
        MpoolConfig::load_config(db.writer().as_ref())?,
        state_manager.chain_config().clone(),
        &mut services,
    )?;

    let mpool = Arc::new(mpool);

    // Initialize ChainMuxer
    let chain_muxer = ChainMuxer::new(
        Arc::clone(&state_manager),
        peer_manager,
        mpool.clone(),
        network_send.clone(),
        network_rx,
        Arc::new(Tipset::from(genesis_header)),
        tipset_sink,
        tipset_stream,
    )?;
    let bad_blocks = chain_muxer.bad_blocks_cloned();
    let sync_state = chain_muxer.sync_state_cloned();
    services.spawn(async { Err(anyhow::anyhow!("{}", chain_muxer.await)) });

    // Start services
    if config.client.enable_rpc {
        let keystore_rpc = Arc::clone(&keystore);
        let rpc_listen = tokio::net::TcpListener::bind(config.client.rpc_address)
            .await
            .context(format!(
                "could not bind to rpc address {}",
                config.client.rpc_address
            ))?;

        let rpc_state_manager = Arc::clone(&state_manager);
        let rpc_chain_store = Arc::clone(&chain_store);

        services.spawn(async move {
            info!("JSON-RPC endpoint started at {}", config.client.rpc_address);
            let beacon = Arc::new(
                rpc_state_manager
                    .chain_config()
                    .get_beacon_schedule(chain_store.genesis().timestamp()),
            );
            start_rpc(
                Arc::new(RPCState {
                    state_manager: Arc::clone(&rpc_state_manager),
                    keystore: keystore_rpc,
                    mpool,
                    bad_blocks,
                    sync_state,
                    network_send,
                    network_name,
                    start_time,
                    beacon,
                    chain_store: rpc_chain_store,
                }),
                rpc_listen,
                FOREST_VERSION_STRING.as_str(),
                shutdown_send,
            )
            .await
            .map_err(|err| anyhow::anyhow!("{:?}", serde_json::to_string(&err)))
        });
    } else {
        debug!("RPC disabled.");
    };

    if opts.detach {
        unblock_parent_process()?;
    }

    // Sets proof parameter file download path early, the files will be checked and
    // downloaded later right after snapshot import step
    crate::utils::proofs_api::paramfetch::set_proofs_parameter_cache_dir_env(
        &config.client.data_dir,
    );

    // Sets the latest snapshot if needed for downloading later
    let mut config = config;
    if config.client.snapshot_path.is_none() {
        set_snapshot_path_if_needed(
            &mut config,
            &chain_config,
            epoch,
            opts.auto_download_snapshot,
            &db_root_dir,
        )
        .await?;
    }

    // Import chain if needed
    if !opts.skip_load.unwrap_or_default() {
        if let Some(path) = &config.client.snapshot_path {
            let (car_db_path, ts) = import_chain_as_forest_car(
                path,
                &forest_car_db_dir,
                config.client.consume_snapshot,
            )
            .await?;
            db.read_only_files(std::iter::once(car_db_path.clone()))?;
            debug!("Loaded car DB at {}", car_db_path.display());
            state_manager
                .chain_store()
                .set_heaviest_tipset(Arc::new(ts))?;
        }
    }

    if let (true, Some(validate_from)) = (config.client.snapshot, config.client.snapshot_height) {
        // We've been provided a snapshot and asked to validate it
        ensure_params_downloaded().await?;
        // Use the specified HEAD, otherwise take the current HEAD.
        let current_height = config
            .client
            .snapshot_head
            .unwrap_or(state_manager.chain_store().heaviest_tipset().epoch());
        assert!(current_height.is_positive());
        match validate_from.is_negative() {
            // allow --height=-1000 to scroll back from the current head
            true => {
                state_manager.validate_range((current_height + validate_from)..=current_height)?
            }
            false => state_manager.validate_range(validate_from..=current_height)?,
        }
    }

    // Halt
    if opts.halt_after_import {
        // Cancel all async services
        services.shutdown().await;
        return Ok(());
    }

    ensure_params_downloaded().await?;
    services.spawn(p2p_service.run());

    // blocking until any of the services returns an error,
    propagate_error(&mut services)
        .await
        .context("services failure")
        .map(|_| {})
}

/// If our current chain is below a supported height, we need a snapshot to bring it up
/// to a supported height. If we've not been given a snapshot by the user, get one.
///
/// An [`Err`] should be considered fatal.
async fn set_snapshot_path_if_needed(
    config: &mut Config,
    chain_config: &ChainConfig,
    epoch: ChainEpoch,
    auto_download_snapshot: bool,
    download_directory: &Path,
) -> anyhow::Result<()> {
    if !download_directory.is_dir() {
        anyhow::bail!(
            "`download_directory` does not exist: {}",
            download_directory.display()
        );
    }

    let vendor = snapshot::TrustedVendor::default();
    let chain = &config.chain;

    // What height is our chain at right now, and what network version does that correspond to?
    let network_version = chain_config.network_version(epoch);
    let network_version_is_small = network_version < NetworkVersion::V16;

    // We don't support small network versions (we can't validate from e.g genesis).
    // So we need a snapshot (which will be from a recent network version)
    let require_a_snapshot = network_version_is_small;
    let have_a_snapshot = config.client.snapshot_path.is_some();

    match (require_a_snapshot, have_a_snapshot, auto_download_snapshot) {
        (false, _, _) => {}   // noop - don't need a snapshot
        (true, true, _) => {} // noop - we need a snapshot, and we have one
        (true, false, true) => {
            let url = crate::cli_shared::snapshot::stable_url(vendor, chain)?;
            config.client.snapshot_path = Some(url.to_string().into());
            config.client.snapshot = true;
        }
        (true, false, false) => {
            // we need a snapshot, don't have one, and don't have permission to download one, so ask the user
            let url = crate::cli_shared::snapshot::stable_url(vendor, chain)?;
            let (num_bytes, _path) = crate::cli_shared::snapshot::peek(vendor, chain)
                .await
                .context("couldn't get snapshot size")?;
            // dialoguer will double-print long lines, so manually print the first clause ourselves,
            // then let `Confirm` handle the second.
            println!("Forest requires a snapshot to sync with the network, but automatic fetching is disabled.");
            let message = format!(
                "Fetch a {} snapshot to the current directory? (denying will exit the program). ",
                indicatif::HumanBytes(num_bytes)
            );
            let have_permission = asyncify(|| {
                dialoguer::Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt(message)
                    .default(false)
                    .interact()
                    // e.g not a tty (or some other error), so haven't got permission.
                    .unwrap_or(false)
            })
            .await;
            if !have_permission {
                bail!("Forest requires a snapshot to sync with the network, but automatic fetching is disabled.")
            }
            config.client.snapshot_path = Some(url.to_string().into());
            config.client.snapshot = true;
        }
    };

    Ok(())
}

/// Generates, prints and optionally writes to a file the administrator JWT
/// token.
fn handle_admin_token(opts: &CliOpts, config: &Config, keystore: &KeyStore) -> anyhow::Result<()> {
    let ki = keystore.get(JWT_IDENTIFIER)?;
    let token_exp = config.client.token_exp;
    let token = create_token(
        ADMIN.iter().map(ToString::to_string).collect(),
        ki.private_key(),
        token_exp,
    )?;
    info!("Admin token: {token}");
    if let Some(path) = opts.save_token.as_ref() {
        std::fs::write(path, token)?;
    }

    Ok(())
}

/// returns the first error with which any of the services end, or never returns at all
// This should return anyhow::Result<!> once the `Never` type is stabilized
async fn propagate_error(
    services: &mut JoinSet<Result<(), anyhow::Error>>,
) -> anyhow::Result<std::convert::Infallible> {
    while !services.is_empty() {
        select! {
            option = services.join_next().fuse() => {
                if let Some(Ok(Err(error_message))) = option {
                    return Err(error_message)
                }
            },
        }
    }
    std::future::pending().await
}

pub fn get_actual_chain_name(internal_network_name: &str) -> &str {
    match internal_network_name {
        "testnetnet" => "mainnet",
        "calibrationnet" => "calibnet",
        _ => internal_network_name,
    }
}

/// This may:
/// - create a [`KeyStore`]
/// - load a [`KeyStore`]
/// - ask a user for password input
async fn load_or_create_keystore(config: &Config) -> anyhow::Result<KeyStore> {
    use std::env::VarError;

    let passphrase_from_env = std::env::var(FOREST_KEYSTORE_PHRASE_ENV);
    let require_encryption = config.client.encrypt_keystore;
    let keystore_already_exists = config
        .client
        .data_dir
        .join(ENCRYPTED_KEYSTORE_NAME)
        .is_dir();

    match (require_encryption, passphrase_from_env) {
        // don't need encryption, we can implicitly create a keystore
        (false, maybe_passphrase) => {
            warn!("Forest has encryption disabled");
            if let Ok(_) | Err(VarError::NotUnicode(_)) = maybe_passphrase {
                warn!(
                    "Ignoring passphrase provided in {} - encryption is disabled",
                    FOREST_KEYSTORE_PHRASE_ENV
                )
            }
            KeyStore::new(KeyStoreConfig::Persistent(config.client.data_dir.clone()))
                .map_err(anyhow::Error::new)
        }

        // need encryption, the user has provided the password through env
        (true, Ok(passphrase)) => KeyStore::new(KeyStoreConfig::Encrypted(
            config.client.data_dir.clone(),
            passphrase,
        ))
        .map_err(anyhow::Error::new),

        // need encryption, we've not been given a password
        (true, Err(error)) => {
            // prompt for passphrase and try and load the keystore

            if let VarError::NotUnicode(_) = error {
                // If we're ignoring the user's password, tell them why
                warn!(
                    "Ignoring passphrase provided in {} - it's not utf-8",
                    FOREST_KEYSTORE_PHRASE_ENV
                )
            }

            let data_dir = config.client.data_dir.clone();

            match keystore_already_exists {
                true => asyncify(move || input_password_to_load_encrypted_keystore(data_dir))
                    .await
                    .context("Couldn't load keystore"),
                false => {
                    let password =
                        asyncify(|| create_password("Create a password for Forest's keystore"))
                            .await?;
                    KeyStore::new(KeyStoreConfig::Encrypted(data_dir, password))
                        .context("Couldn't create keystore")
                }
            }
        }
    }
}

/// Run the closure on a thread where blocking is allowed
///
/// # Panics
/// If the closure panics
fn asyncify<T>(f: impl FnOnce() -> T + Send + 'static) -> impl Future<Output = T>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f).then(|res| async { res.expect("spawned task panicked") })
}

/// Prompts for password, looping until the [`KeyStore`] is successfully loaded.
///
/// This code makes blocking syscalls.
fn input_password_to_load_encrypted_keystore(data_dir: PathBuf) -> dialoguer::Result<KeyStore> {
    let keystore = RefCell::new(None);
    let term = Term::stderr();

    // Unlike `dialoguer::Confirm`, `dialoguer::Password` doesn't fail if the terminal is not a tty
    // so do that check ourselves.
    // This means users can't pipe their password from stdin.
    if !term.is_term() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "cannot read password from non-terminal",
        )
        .into());
    }

    dialoguer::Password::new()
        .with_prompt("Enter the password for Forest's keystore")
        .allow_empty_password(true) // let validator do validation
        .validate_with(|input: &String| {
            KeyStore::new(KeyStoreConfig::Encrypted(data_dir.clone(), input.clone()))
                .map(|created| *keystore.borrow_mut() = Some(created))
                .context(
                    "Error: couldn't load keystore with this password. Try again or press Ctrl+C to abort.",
                )
        })
        .interact_on(&term)?;

    Ok(keystore
        .into_inner()
        .expect("validation succeeded, so keystore must be emplaced"))
}

/// Loops until the user provides two matching passwords.
///
/// This code makes blocking syscalls
fn create_password(prompt: &str) -> dialoguer::Result<String> {
    let term = Term::stderr();

    // Unlike `dialoguer::Confirm`, `dialoguer::Password` doesn't fail if the terminal is not a tty
    // so do that check ourselves.
    // This means users can't pipe their password from stdin.
    if !term.is_term() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "cannot read password from non-terminal",
        )
        .into());
    }
    dialoguer::Password::new()
        .with_prompt(prompt)
        .allow_empty_password(false)
        .with_confirmation(
            "Confirm password",
            "Error: the passwords do not match. Try again or press Ctrl+C to abort.",
        )
        .interact_on(&term)
}

/// Displays the network logo/ASCII art if available.
fn display_chain_logo(chain: &NetworkChain) {
    let logo = match chain {
        NetworkChain::Butterflynet => {
            Some(include_str!("../../build/ascii-art/butterfly").to_string())
        }
        _ => None,
    };
    if let Some(logo) = logo {
        info!("\n{logo}");
    }
}
