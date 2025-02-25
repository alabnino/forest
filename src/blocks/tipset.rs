// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::{fmt, sync::OnceLock};

use crate::cid_collections::FrozenCidVec;
use crate::db::{SettingsStore, SettingsStoreExt};
use crate::networks::{calibnet, mainnet};
use crate::shim::{address::Address, clock::ChainEpoch};
use crate::utils::cid::CidCborExt;
use ahash::{HashMap, HashSet, HashSetExt};
use anyhow::Context as _;
use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::CborStore;
use num::BigInt;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tracing::info;

use super::{Block, BlockHeader, Error, Ticket};

/// A set of `CIDs` forming a unique key for a Tipset.
/// Equal keys will have equivalent iteration order, but note that the `CIDs`
/// are *not* maintained in the same order as the canonical iteration order of
/// blocks in a tipset (which is by ticket)
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(derive_quickcheck_arbitrary::Arbitrary))]
#[serde(transparent)]
pub struct TipsetKeys {
    pub cids: FrozenCidVec,
}

impl TipsetKeys {
    // Special encoding to match Lotus.
    pub fn cid(&self) -> anyhow::Result<Cid> {
        use fvm_ipld_encoding::RawBytes;
        let mut bytes = Vec::new();
        for cid in self.cids.clone() {
            bytes.append(&mut cid.to_bytes())
        }
        Ok(Cid::from_cbor_blake2b256(&RawBytes::new(bytes))?)
    }
}

impl FromIterator<Cid> for TipsetKeys {
    fn from_iter<T: IntoIterator<Item = Cid>>(iter: T) -> Self {
        Self {
            cids: iter.into_iter().collect(),
        }
    }
}

impl fmt::Display for TipsetKeys {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = self
            .cids
            .clone()
            .into_iter()
            .map(|cid| cid.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        write!(f, "[{}]", s)
    }
}

/// An immutable set of blocks at the same height with the same parent set.
/// Blocks in a tipset are canonically ordered by ticket size.
#[derive(Clone, Debug)]
pub struct Tipset {
    headers: Vec<BlockHeader>,
    key: OnceCell<TipsetKeys>,
}

impl From<&BlockHeader> for Tipset {
    fn from(value: &BlockHeader) -> Self {
        Self {
            headers: vec![value.clone()],
            key: OnceCell::new(),
        }
    }
}

impl From<BlockHeader> for Tipset {
    fn from(value: BlockHeader) -> Self {
        Self {
            headers: vec![value],
            key: OnceCell::new(),
        }
    }
}

impl PartialEq for Tipset {
    fn eq(&self, other: &Self) -> bool {
        self.headers.eq(&other.headers)
    }
}

#[cfg(test)]
impl quickcheck::Arbitrary for Tipset {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        // TODO(forest): https://github.com/ChainSafe/forest/issues/3570
        //               Support random generation of tipsets with multiple blocks.
        Tipset::from(BlockHeader::arbitrary(g))
    }
}

impl From<FullTipset> for Tipset {
    fn from(full_tipset: FullTipset) -> Self {
        let key = full_tipset.key;
        let headers: Vec<BlockHeader> = full_tipset
            .blocks
            .into_iter()
            .map(|block| block.header)
            .collect();

        Tipset { headers, key }
    }
}

#[allow(clippy::len_without_is_empty)]
impl Tipset {
    /// Builds a new Tipset from a collection of blocks.
    /// A valid tipset contains a non-empty collection of blocks that have
    /// distinct miners and all specify identical epoch, parents, weight,
    /// height, state root, receipt root; content-id for headers are
    /// supposed to be distinct but until encoding is added will be equal.
    pub fn new(mut headers: Vec<BlockHeader>) -> Result<Self, Error> {
        verify_blocks(&headers)?;

        // sort headers by ticket size
        // break ticket ties with the header CIDs, which are distinct
        headers.sort_by_cached_key(|h| h.to_sort_key());

        // return tipset where sorted headers have smallest ticket size in the 0th index
        // and the distinct keys
        Ok(Self {
            headers,
            key: OnceCell::new(),
        })
    }

    /// Fetch a tipset from the blockstore. This call fails if the tipset is
    /// present but invalid. If the tipset is missing, None is returned.
    pub fn load(store: impl Blockstore, tsk: &TipsetKeys) -> anyhow::Result<Option<Tipset>> {
        Ok(tsk
            .cids
            .clone()
            .into_iter()
            .map(|key| BlockHeader::load(&store, key))
            .collect::<anyhow::Result<Option<_>>>()?
            .map(Tipset::new)
            .transpose()?)
    }

