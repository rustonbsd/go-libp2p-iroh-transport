use std::collections::HashMap;

use iroh::protocol::ProtocolHandler;
use tracing::{debug, info, warn};

use crate::{
    actor::{Action, Actor, Handle},
    stream::{ConnectionShould, IrohStream},
};

#[derive(Debug, Clone)]
pub struct IrohNode {
    api: Handle<IrohNodeActor>,
}

#[derive(Debug)]
struct IrohNodeActor {
    rx: tokio::sync::mpsc::Receiver<Action<IrohNodeActor>>,
    handle: u64,

    endpoint: iroh::Endpoint,
    router: Option<iroh::protocol::Router>,

    streams: HashMap<u64, IrohStream>,

    last_accepted_handles_sender: tokio::sync::mpsc::Sender<u64>,
    last_accepted_handles_receiver: tokio::sync::mpsc::Receiver<u64>,
}

impl IrohNode {
    pub async fn new(handle: u64, secret_key: iroh::SecretKey) -> anyhow::Result<Self> {
        let (api, rx) = Handle::channel(1024);
        let _self = Self { api: api.clone() };
        let protocol_handler = _self.clone();
        let endpoint = iroh::Endpoint::builder()
            .discovery_n0()
            .secret_key(secret_key.clone())
            .bind()
            .await
            .expect("failed to bind endpoint (fatal)");

        let mut actor = IrohNodeActor::new(rx, handle, endpoint.clone())
            .await
            .expect("failed to spawn IrohNodeActor (fatal)");
        let router = iroh::protocol::RouterBuilder::new(endpoint.clone())
            .accept(crate::ALPN, protocol_handler)
            .spawn();
        actor.set_router(router).await;

        crate::runtime_handle().spawn(async move {
            debug!("[rust] IrohNodeActor started");
            if let Err(e) = actor.run().await {
                warn!("[rust] IrohNodeActor exited with error: {:?}", e);
            }
        });

        Ok(_self)
    }

    pub async fn get_stream_by_handle(&self, handle: u64) -> Option<IrohStream> {
        self.api
            .call(move |actor| {
                Box::pin(async move { Ok(actor.get_stream_by_handle(handle).await) })
            })
            .await
            .ok()?
    }

    pub async fn get_handle(&self) -> u64 {
        self.api
            .call(move |actor| Box::pin(async move { Ok(actor.get_handle().await) }))
            .await
            .unwrap_or_default()
    }

    pub async fn try_accept_next(&self) -> Option<u64> {
        self.api
            .call(move |actor| Box::pin(async move { Ok(actor.external_try_accept_next().await) }))
            .await
            .ok()?
    }

    pub async fn remove_stream_by_handle(&self, handle: u64) {
        let _ = self
            .api
            .call(move |actor| {
                Box::pin(async move { Ok(actor.remove_stream_by_handle(handle).await) })
            })
            .await;
    }

    pub async fn connect(&self, node_id: iroh::NodeId) -> anyhow::Result<u64> {
        self.api
            .call(move |actor| Box::pin(actor.connect(node_id)))
            .await
    }

    pub async fn get_stream_handle_by_iroh_node_id(&self, node_id: iroh::NodeId) -> Option<u64> {
        self.api
            .call(move |actor| {
                Box::pin(async move {
                    if let Some(stream) = actor.get_stream_by_iroh_node_id(node_id).await {
                        stream.get_handle().await
                    } else {
                        Err(anyhow::anyhow!("no stream found for given node id"))
                    }
                })
            })
            .await
            .ok()
    }
}

impl IrohNodeActor {
    pub async fn new(
        rx: tokio::sync::mpsc::Receiver<Action<IrohNodeActor>>,
        node_handle: u64,
        endpoint: iroh::Endpoint,
    ) -> anyhow::Result<Self> {
        let (last_accepted_handles_sender, last_accepted_handles_receiver) =
            tokio::sync::mpsc::channel(1024);

        Ok(Self {
            rx,
            handle: node_handle,
            endpoint,
            router: None,
            streams: HashMap::new(),
            last_accepted_handles_sender,
            last_accepted_handles_receiver,
        })
    }

    async fn set_router(&mut self, router: iroh::protocol::Router) {
        self.router = Some(router);
    }

    async fn get_handle(&self) -> u64 {
        self.handle
    }

    async fn handle_incoming(&mut self, conn: iroh::endpoint::Connection) -> anyhow::Result<u64> {
        let handle = crate::get_next_stream_handle();
        self.streams.insert(
            handle,
            IrohStream::new(handle, conn, ConnectionShould::Accept).await?,
        );

        self.last_accepted_handles_sender
            .send(handle)
            .await
            .map_err(|e| anyhow::anyhow!("failed to send last accepted handle: {}", e))?;
        Ok(handle)
    }

    async fn external_try_accept_next(&mut self) -> Option<u64> {
        self.last_accepted_handles_receiver.try_recv().ok()
    }

    async fn get_stream_by_handle(&self, handle: u64) -> Option<IrohStream> {
        self.streams.get(&handle).cloned()
    }

    async fn get_stream_by_iroh_node_id(&self, node_id: iroh::NodeId) -> Option<IrohStream> {
        for stream in self.streams.values() {
            if stream.get_iroh_node_id().await.ok()? == node_id {
                return Some(stream.clone());
            }
        }
        None
    }

    async fn remove_stream_by_handle(&mut self, handle: u64) {
        if let Some(stream) = self.streams.get(&handle) {
            stream.stop().await;
            self.streams.remove(&handle);
        }
    }

    async fn connect(&mut self, node_id: iroh::NodeId) -> anyhow::Result<u64> {
        let conn = self.endpoint.connect(node_id, crate::ALPN).await?;

        let handle = crate::get_next_stream_handle();
        let stream = IrohStream::new(handle, conn, ConnectionShould::Open).await?;
        self.streams.insert(handle, stream);

        info!("[rust] new stream handle={handle} with node id: {node_id}");

        Ok(handle)
    }
}

impl Actor for IrohNodeActor {
    async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            tokio::select! {
                Some(action) = self.rx.recv() => {
                    action(self).await
                }
                _ = tokio::signal::ctrl_c() => break,
            }
        }
        Ok(())
    }
}

impl ProtocolHandler for IrohNode {
    async fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> Result<(), iroh::protocol::AcceptError> {
        let node_id = connection.remote_node_id()?;
        info!("[rust] accepting connection from node id: {node_id}");

        let stream_handle = self
            .api
            .call(move |actor| Box::pin(async move { actor.handle_incoming(connection).await }))
            .await
            .map_err(|_| iroh::protocol::AcceptError::NotAllowed {})?;

        let stream = self
            .get_stream_by_handle(stream_handle)
            .await
            .ok_or(iroh::protocol::AcceptError::NotAllowed {})?;

        while stream.get_status().await.map_err(|e| {
            warn!("failed to get stream status: {}", e);
            iroh::protocol::AcceptError::NotAllowed {}
        })? != crate::stream::ActorStatus::Stopped
        {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        debug!("[rust] connection accepted closed");

        Ok(())
    }
}
