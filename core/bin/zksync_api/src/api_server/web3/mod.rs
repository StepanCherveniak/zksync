// Built-in uses
// External uses
use futures::channel::mpsc;
use jsonrpc_core::{Error, IoHandler, MetaIoHandler, Metadata, Middleware, Result};
use jsonrpc_http_server::ServerBuilder;
// Workspace uses

use zksync_storage::{ConnectionPool, StorageProcessor};
use zksync_utils::panic_notify::ThreadPanicNotify;
// Local uses
use self::{calls::CallsHelper, logs::LogsHelper, rpc_trait::Web3Rpc};
use zksync_config::configs::api::Web3Config;

mod calls;
mod converter;
mod logs;
mod rpc_impl;
mod rpc_trait;
#[cfg(test)]
mod tests;
mod types;

pub const ZKSYNC_PROXY_ADDRESS: &str = "1000000000000000000000000000000000000000";
pub const NFT_FACTORY_ADDRESS: &str = "2000000000000000000000000000000000000000";

#[derive(Clone)]
pub struct Web3RpcApp {
    runtime_handle: tokio::runtime::Handle,
    connection_pool: ConnectionPool,
    logs_helper: LogsHelper,
    calls_helper: CallsHelper,
    max_block_range: u32,
}

impl Web3RpcApp {
    pub fn new(connection_pool: ConnectionPool, max_block_range: u32) -> Self {
        let runtime_handle = tokio::runtime::Handle::try_current()
            .expect("Web3RpcApp must be created from the context of Tokio Runtime");
        Web3RpcApp {
            runtime_handle,
            connection_pool,
            logs_helper: LogsHelper::new(),
            calls_helper: CallsHelper::new(),
            max_block_range,
        }
    }

    pub fn extend<T: Metadata, S: Middleware<T>>(self, io: &mut MetaIoHandler<T, S>) {
        io.extend_with(self.to_delegate())
    }

    async fn access_storage(&self) -> Result<StorageProcessor<'_>> {
        self.connection_pool
            .access_storage()
            .await
            .map_err(|_| Error::internal_error())
    }
}

pub fn start_rpc_server(
    connection_pool: ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
    web3_config: &Web3Config,
) {
    let addr = web3_config.bind_addr();

    let rpc_app = Web3RpcApp::new(connection_pool, web3_config.max_block_range);
    std::thread::spawn(move || {
        let _panic_sentinel = ThreadPanicNotify(panic_notify);
        let mut io = IoHandler::new();
        rpc_app.extend(&mut io);

        let server = ServerBuilder::new(io)
            .threads(super::THREADS_PER_SERVER)
            .start_http(&addr)
            .unwrap();
        server.wait();
    });
}