    /// Load the heaviest tipset from the blockstore
    pub fn load_heaviest(
        store: &impl Blockstore,
        settings: &impl SettingsStore,
    ) -> anyhow::Result<Option<Tipset>> {
        Ok(
            match settings.read_obj::<TipsetKeys>(crate::db::setting_keys::HEAD_KEY)? {
                Some(tsk) => tsk
                    .cids
                    .into_iter()
                    .map(|key| BlockHeader::load(store, key))
                    .collect::<anyhow::Result<Option<_>>>()?
                    .map(Tipset::new)
                    .transpose()?,
                None => None,
            },
        )
    }

    /// Fetch a tipset from the blockstore. This calls fails if the tipset is
    /// missing or invalid.
    pub fn load_required(store: impl Blockstore, tsk: &TipsetKeys) -> anyhow::Result<Tipset> {
        Tipset::load(store, tsk)?.context("Required tipset missing from database")
    }

    /// Constructs and returns a full tipset if messages from storage exists
    pub fn fill_from_blockstore(&self, store: impl Blockstore) -> Option<FullTipset> {
        // Find tipset messages. If any are missing, return `None`.
        let blocks = self
            .blocks()
            .iter()
            .cloned()
            .map(|header| {
                let (bls_messages, secp_messages) =
                    crate::chain::store::block_messages(&store, &header).ok()?;
                Some(Block {
                    header,
                    bls_messages,
                    secp_messages,
                })
            })
            .collect::<Option<Vec<_>>>()?;

        // the given tipset has already been verified, so this cannot fail
        Some(
            FullTipset::new(blocks)
                .expect("block headers have already been verified so this check cannot fail"),
        )
    }

    /// Returns epoch of the tipset.
    pub fn epoch(&self) -> ChainEpoch {
        self.min_ticket_block().epoch()
    }
    /// Returns all blocks in tipset.
    pub fn blocks(&self) -> &[BlockHeader] {
        &self.headers
    }
    /// Consumes tipset to convert into a vector of [`BlockHeader`].
    pub fn into_blocks(self) -> Vec<BlockHeader> {
        self.headers
    }
    /// Returns the smallest ticket of all blocks in the tipset
    pub fn min_ticket(&self) -> Option<&Ticket> {
        self.min_ticket_block().ticket().as_ref()
    }
    /// Returns the block with the smallest ticket of all blocks in the tipset
    pub fn min_ticket_block(&self) -> &BlockHeader {
        // `Tipset::new` guarantees that `blocks` isn't empty
        self.headers.first().unwrap()
    }
    /// Returns the smallest timestamp of all blocks in the tipset
    pub fn min_timestamp(&self) -> u64 {
        self.headers
            .iter()
            .map(|block| block.timestamp())
            .min()
            .unwrap()
    }
    /// Returns the number of blocks in the tipset.
    pub fn len(&self) -> usize {
        self.headers.len()
    }
    /// Returns a key for the tipset.
    pub fn key(&self) -> &TipsetKeys {
        self.key.get_or_init(|| {
            TipsetKeys::from_iter(self.headers.iter().map(BlockHeader::cid).copied())
        })
    }
    /// Returns slice of `CIDs` for the current tipset
    pub fn cids(&self) -> Vec<Cid> {
        self.key().cids.clone().into_iter().collect()
    }
    /// Returns the keys of the parents of the blocks in the tipset.
    pub fn parents(&self) -> &TipsetKeys {
        self.min_ticket_block().parents()
    }
    /// Returns the state root for the tipset parent.
    pub fn parent_state(&self) -> &Cid {
        self.min_ticket_block().state_root()
    }
    /// Returns the tipset's calculated weight
    pub fn weight(&self) -> &BigInt {
        self.min_ticket_block().weight()
    }
    /// Returns true if self wins according to the Filecoin tie-break rule
    /// (FIP-0023)
    pub fn break_weight_tie(&self, other: &Tipset) -> bool {
        // blocks are already sorted by ticket
        let broken = self
            .blocks()
            .iter()
            .zip(other.blocks().iter())
            .any(|(a, b)| {
                const MSG: &str =
                    "The function block_sanity_checks should have been called at this point.";
                let ticket = a.ticket().as_ref().expect(MSG);
                let other_ticket = b.ticket().as_ref().expect(MSG);
                ticket.vrfproof < other_ticket.vrfproof
            });
        if broken {
            info!("Weight tie broken in favour of {}", self.key());
        } else {
            info!("Weight tie left unbroken, default to {}", other.key());
        }
        broken
    }
    /// Returns an iterator of all tipsets
    pub fn chain(self, store: impl Blockstore) -> impl Iterator<Item = Tipset> {
        itertools::unfold(Some(self), move |tipset| {
            tipset.take().map(|child| {
                *tipset = Tipset::load(&store, child.parents()).ok().flatten();
                child
            })
        })
    }

