// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::{io, marker::PhantomData, pin::Pin, task::Poll};

use bytes::BytesMut;
use futures::prelude::*;
use pin_project_lite::pin_project;
use tracing::warn;

pin_project! {
    #[derive(Debug)]
    pub(super) struct DagCborDecodingReader<B, T> {
        #[pin]
        io: B,
        max_bytes_allowed: usize,
        bytes: BytesMut,
        bytes_read: usize,
        _pd: PhantomData<T>,
    }
}

impl<B, T> DagCborDecodingReader<B, T> {
    /// `max_bytes_allowed == 0` means unlimited
    pub(super) fn new(io: B, max_bytes_allowed: usize) -> Self {
        Self {
            io,
            max_bytes_allowed,
            bytes: BytesMut::new(),
            bytes_read: 0,
            _pd: Default::default(),
        }
    }
}

impl<B, T> Future for DagCborDecodingReader<B, T>
where
    B: AsyncRead,
    T: serde::de::DeserializeOwned,
{
    type Output = io::Result<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        // https://github.com/mxinden/asynchronous-codec/blob/master/src/framed_read.rs#L161
        let mut buf = [0u8; 8 * 1024];
        loop {
            let this = self.as_mut().project();
            let n = std::task::ready!(this.io.poll_read(cx, &mut buf))?;
            // Terminated
            if n == 0 {
                let item =
                    serde_ipld_dagcbor::de::from_reader(&self.bytes[..]).map_err(io::Error::other);
                return Poll::Ready(item);
            }
            *this.bytes_read += n;
            if *this.max_bytes_allowed > 0 && *this.bytes_read > *this.max_bytes_allowed {
                let err = io::Error::other(format!(
                    "Buffer size exceeds the maximum allowed {}B",
                    *this.max_bytes_allowed,
                ));
                warn!("{err}");
                return Poll::Ready(Err(err));
            }
            this.bytes.extend_from_slice(&buf[..n.min(buf.len())]);
            // This is what `FramedRead` does internally
            // Assuming io will be re-used to send new messages.
            //
            // Note: `from_reader` ensures no trailing data left in `bytes`
            if let Ok(r) = serde_ipld_dagcbor::de::from_reader(&this.bytes[..]) {
                return Poll::Ready(Ok(r));
            }
        }
    }
}
