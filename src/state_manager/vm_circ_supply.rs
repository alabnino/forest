// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::sync::Arc;

use crate::chain::*;
use crate::networks::{ChainConfig, Height};
use crate::rpc_api::data_types::CirculatingSupply;
use crate::shim::{
    address::Address,
    clock::{ChainEpoch, EPOCHS_IN_DAY},
    econ::{TokenAmount, TOTAL_FILECOIN},
    state_tree::{ActorState, StateTree},
};
use anyhow::{bail, Context as _};
use cid::Cid;
use fil_actor_interface::{
    is_account_actor, is_eth_account_actor, is_evm_actor, is_miner_actor, is_multisig_actor,
    is_paych_actor, is_placeholder_actor,
};
use fil_actor_interface::{market, miner, multisig, power, reward};
use fvm_ipld_blockstore::Blockstore;
use num_traits::Zero;

const EPOCHS_IN_YEAR: ChainEpoch = 365 * EPOCHS_IN_DAY;
const PRE_CALICO_VESTING: [(ChainEpoch, usize); 5] = [
    (183 * EPOCHS_IN_DAY, 82_717_041),
    (EPOCHS_IN_YEAR, 22_421_712),
    (2 * EPOCHS_IN_YEAR, 7_223_364),
    (3 * EPOCHS_IN_YEAR, 87_637_883),
    (6 * EPOCHS_IN_YEAR, 400_000_000),
];
const CALICO_VESTING: [(ChainEpoch, usize); 6] = [
    (0, 10_632_000),
    (183 * EPOCHS_IN_DAY, 19_015_887 + 32_787_700),
    (EPOCHS_IN_YEAR, 22_421_712 + 9_400_000),
    (2 * EPOCHS_IN_YEAR, 7_223_364),
    (3 * EPOCHS_IN_YEAR, 87_637_883 + 898_958),
    (6 * EPOCHS_IN_YEAR, 100_000_000 + 300_000_000 + 9_805_053),
];

/// Genesis information used when calculating circulating supply.
#[derive(Default, Clone)]
pub struct GenesisInfo {
    vesting: GenesisInfoVesting,

    /// info about the Accounts in the genesis state
    genesis_pledge: TokenAmount,
    genesis_market_funds: TokenAmount,

    /// Heights epoch
    ignition_height: ChainEpoch,
    actors_v2_height: ChainEpoch,
    calico_height: ChainEpoch,
}

impl GenesisInfo {
    pub fn from_chain_config(chain_config: &ChainConfig) -> Self {
        let ignition_height = chain_config.epoch(Height::Ignition);
        let actors_v2_height = chain_config.epoch(Height::ActorsV2);
        let liftoff_height = chain_config.epoch(Height::Liftoff);
        let calico_height = chain_config.epoch(Height::Calico);
        Self {
            ignition_height,
            actors_v2_height,
            calico_height,
            vesting: GenesisInfoVesting::new(liftoff_height),
            ..GenesisInfo::default()
        }
    }

    // Allows generation of the current circulating supply
    pub fn get_vm_circulating_supply<DB: Blockstore>(
        &self,
        height: ChainEpoch,
        db: &Arc<DB>,
        root: &Cid,
    ) -> Result<TokenAmount, anyhow::Error> {
        let detailed = self.get_vm_circulating_supply_detailed(height, db, root)?;

        Ok(detailed.fil_circulating)
    }

    pub fn get_vm_circulating_supply_detailed<DB: Blockstore>(
        &self,
        height: ChainEpoch,
        db: &Arc<DB>,
        root: &Cid,
    ) -> anyhow::Result<CirculatingSupply> {
        let state_tree = StateTree::new_from_root(Arc::clone(db), root)?;

        let fil_vested = get_fil_vested(self, height);
        let fil_mined = get_fil_mined(&state_tree)?;
        let fil_burnt = get_fil_burnt(&state_tree)?;
        let fil_locked = get_fil_locked(&state_tree)?;
        let fil_reserve_disbursed = if height > self.actors_v2_height {
            get_fil_reserve_disbursed(&state_tree)?
        } else {
            TokenAmount::default()
        };
        let fil_circulating = TokenAmount::max(
            &fil_vested + &fil_mined + &fil_reserve_disbursed - &fil_burnt - &fil_locked,
            TokenAmount::default(),
        );
        Ok(CirculatingSupply {
            fil_vested,
            fil_mined,
            fil_burnt,
            fil_locked,
            fil_circulating,
            fil_reserve_disbursed,
        })
    }

