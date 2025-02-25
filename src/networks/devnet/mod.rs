// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use cid::Cid;
use once_cell::sync::Lazy;

use super::{drand::DRAND_MAINNET, DrandPoint, Height, HeightInfo};

// https://github.com/ethereum-lists/chains/blob/6b1e3ccad1cfcaae5aa1ab917960258f0ef1a6b6/_data/chains/eip155-31415926.json
pub const ETH_CHAIN_ID: u64 = 31415926;

/// Height epochs.
pub static HEIGHT_INFOS: Lazy<[HeightInfo; 22]> = Lazy::new(|| {
    [
        HeightInfo {
            height: Height::Breeze,
            epoch: -50,
            bundle: None,
        },
        HeightInfo {
            height: Height::Smoke,
            epoch: -2,
            bundle: None,
        },
        HeightInfo {
            height: Height::Ignition,
            epoch: -3,
            bundle: None,
        },
        HeightInfo {
            height: Height::ActorsV2,
            epoch: -3,
            bundle: None,
        },
        HeightInfo {
            height: Height::Tape,
            epoch: -4,
            bundle: None,
        },
        HeightInfo {
            height: Height::Liftoff,
            epoch: -6,
            bundle: None,
        },
        HeightInfo {
            height: Height::Kumquat,
            epoch: -7,
            bundle: None,
        },
        HeightInfo {
            height: Height::Calico,
            epoch: -9,
            bundle: None,
        },
        HeightInfo {
            height: Height::Persian,
            epoch: -10,
            bundle: None,
        },
        HeightInfo {
            height: Height::Orange,
            epoch: -11,
            bundle: None,
        },
        HeightInfo {
            height: Height::Trust,
            epoch: -13,
            bundle: None,
        },
        HeightInfo {
            height: Height::Norwegian,
            epoch: -14,
            bundle: None,
        },
        HeightInfo {
            height: Height::Turbo,
            epoch: -15,
            bundle: None,
        },
        HeightInfo {
            height: Height::Hyperdrive,
            epoch: -16,
            bundle: None,
        },
        HeightInfo {
            height: Height::Chocolate,
            epoch: -17,
            bundle: None,
        },
        HeightInfo {
            height: Height::OhSnap,
            epoch: -18,
            bundle: None,
        },
        HeightInfo {
            height: Height::Skyr,
            epoch: -19,
            bundle: None,
        },
        HeightInfo {
            height: Height::Shark,
            epoch: -20,
            bundle: Some(
                Cid::try_from("bafy2bzacedozk3jh2j4nobqotkbofodq4chbrabioxbfrygpldgoxs3zwgggk")
                    .unwrap(),
            ),
        },
        HeightInfo {
            height: Height::Hygge,
            epoch: -21,
            bundle: Some(
                Cid::try_from("bafy2bzacebzz376j5kizfck56366kdz5aut6ktqrvqbi3efa2d4l2o2m653ts")
                    .unwrap(),
            ),
        },
        HeightInfo {
            height: Height::Lightning,
            epoch: -22,
            bundle: Some(
                Cid::try_from("bafy2bzaceay35go4xbjb45km6o46e5bib3bi46panhovcbedrynzwmm3drr4i")
                    .unwrap(),
            ),
        },
        HeightInfo {
            height: Height::Thunder,
            epoch: -1,
            bundle: None,
        },
        HeightInfo {
            height: Height::Watermelon,
            epoch: 10,
            bundle: Some(
                Cid::try_from("bafy2bzaceasjdukhhyjbegpli247vbf5h64f7uvxhhebdihuqsj2mwisdwa6o")
                    .unwrap(),
            ),
        },
    ]
});

pub(super) static DRAND_SCHEDULE: [DrandPoint<'static>; 1] = [DrandPoint {
    height: 0,
    config: &DRAND_MAINNET,
}];
