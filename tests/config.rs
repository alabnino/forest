// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT
use std::{io::Write, net::SocketAddr, path::PathBuf, str::FromStr};

use anyhow::*;
use assert_cmd::Command;
use forest_filecoin::{Client, Config};
use rand::Rng;
use tempfile::TempDir;

#[test]
fn test_config_subcommand_produces_valid_toml_configuration_dump() {
    let cmd = Command::cargo_bin("forest-tool")
        .unwrap()
        .arg("config")
        .arg("dump")
        .arg("--rpc")
        .arg("true")
        .arg("--token")
        .arg("Azazello")
        .assert()
        .success();

    let output = &cmd.get_output().stdout;
    toml::from_str::<Config>(std::str::from_utf8(output).unwrap()).expect("Invalid configuration!");
}

#[test]
fn test_overrides_are_reflected_in_configuration_dump() {
    let mut rng = rand::thread_rng();
    let randomized_metrics_host = format!("127.0.0.1:{}", rng.gen::<u16>());

    let cmd = Command::cargo_bin("forest-tool")
        .unwrap()
        .arg("config")
        .arg("dump")
        .arg("--rpc")
        .arg("true")
        .arg("--token")
        .arg("Azazello")
        .arg("--metrics-address")
        .arg(&randomized_metrics_host)
        .assert()
        .success();

    let output = &cmd.get_output().stdout;
    let config = toml::from_str::<Config>(std::str::from_utf8(output).unwrap())
        .expect("Invalid configuration!");

    assert_eq!(
        config.client.metrics_address,
        SocketAddr::from_str(&randomized_metrics_host).unwrap()
    );
}

#[test]
fn test_reading_configuration_from_file() {
    let mut rng = rand::thread_rng();

    let expected_config = Config {
        client: Client {
            rpc_token: Some("Azazello".into()),
            genesis_file: Some("cthulhu".into()),
            encrypt_keystore: false,
            metrics_address: SocketAddr::from_str(&format!("127.0.0.1:{}", rng.gen::<u16>()))
                .unwrap(),
            ..Client::default()
        },
        ..Config::default()
    };

    let mut config_file = tempfile::Builder::new().tempfile().unwrap();
    config_file
        .write_all(toml::to_string(&expected_config).unwrap().as_bytes())
        .expect("Failed writing configuration!");

    let cmd = Command::cargo_bin("forest-tool")
        .unwrap()
        .arg("config")
        .arg("dump")
        .arg("--rpc")
        .arg("true")
        .arg("--token")
        .arg("Azazello")
        .arg("--config")
        .arg(config_file.path())
        .assert()
        .success();

    let output = &cmd.get_output().stdout;
    let actual_config = toml::from_str::<Config>(std::str::from_utf8(output).unwrap())
        .expect("Invalid configuration!");

    assert!(expected_config == actual_config);
}

#[test]
fn test_config_env_var() -> Result<()> {
    let expected_config = Config {
        client: Client {
            rpc_token: Some("some_rpc_token".into()),
            data_dir: PathBuf::from("some_path_buf"),
            ..Client::default()
        },
        ..Config::default()
    };

    let mut config_file = tempfile::Builder::new().tempfile()?;
    config_file
        .write_all(toml::to_string(&expected_config)?.as_bytes())
        .context("Failed writing configuration!")?;

    let cmd = Command::cargo_bin("forest-tool")
        .unwrap()
        .env("FOREST_CONFIG_PATH", config_file.path())
        .arg("config")
        .arg("dump")
        .assert()
        .success();

    let output = &cmd.get_output().stdout;
    let actual_config =
        toml::from_str::<Config>(std::str::from_utf8(output)?).context("Invalid configuration!")?;

    ensure!(expected_config == actual_config);

    Ok(())
}

#[test]
fn test_download_location_of_proof_parameter_files_env() {
    let tmp_dir = TempDir::new().unwrap();

    Command::cargo_bin("forest-tool")
        .unwrap()
        .env("FIL_PROOFS_PARAMETER_CACHE", tmp_dir.path())
        .arg("fetch-params")
        .arg("--keys")
        .arg("--dry-run")
        .assert()
        .stdout(tmp_dir.into_path().to_string_lossy().into_owned() + "\n")
        .success();
}

#[test]
fn test_download_location_of_proof_parameter_files_default() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp_param_dir = tmp_dir.path().join("filecoin-proof-parameters");
    let config = Config {
        client: Client {
            data_dir: tmp_dir.path().to_path_buf(),
            ..Client::default()
        },
        ..Config::default()
    };

    let mut config_file = tempfile::Builder::new().tempfile().unwrap();
    config_file
        .write_all(toml::to_string(&config).unwrap().as_bytes())
        .expect("Failed writing configuration!");

    Command::cargo_bin("forest-tool")
        .unwrap()
        .env("FOREST_CONFIG_PATH", config_file.path())
        .arg("fetch-params")
        .arg("--keys")
        .arg("--dry-run")
        .assert()
        .stdout(tmp_param_dir.to_string_lossy().into_owned() + "\n")
        .success();
}
