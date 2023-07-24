// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::sync::Arc;

use crate::libp2p_bitswap::{BitswapStoreRead, BitswapStoreReadWrite};
use ahash::HashMap;
use anyhow::Result;
use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use parking_lot::RwLock;

use super::SettingsStore;

#[derive(Debug, Default, Clone)]
pub struct MemoryDB {
    blockchain_db: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
    settings_db: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl SettingsStore for MemoryDB {
    fn read_bin<K>(&self, key: K) -> anyhow::Result<Option<Vec<u8>>>
    where
        K: AsRef<str>,
    {
        Ok(self.settings_db.read().get(key.as_ref()).cloned())
    }

    fn write_bin<K, V>(&self, key: K, value: V) -> anyhow::Result<()>
    where
        K: AsRef<str>,
        V: AsRef<[u8]>,
    {
        self.settings_db
            .write()
            .insert(key.as_ref().to_owned(), value.as_ref().to_vec());
        Ok(())
    }

    fn exists<K>(&self, key: K) -> anyhow::Result<bool>
    where
        K: AsRef<str>,
    {
        Ok(self.settings_db.read().contains_key(key.as_ref()))
    }
}

impl Blockstore for MemoryDB {
    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>> {
        Ok(self.blockchain_db.read().get(&k.to_bytes()).cloned())
    }

    fn put_keyed(&self, k: &Cid, block: &[u8]) -> Result<()> {
        self.blockchain_db
            .write()
            .insert(k.to_bytes(), block.to_vec());
        Ok(())
    }
}

impl BitswapStoreRead for MemoryDB {
    fn contains(&self, cid: &Cid) -> Result<bool> {
        Ok(self.blockchain_db.read().contains_key(&cid.to_bytes()))
    }

    fn get(&self, cid: &Cid) -> Result<Option<Vec<u8>>> {
        Blockstore::get(self, cid)
    }
}

impl BitswapStoreReadWrite for MemoryDB {
    type Params = libipld::DefaultParams;

    fn insert(&self, block: &libipld::Block<Self::Params>) -> Result<()> {
        self.put_keyed(block.cid(), block.data())
    }
}
