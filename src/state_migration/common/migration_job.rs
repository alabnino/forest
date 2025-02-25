// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::sync::Arc;

use crate::shim::{address::Address, clock::ChainEpoch, state_tree::ActorState};
use fvm_ipld_blockstore::Blockstore;

use super::{ActorMigration, ActorMigrationInput, MigrationCache};

/// Defines migration result for a single actor migration.
#[derive(Debug)]
pub(in crate::state_migration) struct MigrationJobOutput {
    pub address: Address,
    pub actor_state: ActorState,
}

/// Defines migration job for a single actor migration.
pub(in crate::state_migration) struct MigrationJob<BS: Blockstore> {
    pub address: Address,
    pub actor_state: ActorState,
    pub actor_migration: Arc<dyn ActorMigration<BS>>,
}

impl<BS: Blockstore> MigrationJob<BS> {
    pub(in crate::state_migration) fn run(
        &self,
        store: &BS,
        prior_epoch: ChainEpoch,
        cache: MigrationCache,
    ) -> anyhow::Result<Option<MigrationJobOutput>> {
        if let Some(result) = self
            .actor_migration
            .migrate_state(
                store,
                ActorMigrationInput {
                    address: self.address,
                    balance: self.actor_state.balance.clone().into(),
                    head: self.actor_state.state,
                    prior_epoch,
                    cache,
                },
            )
            .map_err(|e| {
                anyhow::anyhow!(
                    "state migration failed for {} actor, addr {}:{}",
                    self.actor_state.code,
                    self.address,
                    e
                )
            })?
        {
            Ok(Some(MigrationJobOutput {
                address: self.address,
                actor_state: ActorState::new(
                    result.new_code_cid,
                    result.new_head,
                    self.actor_state.balance.clone().into(),
                    self.actor_state.sequence,
                    self.actor_state.delegated_address.map(Address::from),
                ),
            }))
        } else {
            Ok(None)
        }
    }
}
