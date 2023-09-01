// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::Context;
use async_compression::tokio::bufread::ZstdDecoder;
use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use tokio_util::compat::TokioAsyncReadCompatExt;

pub async fn load_actor_bundles(db: &impl Blockstore) -> anyhow::Result<Vec<Cid>> {
    const ERROR_MESSAGE: &str = "Actor bundles assets are not properly downloaded, make sure git-lfs is installed and run `git lfs pull` again. See <https://github.com/git-lfs/git-lfs/blob/main/INSTALLING.md>";

    const ACTOR_BUNDLES_CAR_ZST: &[u8] = include_bytes!("../../assets/actor_bundles.car.zst");

    fvm_ipld_car::load_car(
        db,
        ZstdDecoder::new(tokio::io::BufReader::new(ACTOR_BUNDLES_CAR_ZST)).compat(),
    )
    .await
    .context(ERROR_MESSAGE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::networks::ACTOR_BUNDLES;
    use ahash::HashSet;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_load_actor_bundles() {
        let db = fvm_ipld_blockstore::MemoryBlockstore::new();
        let roots = HashSet::from_iter(load_actor_bundles(&db).await.unwrap());
        let roots_expected: HashSet<Cid> = ACTOR_BUNDLES.iter().map(|b| b.manifest).collect();
        assert_eq!(roots, roots_expected);
    }
}
