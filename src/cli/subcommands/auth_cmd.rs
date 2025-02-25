// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use crate::auth::*;
use crate::rpc_client::{ApiInfo, JsonRpcError};
use chrono::Duration;
use clap::Subcommand;
use std::str::FromStr;

use super::print_rpc_res_bytes;

#[derive(Debug, Subcommand)]
pub enum AuthCommands {
    /// Create a new Authentication token with given permission
    CreateToken {
        /// Permission to assign to the token, one of: read, write, sign, admin
        #[arg(short, long)]
        perm: String,
        /// Token is revoked after this duration
        #[arg(long, default_value_t = humantime::Duration::from_str("2 months").expect("infallible"))]
        expire_in: humantime::Duration,
    },
    /// Get RPC API Information
    ApiInfo {
        /// permission to assign the token, one of: read, write, sign, admin
        #[arg(short, long)]
        perm: String,
        /// Token is revoked after this duration
        #[arg(long, default_value_t = humantime::Duration::from_str("2 months").expect("infallible"))]
        expire_in: humantime::Duration,
    },
}

fn process_perms(perm: String) -> Result<Vec<String>, JsonRpcError> {
    Ok(match perm.as_str() {
        "admin" => ADMIN,
        "sign" => SIGN,
        "write" => WRITE,
        "read" => READ,
        _ => return Err(JsonRpcError::INVALID_PARAMS),
    }
    .iter()
    .map(ToString::to_string)
    .collect())
}

impl AuthCommands {
    pub async fn run(self, api: ApiInfo) -> anyhow::Result<()> {
        match self {
            Self::CreateToken { perm, expire_in } => {
                let perm: String = perm.parse()?;
                let perms = process_perms(perm)?;
                let token_exp = Duration::from_std(expire_in.into())?;
                print_rpc_res_bytes(api.auth_new(perms, token_exp).await?)
            }
            Self::ApiInfo { perm, expire_in } => {
                let perm: String = perm.parse()?;
                let perms = process_perms(perm)?;
                let token_exp = Duration::from_std(expire_in.into())?;
                let token = api.auth_new(perms, token_exp).await?;
                let new_api = ApiInfo {
                    token: Some(String::from_utf8(token)?),
                    ..api
                };
                println!("FULLNODE_API_INFO=\"{}\"", new_api);
                Ok(())
            }
        }
    }
}