    /// Fetch the genesis block header for a given tipset.
    pub fn genesis(&self, store: impl Blockstore) -> anyhow::Result<BlockHeader> {
        // Scanning through millions of epochs to find the genesis is quite
        // slow. Let's use a list of known blocks to short-circuit the search.
        // The blocks are hash-chained together and known blocks are guaranteed
        // to have a known genesis.
        #[derive(Serialize, Deserialize)]
        struct KnownHeaders {
            calibnet: HashMap<ChainEpoch, String>,
            mainnet: HashMap<ChainEpoch, String>,
        }

        static KNOWN_HEADERS: OnceLock<KnownHeaders> = OnceLock::new();
        let headers = KNOWN_HEADERS.get_or_init(|| {
            serde_yaml::from_str(include_str!("../../build/known_blocks.yaml")).unwrap()
        });

        for tipset in self.clone().chain(&store) {
            // Search for known calibnet and mainnet blocks
            for (genesis_cid, known_blocks) in [
                (*calibnet::GENESIS_CID, &headers.calibnet),
                (*mainnet::GENESIS_CID, &headers.mainnet),
            ] {
                if let Some(known_block_cid) = known_blocks.get(&tipset.epoch()) {
                    if known_block_cid == &tipset.min_ticket_block().cid().to_string() {
                        return store
                            .get_cbor(&genesis_cid)?
                            .context("Genesis block missing from database");
                    }
                }
            }

            // If no known blocks are found, we'll eventually hit the genesis tipset.
            if tipset.epoch() == 0 {
                return Ok(tipset.min_ticket_block().clone());
            }
        }
        anyhow::bail!("Genesis block not found")
    }
}

/// `FullTipset` is an expanded version of a tipset that contains all the blocks
/// and messages
#[derive(Debug, Clone)]
pub struct FullTipset {
    blocks: Vec<Block>,
    key: OnceCell<TipsetKeys>,
}

// Constructing a FullTipset from a single Block is infallible.
impl From<Block> for FullTipset {
    fn from(block: Block) -> Self {
        FullTipset {
            blocks: vec![block],
            key: OnceCell::new(),
        }
    }
}

impl PartialEq for FullTipset {
    fn eq(&self, other: &Self) -> bool {
        self.blocks.eq(&other.blocks)
    }
}

impl FullTipset {
    pub fn new(mut blocks: Vec<Block>) -> Result<Self, Error> {
        verify_blocks(blocks.iter().map(Block::header))?;

        // sort blocks on creation to allow for more seamless conversions between
        // FullTipset and Tipset
        blocks.sort_by_cached_key(|block| block.header().to_sort_key());
        Ok(Self {
            blocks,
            key: OnceCell::new(),
        })
    }
    /// Returns the first block of the tipset.
    fn first_block(&self) -> &Block {
        // `FullTipset::new` guarantees that `blocks` isn't empty
        self.blocks.first().unwrap()
    }
    /// Returns reference to all blocks in a full tipset.
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }
    /// Returns all blocks in a full tipset.
    pub fn into_blocks(self) -> Vec<Block> {
        self.blocks
    }
    /// Converts the full tipset into a [Tipset] which removes the messages
    /// attached.
    pub fn into_tipset(self) -> Tipset {
        Tipset::from(self)
    }
    /// Returns a key for the tipset.
    pub fn key(&self) -> &TipsetKeys {
        self.key
            .get_or_init(|| TipsetKeys::from_iter(self.blocks.iter().map(Block::cid).copied()))
    }
    /// Returns the state root for the tipset parent.
    pub fn parent_state(&self) -> &Cid {
        self.first_block().header().state_root()
    }
    /// Returns epoch of the tipset.
    pub fn epoch(&self) -> ChainEpoch {
        self.first_block().header().epoch()
    }
    /// Returns the tipset's calculated weight.
    pub fn weight(&self) -> &BigInt {
        self.first_block().header().weight()
    }
}