    // This can be a lengthy operation
    pub fn get_circulating_supply<DB: Blockstore>(
        &self,
        height: ChainEpoch,
        db: &Arc<DB>,
        root: &Cid,
    ) -> Result<TokenAmount, anyhow::Error> {
        let mut circ = TokenAmount::default();
        let mut un_circ = TokenAmount::default();

        let state_tree = StateTree::new_from_root(Arc::clone(db), root)?;

        state_tree.for_each(|addr: Address, actor: &ActorState| {
            let actor_balance = TokenAmount::from(actor.balance.clone());
            if !actor_balance.is_zero() {
                match addr {
                    Address::INIT_ACTOR
                    | Address::REWARD_ACTOR
                    | Address::VERIFIED_REGISTRY_ACTOR
                    // The power actor itself should never receive funds
                    | Address::POWER_ACTOR
                    | Address::SYSTEM_ACTOR
                    | Address::CRON_ACTOR
                    | Address::BURNT_FUNDS_ACTOR
                    | Address::SAFT_ACTOR
                    | Address::RESERVE_ACTOR
                    | Address::ETHEREUM_ACCOUNT_MANAGER_ACTOR => {
                        un_circ += actor_balance;
                    }
                    Address::MARKET_ACTOR => {
                        let ms = market::State::load(&db, actor.code, actor.state)?;

                        let locked_balance: TokenAmount = ms.total_locked().into();
                        circ += actor_balance - &locked_balance;
                        un_circ += locked_balance;
                    }
                    _ if is_account_actor(&actor.code)
                    || is_paych_actor(&actor.code)
                    || is_eth_account_actor(&actor.code)
                    || is_evm_actor(&actor.code)
                    || is_placeholder_actor(&actor.code) => {
                        circ += actor_balance;
                    },
                    _ if is_miner_actor(&actor.code) => {
                        let ms = miner::State::load(&db, actor.code, actor.state)?;

                        if let Ok(avail_balance) = ms.available_balance(actor.balance.atto()) {
                            let avail_balance = TokenAmount::from(avail_balance);
                            circ += avail_balance.clone();
                            un_circ += actor_balance.clone() - &avail_balance;
                        } else {
                            // Assume any error is because the miner state is "broken" (lower actor balance than locked funds)
                            // In this case, the actor's entire balance is considered "uncirculating"
                            un_circ += actor_balance;
                        }
                    }
                    _ if is_multisig_actor(&actor.code) => {
                        let ms = multisig::State::load(&db, actor.code, actor.state)?;

                        let locked_balance: TokenAmount = ms.locked_balance(height)?.into();
                        let avail_balance = actor_balance.clone() - &locked_balance;
                        circ += avail_balance.max(TokenAmount::zero());
                        un_circ += actor_balance.min(locked_balance);
                    }
                    _ => bail!("unexpected actor: {:?}", actor),
                }
            } else {
                // Do nothing for zero-balance actors
            }
            Ok(())
        })?;

        let total = circ.clone() + un_circ;
        if total != *TOTAL_FILECOIN {
            bail!(
                "total filecoin didn't add to expected amount: {} != {}",
                total,
                *TOTAL_FILECOIN
            );
        }

        Ok(circ)
    }
}

/// Vesting schedule info. These states are lazily filled, to avoid doing until
/// needed to calculate circulating supply.
#[derive(Default, Clone)]
struct GenesisInfoVesting {
    genesis: Vec<(ChainEpoch, TokenAmount)>,
    ignition: Vec<(ChainEpoch, ChainEpoch, TokenAmount)>,
    calico: Vec<(ChainEpoch, ChainEpoch, TokenAmount)>,
}

impl GenesisInfoVesting {
    fn new(liftoff_height: i64) -> Self {
        Self {
            genesis: setup_genesis_vesting_schedule(),
            ignition: setup_ignition_vesting_schedule(liftoff_height),
            calico: setup_calico_vesting_schedule(liftoff_height),
        }
    }
}

fn get_actor_state<DB: Blockstore>(
    state_tree: &StateTree<DB>,
    addr: &Address,
) -> Result<ActorState, anyhow::Error> {
    state_tree
        .get_actor(addr)?
        .with_context(|| format!("Failed to get Actor for address {addr}"))
}

fn get_fil_vested(genesis_info: &GenesisInfo, height: ChainEpoch) -> TokenAmount {
    let mut return_value = TokenAmount::default();

    let pre_ignition = &genesis_info.vesting.genesis;
    let post_ignition = &genesis_info.vesting.ignition;
    let calico_vesting = &genesis_info.vesting.calico;

    if height <= genesis_info.ignition_height {
        for (unlock_duration, initial_balance) in pre_ignition {
            return_value +=
                initial_balance - v0_amount_locked(*unlock_duration, initial_balance, height);
        }
    } else if height <= genesis_info.calico_height {
        for (start_epoch, unlock_duration, initial_balance) in post_ignition {
            return_value += initial_balance
                - v0_amount_locked(*unlock_duration, initial_balance, height - start_epoch);
        }
    } else {
        for (start_epoch, unlock_duration, initial_balance) in calico_vesting {
            return_value += initial_balance
                - v0_amount_locked(*unlock_duration, initial_balance, height - start_epoch);
        }
    }

    if height <= genesis_info.actors_v2_height {
        return_value += &genesis_info.genesis_pledge + &genesis_info.genesis_market_funds;
    }

    return_value
}

