use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

use crate::IrohTransport;

#[derive(Debug, Clone)]
pub struct State {
    transports: Arc<Mutex<HashMap<u64, IrohTransport>>>,
}

impl State {
    pub fn new() -> Self {
        Self {
            transports: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn get_transport_by_transport_handle(&self, handle: u64) -> Option<IrohTransport> {
        self.transports.lock().await.get(&handle).cloned()
    }

    pub async fn add_transport(&self, transport: IrohTransport) -> anyhow::Result<()> {
        self.transports
            .lock()
            .await
            .insert(transport.get_handle().await.ok_or(anyhow::anyhow!("no transport handle"))?, transport);
        Ok(())
    }

    pub async fn remove_transport(&self, handle: u64) {
        self.transports.lock().await.remove(&handle);
    }

    pub async fn get_transport_by_node_handle(&self, handle: u64) -> Option<IrohTransport> {
        let values = self
            .transports
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for transport in values {
            if transport.get_node_by_node_handle(handle).await.is_some() {
                return Some(transport.clone());
            }
        }
        None
    }

    pub async fn get_transport_by_stream_handle(&self, handle: u64) -> Option<IrohTransport> {
        let values = self
            .transports
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for transport in values {
            if transport.get_stream_by_handle(handle).await.is_some() {
                return Some(transport.clone());
            }
        }
        None
    }
}
