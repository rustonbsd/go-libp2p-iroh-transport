use std::sync::Arc;

use iroh::endpoint::VarInt;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, sync::Mutex};
use tracing::{debug, warn};

use crate::actor::{Action, Actor, Handle};

#[derive(Debug, Clone)]
pub struct IrohStream {
    api: Handle<IrohStreamActor>,

    sender: Arc<Mutex<iroh::endpoint::SendStream>>,
    receiver: Arc<Mutex<iroh::endpoint::RecvStream>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActorStatus {
    Running,
    Stopped,
}

#[derive(Debug)]
struct IrohStreamActor {
    rx: tokio::sync::mpsc::Receiver<Action<IrohStreamActor>>,
    handle: u64,
    actor_status: ActorStatus,

    node_id: iroh::NodeId,
    _conn: iroh::endpoint::Connection,
}

#[derive(Debug, Clone)]
pub enum ConnectionShould {
    Accept,
    Open,
}

impl IrohStream {
    pub async fn new(
        handle: u64,
        conn: iroh::endpoint::Connection,
        connection_should: ConnectionShould,
    ) -> anyhow::Result<Self> {
        let (api, rx) = Handle::channel(1024);

        // Open (or accept) the QUIC bidi stream before spawning the actor.
        // IMPORTANT: write to the open stream and read from the accept stream 1 byte to get things going.
        // (quirks of the iroh).
        let (conn_sender, conn_receiver) = match connection_should {
            ConnectionShould::Accept => {
                let mut split = conn.accept_bi().await?;
                split.1.read_u8().await?;
                split
            }
            ConnectionShould::Open => {
                let mut split = conn.open_bi().await?;
                split.0.write_u8(0).await?;
                split.0.flush().await?;
                split
            }
        };

        // Spawn the stream actor – no handshake / blocking wait.
        crate::runtime_handle().spawn(async move {
            let mut actor =
                match IrohStreamActor::new(rx, handle, conn).await {
                    Ok(a) => a,
                    Err(e) => {
                        warn!("failed to construct IrohStreamActor: {e}");
                        return;
                    }
                };

            if let Err(e) = actor.run().await {
                warn!("IrohStreamActor exited with error: {:?}", e);
            }
        });

        Ok(Self { api, sender: Arc::new(Mutex::new(conn_sender)), receiver: Arc::new(Mutex::new(conn_receiver)) })
    }

    pub async fn get_handle(&self) -> anyhow::Result<u64> {
        self.api
            .call(move |actor| Box::pin(actor.get_handle()))
            .await
    }

    pub async fn get_status(&self) -> anyhow::Result<ActorStatus> {
        self.api
            .call(move |actor| Box::pin(actor.get_status()))
            .await
    }

    pub async fn read(&self, buf: &mut [u8]) -> anyhow::Result<usize> {
        self.receiver.lock().await.read(buf).await?.ok_or(anyhow::anyhow!("failed to read from remote"))
    }

    pub async fn write(&self, buf: &[u8]) -> anyhow::Result<usize> {
        self.sender.lock().await.write(buf).await.map_err(Into::into)
    }

    pub async fn stop(&self) {
        let _ = self
            .api
            .call(move |actor| {
                Box::pin(async move {
                    actor.stop().await;
                    Ok(())
                })
            })
            .await;
    }

    pub async fn get_iroh_node_id(&self) -> anyhow::Result<iroh::NodeId> {
        self.api
            .call(move |actor| Box::pin(actor.get_node_id()))
            .await
    }
}

impl IrohStreamActor {
    pub async fn new(
        rx: tokio::sync::mpsc::Receiver<Action<IrohStreamActor>>,
        handle: u64,
        conn: iroh::endpoint::Connection,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            actor_status: ActorStatus::Running,
            rx,
            handle,
            node_id: conn.remote_node_id()?,
            _conn: conn,
        })
    }

    async fn get_handle(&self) -> anyhow::Result<u64> {
        Ok(self.handle)
    }

    async fn stop(&mut self) {
        debug!("[rust] stopping stream with handle: {}", self.handle);
        self._conn
            .close(VarInt::default(), b"stopped called on strem wrapper");
        self.actor_status = ActorStatus::Stopped;
    }

    async fn get_node_id(&self) -> anyhow::Result<iroh::NodeId> {
        Ok(self.node_id)
    }

    async fn get_status(&self) -> anyhow::Result<ActorStatus> {
        Ok(self.actor_status.clone())
    }
}

impl Actor for IrohStreamActor {
    async fn run(&mut self) -> anyhow::Result<()> {
        while self.actor_status == ActorStatus::Running {
            tokio::select! {
                Some(action) = self.rx.recv() => action(self).await,
                _ = tokio::signal::ctrl_c() => break,
            }
        }

        self.actor_status = ActorStatus::Stopped;
        Ok(())
    }
}
