use iroh::NodeId;
use std::collections::HashMap;
use tracing::warn;

use crate::{
    actor::{Action, Actor, Handle},
    node::IrohNode,
};

#[derive(Debug, Clone)]
pub struct IrohTransport {
    api: Handle<IrohTransportActor>,
}

#[derive(Debug)]
struct IrohTransportActor {
    rx: tokio::sync::mpsc::Receiver<Action<IrohTransportActor>>,
    handle: u64,
    nodes: HashMap<u64, IrohNode>,
}

impl IrohTransport {
    pub fn new(handle: u64) -> anyhow::Result<Self> {
        let (api, rx) = Handle::channel(1024);
        crate::runtime_handle()?.spawn(async move {
            let mut actor = IrohTransportActor {
                rx,
                handle,
                nodes: HashMap::new(),
            };
            if let Err(e) = actor.run().await {
                warn!("IrohNodeActor exited with error: {:?}", e);
            }
        });
        Ok(Self { api })
    }

    pub async fn get_handle(&self) -> Option<u64> {
        self.api
            .call(move |actor| Box::pin(actor.get_handle()))
            .await
            .ok()
    }

    pub async fn get_node_by_handle(&self, handle: u64) -> Option<IrohNode> {
        self.api
            .call(move |actor| {
                Box::pin(async move {
                    actor
                        .nodes
                        .get(&handle)
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("no node found for given handle"))
                })
            })
            .await
            .ok()
    }

    pub async fn get_node_by_listener_handle(&self, handle: u64) -> Option<IrohNode> {
        self.api
            .call(move |actor| {
                Box::pin(async move {
                    actor
                        .nodes
                        .get(&handle)
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("no node found for given handle"))
                })
            })
            .await
            .ok()
    }

    pub async fn get_stream_by_handle(&self, handle: u64) -> Option<crate::stream::IrohStream> {
        self.api
            .call(move |actor| {
                Box::pin(async move {
                    actor
                        .get_stream_by_handle(handle)
                        .await
                        .ok_or_else(|| anyhow::anyhow!("no stream found for given handle"))
                })
            })
            .await
            .ok()
    }

    pub async fn get_node_by_stream_handle(&self, handle: u64) -> Option<IrohNode> {
        self.api
            .call(move |actor| {
                Box::pin(async move {
                    actor
                        .get_node_by_stream_handle(handle)
                        .await
                        .ok_or_else(|| anyhow::anyhow!("no node found for given stream handle"))
                })
            })
            .await
            .ok()
    }

    pub async fn add_node(&self, node: IrohNode) {
        let _ = self
            .api
            .call(move |actor| Box::pin(actor.add_node(node)))
            .await;
    }

    pub async fn get_stream_handle_by_iroh_node_id(&self, node_id: NodeId) -> Option<u64> {
        self.api
            .call(move |actor| {
                Box::pin(async move {
                    actor
                        .get_stream_handle_by_iroh_node_id(node_id)
                        .await
                        .ok_or_else(|| anyhow::anyhow!("no stream found for given node id"))
                })
            })
            .await
            .ok()
    }
}

impl IrohTransportActor {
    pub async fn get_handle(&self) -> anyhow::Result<u64> {
        Ok(self.handle)
    }

    pub async fn get_stream_by_handle(&self, handle: u64) -> Option<crate::stream::IrohStream> {
        for node in self.nodes.values() {
            if let Some(stream) = node.get_stream_by_handle(handle).await {
                return Some(stream);
            }
        }
        None
    }

    pub async fn get_node_by_stream_handle(&self, handle: u64) -> Option<IrohNode> {
        for node in self.nodes.values() {
            if node.get_stream_by_handle(handle).await.is_some() {
                return Some(node.clone());
            }
        }
        None
    }

    pub async fn get_stream_handle_by_iroh_node_id(&self, node_id: NodeId) -> Option<u64> {
        for node in self.nodes.values() {
            if let Some(stream) = node
                .get_stream_handle_by_iroh_node_id(node_id.clone())
                .await
            {
                return Some(stream);
            }
        }
        None
    }

    pub async fn add_node(&mut self, node: IrohNode) -> anyhow::Result<()> {
        let handle = node
            .get_handle()
            .await
            .ok_or_else(|| anyhow::anyhow!("no node handle"))?;
        self.nodes.insert(handle, node);
        Ok(())
    }
}

impl Actor for IrohTransportActor {
    async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                Some(action) = self.rx.recv() => action(self).await,
            }
        }
    }
}
