// Copyright 2019-2023 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use crate::rpc_api::data_types::ApiMessage;
use crate::shim::message::Message;
use crate::{
    blocks::{BlockHeader, Tipset, TipsetKeys},
    rpc_api::chain_api::*,
    rpc_api::data_types::BlockMessages,
    shim::clock::ChainEpoch,
};
use cid::Cid;

use super::{ApiInfo, JsonRpcError, RpcRequest};

impl ApiInfo {
    pub async fn chain_head(&self) -> Result<Tipset, JsonRpcError> {
        self.call(Self::chain_head_req()).await
    }

    pub fn chain_head_req() -> RpcRequest<Tipset> {
        RpcRequest::new(CHAIN_HEAD, ())
    }

    pub async fn chain_get_block(&self, cid: Cid) -> Result<BlockHeader, JsonRpcError> {
        self.call(Self::chain_get_block_req(cid)).await
    }

    pub fn chain_get_block_req(cid: Cid) -> RpcRequest<BlockHeader> {
        RpcRequest::new(CHAIN_GET_BLOCK, (cid,))
    }

    pub fn chain_get_block_messages_req(cid: Cid) -> RpcRequest<BlockMessages> {
        RpcRequest::new(CHAIN_GET_BLOCK_MESSAGES, (cid,))
    }

    /// Get tipset at epoch. Pick younger tipset if epoch points to a
    /// null-tipset. Only tipsets below the given `head` are searched. If `head`
    /// is null, the node will use the heaviest tipset.
    pub async fn chain_get_tipset_by_height(
        &self,
        epoch: ChainEpoch,
        head: TipsetKeys,
    ) -> Result<Tipset, JsonRpcError> {
        self.call(Self::chain_get_tipset_by_height_req(epoch, head))
            .await
    }

    pub fn chain_get_tipset_by_height_req(
        epoch: ChainEpoch,
        head: TipsetKeys,
    ) -> RpcRequest<Tipset> {
        RpcRequest::new(CHAIN_GET_TIPSET_BY_HEIGHT, (epoch, head))
    }

    pub fn chain_get_tipset_req(tsk: TipsetKeys) -> RpcRequest<Tipset> {
        RpcRequest::new(CHAIN_GET_TIPSET, (tsk,))
    }

    pub async fn chain_get_genesis(&self) -> Result<Option<Tipset>, JsonRpcError> {
        self.call(Self::chain_get_genesis_req()).await
    }

    pub fn chain_get_genesis_req() -> RpcRequest<Option<Tipset>> {
        RpcRequest::new(CHAIN_GET_GENESIS, ())
    }

    pub async fn chain_set_head(&self, new_head: TipsetKeys) -> Result<(), JsonRpcError> {
        self.call(Self::chain_set_head_req(new_head)).await
    }

    pub fn chain_set_head_req(new_head: TipsetKeys) -> RpcRequest<()> {
        RpcRequest::new(CHAIN_SET_HEAD, (new_head,))
    }

    pub async fn chain_export(
        &self,
        params: ChainExportParams,
    ) -> Result<ChainExportResult, JsonRpcError> {
        self.call(Self::chain_export_req(params)).await
    }

    pub fn chain_export_req(params: ChainExportParams) -> RpcRequest<ChainExportResult> {
        RpcRequest::new(CHAIN_EXPORT, params)
    }

    #[allow(dead_code)]
    pub async fn chain_get_message(&self, cid: Cid) -> Result<Message, JsonRpcError> {
        self.call(Self::chain_get_message_req(cid)).await
    }

    pub fn chain_get_message_req(cid: Cid) -> RpcRequest<Message> {
        RpcRequest::new(CHAIN_GET_MESSAGE, (cid,))
    }

    pub async fn chain_read_obj(&self, cid: Cid) -> Result<Vec<u8>, JsonRpcError> {
        self.call(Self::chain_read_obj_req(cid)).await
    }

    pub fn chain_read_obj_req(cid: Cid) -> RpcRequest<Vec<u8>> {
        RpcRequest::new(CHAIN_READ_OBJ, (cid,))
    }

    pub fn chain_has_obj_req(cid: Cid) -> RpcRequest<bool> {
        RpcRequest::new(CHAIN_HAS_OBJ, (cid,))
    }

    pub async fn chain_get_min_base_fee(
        &self,
        basefee_lookback: u32,
    ) -> Result<String, JsonRpcError> {
        self.call(Self::chain_get_min_base_fee_req(basefee_lookback))
            .await
    }

    pub fn chain_get_min_base_fee_req(basefee_lookback: u32) -> RpcRequest<String> {
        RpcRequest::new(CHAIN_GET_MIN_BASE_FEE, (basefee_lookback,))
    }

    pub fn chain_get_messages_in_tipset_req(tsk: TipsetKeys) -> RpcRequest<Vec<ApiMessage>> {
        RpcRequest::new(CHAIN_GET_MESSAGES_IN_TIPSET, (tsk,))
    }

    pub fn chain_get_parent_messages_req(block_cid: Cid) -> RpcRequest<Vec<ApiMessage>> {
        RpcRequest::new(CHAIN_GET_PARENT_MESSAGES, (block_cid,))
    }
}
