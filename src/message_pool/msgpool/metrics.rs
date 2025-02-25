// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use once_cell::sync::Lazy;
use prometheus::core::{AtomicU64, GenericGauge};

pub static MPOOL_MESSAGE_TOTAL: Lazy<Box<GenericGauge<AtomicU64>>> = Lazy::new(|| {
    let mpool_message_total = Box::new(
        GenericGauge::<AtomicU64>::new(
            "mpool_message_total",
            "Total number of messages in the message pool",
        )
        .expect("Defining the mpool_message_total metric must succeed"),
    );
    prometheus::default_registry()
        .register(mpool_message_total.clone())
        .expect(
            "Registering the mpool_message_total metric with the metrics registry must succeed",
        );
    mpool_message_total
});
