use std::{collections::{hash_map::Entry, HashMap}, sync::Arc};

use once_cell::sync::OnceCell;
use tokio::{runtime::{Builder, Runtime}, sync::Mutex};
use tokio_util::sync::CancellationToken;
use tracing::debug;


static RUNTIME: OnceCell<Mutex<Option<Runtime>>> = OnceCell::new();
static RUNTIME_HANDLE: OnceCell<tokio::runtime::Handle> = OnceCell::new();

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Token {
    Transport(u64),
    Node(u64),
    Stream(u64),
}

static CANCELLATION_TOKENS: OnceCell<Arc<Mutex<HashMap<Token, Vec<CancellationToken>>>>> =
    OnceCell::new();

pub fn get_cancellation_tokens() -> Arc<Mutex<HashMap<Token, Vec<CancellationToken>>>> {
    CANCELLATION_TOKENS
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}

pub fn runtime_handle() -> anyhow::Result<tokio::runtime::Handle> {
    if RUNTIME.get().is_none() {
        RUNTIME.get_or_init(|| {
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .init();

            let runtime = Builder::new_multi_thread()
                .enable_all()
                .thread_name("iroh-rt")
                .build()
                .expect("build runtime");
            RUNTIME_HANDLE
                .set(runtime.handle().clone())
                .expect("set runtime handle");
            Mutex::new(Some(runtime))
        });
    }
    Ok(RUNTIME_HANDLE
        .get()
        .ok_or_else(|| anyhow::anyhow!("runtime not initialized"))?
        .clone())
}

pub fn run_on_runtime<F, T>(
    token: Token,
    future: F,
) -> anyhow::Result<T>
where
    F: std::future::Future<Output = anyhow::Result<T>> + Send + 'static,
    T: Send + 'static,
{
    let runtime = runtime_handle()?;
    let (tx, rx) = tokio::sync::oneshot::channel();

    runtime.spawn(async move {
        let cancelation_token = CancellationToken::new();
        let tokens = get_cancellation_tokens();
        {
            let mut tokens = tokens.lock().await;
            let entry = tokens.entry(token.clone()) ;
            match entry {
                Entry::Occupied(mut entry) => {
                    entry.get_mut().push(cancelation_token.clone());
                },
                Entry::Vacant(entry) => {
                    entry.insert(vec![cancelation_token.clone()]);
                },
            }
        }
        debug!("request spawned: {token:?}");

        let mut handle = tokio::spawn(future);

        let result = tokio::select! {
            _ = cancelation_token.cancelled() => {
                debug!("request cancelled: {token:?}");
                handle.abort();
                Err(anyhow::anyhow!("request cancelled"))
            }
            res = &mut handle => {
                debug!("request completed: {token:?}");
                res.unwrap_or_else(|e| Err(anyhow::anyhow!("request error: {}", e)))
            }
        };

        let _ = tx.send(result);
        
        // Clean up the cancellation token
        {
            cancelation_token.cancel();
            let mut tokens = tokens.lock().await;
            let entry = tokens.entry(token.clone()) ;
            match entry {
                Entry::Occupied(mut entry) => {
                    entry.get_mut().retain(|t| !t.is_cancelled());
                },
                _ => {}
            }
        }
    });

    rx.blocking_recv().map_err(|_| anyhow::anyhow!("request cancelled"))?
}

pub fn cancel_token_for(token: Token) {
    let tokens = get_cancellation_tokens();
    if let Ok(mut tokens) = tokens.try_lock() {
        let entry = tokens.entry(token.clone()) ;
        match entry {
            Entry::Occupied(mut entry) => {
                while let Some(token) = entry.get_mut().pop() {
                    if !token.is_cancelled() {
                        token.cancel();
                    }
                }
            },
            _ =>  {
                debug!("no cancellation token found for: {token:?}");
            },
        }
    }
}

pub fn shutdown_runtime() -> anyhow::Result<()> {
    if let Some(runtime) = RUNTIME.get() {
        let tokens = get_cancellation_tokens();
        let mut tokens = tokens.blocking_lock();
        for (_, tokens) in tokens.iter_mut() {
            for token in tokens.iter_mut() {
                if !token.is_cancelled() {
                    token.cancel();
                }
            }
        }
        debug!("shutting down runtime");
        if let Ok(mut runtime) = runtime.try_lock() {
            if let Some(runtime) = runtime.take() {
                runtime.shutdown_timeout(std::time::Duration::from_millis(500));
                return Ok(());
            }
        }
    }
    Err(anyhow::anyhow!("failed to shutdown runtime"))
}