fn verify_blocks<'a, I>(headers: I) -> Result<(), Error>
where
    I: IntoIterator<Item = &'a BlockHeader>,
{
    let mut headers = headers.into_iter();
    let first_header = headers.next().ok_or(Error::NoBlocks)?;

    let verify = |predicate: bool, message: &'static str| {
        if predicate {
            Ok(())
        } else {
            Err(Error::InvalidTipset(message.to_string()))
        }
    };

    let mut headers_set: HashSet<Address> = HashSet::new();
    headers_set.insert(*first_header.miner_address());

    for header in headers {
        verify(
            header.parents() == first_header.parents(),
            "parent cids are not equal",
        )?;
        verify(
            header.state_root() == first_header.state_root(),
            "state_roots are not equal",
        )?;
        verify(
            header.epoch() == first_header.epoch(),
            "epochs are not equal",
        )?;

        verify(
            headers_set.insert(*header.miner_address()),
            "miner_addresses are not distinct",
        )?;
    }

    Ok(())
}

pub mod lotus_json {
    //! [Tipset] isn't just plain old data - it has an invariant (all [`BlockHeader`]s are valid)
    //! So there is custom de-serialization here

    use crate::blocks::{BlockHeader, Tipset};
    use crate::lotus_json::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::TipsetKeys;

    pub struct TipsetLotusJson(Tipset);

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct TipsetLotusJsonInner {
        cids: LotusJson<TipsetKeys>,
        blocks: LotusJson<Vec<BlockHeader>>,
        height: LotusJson<i64>,
    }