fn get_fil_mined<DB: Blockstore>(state_tree: &StateTree<DB>) -> Result<TokenAmount, anyhow::Error> {
    let actor = state_tree
        .get_actor(&Address::REWARD_ACTOR)?
        .context("Reward actor address could not be resolved")?;
    let state = reward::State::load(state_tree.store(), actor.code, actor.state)?;

    Ok(state.into_total_storage_power_reward().into())
}

fn get_fil_market_locked<DB: Blockstore>(
    state_tree: &StateTree<DB>,
) -> Result<TokenAmount, anyhow::Error> {
    let actor = state_tree
        .get_actor(&Address::MARKET_ACTOR)?
        .ok_or_else(|| Error::State("Market actor address could not be resolved".to_string()))?;
    let state = market::State::load(state_tree.store(), actor.code, actor.state)?;

    Ok(state.total_locked().into())
}

fn get_fil_power_locked<DB: Blockstore>(
    state_tree: &StateTree<DB>,
) -> Result<TokenAmount, anyhow::Error> {
    let actor = state_tree
        .get_actor(&Address::POWER_ACTOR)?
        .ok_or_else(|| Error::State("Power actor address could not be resolved".to_string()))?;
    let state = power::State::load(state_tree.store(), actor.code, actor.state)?;

    Ok(state.into_total_locked().into())
}

fn get_fil_reserve_disbursed<DB: Blockstore>(
    state_tree: &StateTree<DB>,
) -> Result<TokenAmount, anyhow::Error> {
    let fil_reserved: TokenAmount = TokenAmount::from_whole(300_000_000);
    let reserve_actor = get_actor_state(state_tree, &Address::RESERVE_ACTOR)?;

    // If money enters the reserve actor, this could lead to a negative term
    Ok(TokenAmount::from(&*fil_reserved - &reserve_actor.balance))
}

fn get_fil_locked<DB: Blockstore>(
    state_tree: &StateTree<DB>,
) -> Result<TokenAmount, anyhow::Error> {
    let market_locked = get_fil_market_locked(state_tree)?;
    let power_locked = get_fil_power_locked(state_tree)?;
    Ok(power_locked + market_locked)
}

fn get_fil_burnt<DB: Blockstore>(state_tree: &StateTree<DB>) -> Result<TokenAmount, anyhow::Error> {
    let burnt_actor = get_actor_state(state_tree, &Address::BURNT_FUNDS_ACTOR)?;

    Ok(TokenAmount::from(&burnt_actor.balance))
}

fn setup_genesis_vesting_schedule() -> Vec<(ChainEpoch, TokenAmount)> {
    PRE_CALICO_VESTING
        .into_iter()
        .map(|(unlock_duration, initial_balance)| {
            (unlock_duration, TokenAmount::from_atto(initial_balance))
        })
        .collect()
}

fn setup_ignition_vesting_schedule(
    liftoff_height: ChainEpoch,
) -> Vec<(ChainEpoch, ChainEpoch, TokenAmount)> {
    PRE_CALICO_VESTING
        .into_iter()
        .map(|(unlock_duration, initial_balance)| {
            (
                liftoff_height,
                unlock_duration,
                TokenAmount::from_whole(initial_balance),
            )
        })
        .collect()
}
fn setup_calico_vesting_schedule(
    liftoff_height: ChainEpoch,
) -> Vec<(ChainEpoch, ChainEpoch, TokenAmount)> {
    CALICO_VESTING
        .into_iter()
        .map(|(unlock_duration, initial_balance)| {
            (
                liftoff_height,
                unlock_duration,
                TokenAmount::from_whole(initial_balance),
            )
        })
        .collect()
}

// This exact code (bugs and all) has to be used. The results are locked into
// the blockchain.
/// Returns amount locked in multisig contract
fn v0_amount_locked(
    unlock_duration: ChainEpoch,
    initial_balance: &TokenAmount,
    elapsed_epoch: ChainEpoch,
) -> TokenAmount {
    if elapsed_epoch >= unlock_duration {
        return TokenAmount::zero();
    }
    if elapsed_epoch < 0 {
        return initial_balance.clone();
    }
    // Division truncation is broken here: https://github.com/filecoin-project/specs-actors/issues/1131
    let unit_locked: TokenAmount = initial_balance.div_floor(unlock_duration);
    unit_locked * (unlock_duration - elapsed_epoch)
}