    impl<'de> Deserialize<'de> for TipsetLotusJson {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let TipsetLotusJsonInner {
                cids: _ignored0,
                blocks,
                height: _ignored1,
            } = Deserialize::deserialize(deserializer)?;
            Tipset::new(blocks.into_inner())
                .map_err(serde::de::Error::custom)
                .map(Self)
        }
    }

    impl Serialize for TipsetLotusJson {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let Self(tipset) = self;
            TipsetLotusJsonInner {
                cids: tipset.key().clone().into(),
                blocks: tipset.clone().into_blocks().into(),
                height: tipset.epoch().into(),
            }
            .serialize(serializer)
        }
    }

    impl HasLotusJson for Tipset {
        type LotusJson = TipsetLotusJson;

        fn snapshots() -> Vec<(serde_json::Value, Self)> {
            use serde_json::json;
            vec![(
                json!({
                    "Blocks": [
                        {
                            "BeaconEntries": null,
                            "ForkSignaling": 0,
                            "Height": 0,
                            "Messages": { "/": "baeaaaaa" },
                            "Miner": "f00",
                            "ParentBaseFee": "0",
                            "ParentMessageReceipts": { "/": "baeaaaaa" },
                            "ParentStateRoot": { "/":"baeaaaaa" },
                            "ParentWeight": "0",
                            "Parents": null,
                            "Timestamp": 0,
                            "WinPoStProof": null
                        }
                    ],
                    "Cids": [
                        { "/": "bafy2bzacean6ik6kxe6i6nv5of3ocoq4czioo556fxifhunwue2q7kqmn6zqc" }
                    ],
                    "Height": 0
                }),
                Self::new(vec![BlockHeader::default()]).unwrap(),
            )]
        }

        fn into_lotus_json(self) -> Self::LotusJson {
            TipsetLotusJson(self)
        }

        fn from_lotus_json(TipsetLotusJson(tipset): Self::LotusJson) -> Self {
            tipset
        }
    }

    #[test]
    fn snapshots() {
        assert_all_snapshots::<Tipset>()
    }

    #[cfg(test)]
    quickcheck::quickcheck! {
        fn quickcheck(val: Tipset) -> () {
            assert_unchanged_via_json(val)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::blocks::VRFProof;
    use crate::shim::address::Address;
    use cid::{
        multihash::{Code::Identity, MultihashDigest},
        Cid,
    };
    use fvm_ipld_encoding::DAG_CBOR;
    use num_bigint::BigInt;

    use crate::blocks::{BlockHeader, ElectionProof, Error, Ticket, Tipset, TipsetKeys};

    pub fn mock_block(id: u64, weight: u64, ticket_sequence: u64) -> BlockHeader {
        let addr = Address::new_id(id);
        let cid =
            Cid::try_from("bafyreicmaj5hhoy5mgqvamfhgexxyergw7hdeshizghodwkjg6qmpoco7i").unwrap();

        let fmt_str = format!("===={ticket_sequence}=====");
        let ticket = Ticket::new(VRFProof::new(fmt_str.clone().into_bytes()));
        let election_proof = ElectionProof {
            win_count: 0,
            vrfproof: VRFProof::new(fmt_str.into_bytes()),
        };
        let weight_inc = BigInt::from(weight);
        BlockHeader::builder()
            .miner_address(addr)
            .election_proof(Some(election_proof))
            .ticket(Some(ticket))
            .message_receipts(cid)
            .messages(cid)
            .state_root(cid)
            .weight(weight_inc)
            .build()
            .unwrap()
    }

    #[test]
    fn test_break_weight_tie() {
        let b1 = mock_block(1234561, 1, 1);
        let ts1 = Tipset::from(&b1);

        let b2 = mock_block(1234562, 1, 2);
        let ts2 = Tipset::from(&b2);

        let b3 = mock_block(1234563, 1, 1);
        let ts3 = Tipset::from(&b3);

        // All tipsets have the same weight (but it's not really important here)

        // Can break weight tie
        assert!(ts1.break_weight_tie(&ts2));
        // Can not break weight tie (because of same min tickets)
        assert!(!ts1.break_weight_tie(&ts3));

        // Values are chosen so that Ticket(b4) < Ticket(b5) < Ticket(b1)
        let b4 = mock_block(1234564, 1, 41);
        let b5 = mock_block(1234565, 1, 45);
        let ts4 = Tipset::new(vec![b4.clone(), b5.clone(), b1.clone()]).unwrap();
        let ts5 = Tipset::new(vec![b4.clone(), b5.clone(), b2]).unwrap();
        // Can break weight tie with several min tickets the same
        assert!(ts4.break_weight_tie(&ts5));

        let ts6 = Tipset::new(vec![b4.clone(), b5.clone(), b1.clone()]).unwrap();
        let ts7 = Tipset::new(vec![b4, b5, b1]).unwrap();
        // Can not break weight tie with all min tickets the same
        assert!(!ts6.break_weight_tie(&ts7));
    }

    #[test]
    fn ensure_miner_addresses_are_distinct() {
        let h0 = BlockHeader::builder()
            .miner_address(Address::new_id(0))
            .build()
            .unwrap();
        let h1 = BlockHeader::builder()
            .miner_address(Address::new_id(0))
            .build()
            .unwrap();
        assert_eq!(
            Tipset::new(vec![h0, h1]).unwrap_err(),
            Error::InvalidTipset("miner_addresses are not distinct".to_string())
        );
    }

    // specifically test the case when we are distinct from miner_address 0, but not
    // 1
    #[test]
    fn ensure_multiple_miner_addresses_are_distinct() {
        let h0 = BlockHeader::builder()
            .miner_address(Address::new_id(1))
            .build()
            .unwrap();
        let h1 = BlockHeader::builder()
            .miner_address(Address::new_id(0))
            .build()
            .unwrap();
        let h2 = BlockHeader::builder()
            .miner_address(Address::new_id(0))
            .build()
            .unwrap();
        assert_eq!(
            Tipset::new(vec![h0, h1, h2]).unwrap_err(),
            Error::InvalidTipset("miner_addresses are not distinct".to_string())
        );
    }

    #[test]
    fn ensure_epochs_are_equal() {
        let h0 = BlockHeader::builder()
            .miner_address(Address::new_id(0))
            .epoch(1)
            .build()
            .unwrap();
        let h1 = BlockHeader::builder()
            .miner_address(Address::new_id(1))
            .epoch(2)
            .build()
            .unwrap();
        assert_eq!(
            Tipset::new(vec![h0, h1]).unwrap_err(),
            Error::InvalidTipset("epochs are not equal".to_string())
        );
    }

    #[test]
    fn ensure_state_roots_are_equal() {
        let h0 = BlockHeader::builder()
            .miner_address(Address::new_id(0))
            .state_root(Cid::new_v1(DAG_CBOR, Identity.digest(&[])))
            .build()
            .unwrap();
        let h1 = BlockHeader::builder()
            .miner_address(Address::new_id(1))
            .state_root(Cid::new_v1(DAG_CBOR, Identity.digest(&[1])))
            .build()
            .unwrap();
        assert_eq!(
            Tipset::new(vec![h0, h1]).unwrap_err(),
            Error::InvalidTipset("state_roots are not equal".to_string())
        );
    }

    #[test]
    fn ensure_parent_cids_are_equal() {
        let h0 = BlockHeader::builder()
            .miner_address(Address::new_id(0))
            .parents(TipsetKeys::default())
            .build()
            .unwrap();
        let h1 = BlockHeader::builder()
            .miner_address(Address::new_id(1))
            .parents(TipsetKeys::from_iter([Cid::new_v1(
                DAG_CBOR,
                Identity.digest(&[]),
            )]))
            .build()
            .unwrap();
        assert_eq!(
            Tipset::new(vec![h0, h1]).unwrap_err(),
            Error::InvalidTipset("parent cids are not equal".to_string())
        );
    }

    #[test]
    fn ensure_there_are_blocks() {
        assert_eq!(Tipset::new(vec![]).unwrap_err(), Error::NoBlocks);
    }
}
