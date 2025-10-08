use std::{ffi::CStr, time::Duration};
use tracing::{debug, error, info, warn};

use crate::runtime::{Token, cancel_token_for, run_on_runtime, shutdown_runtime};

#[repr(C)]
pub struct IrohNodeHandle(u64);
#[repr(C)]
pub struct IrohListenerHandle(u64);
#[repr(C)]
pub struct IrohStreamHandle(u64);
#[repr(C)]
pub struct IrohTransportHandle(u64);

#[unsafe(no_mangle)]
pub extern "C" fn iroh_transport_new(out_handle: *mut IrohTransportHandle) -> i32 {
    if out_handle.is_null() {
        warn!("[rust-transport] out_handle is null");
        return -1;
    }

    debug!("[rust-transport] creating transport");
    let transport_handle = crate::get_next_transport_handle();

    match run_on_runtime(Token::Transport(transport_handle), async move {
        let transport = crate::transport::IrohTransport::new(transport_handle)?;
        info!(
            "[rust-transport-{}] Transport created with handle: {:?}",
            transport_handle,
            transport.get_handle().await
        );
        crate::STATE.add_transport(transport).await?;
        Ok(())
    }) {
        Ok(_) => {
            unsafe {
                *out_handle = IrohTransportHandle(transport_handle);
            }
            0i32
        }
        Err(err) => {
            error!(
                "[rust-transport-{}] failed to spawn transport creation task: {}",
                transport_handle, err
            );
            -1i32
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn iroh_node_new(
    transport: IrohTransportHandle,
    ed25519_priv_ptr: *const u8,
    ed25519_priv_len: usize,
    peer_id_raw: *const ::std::os::raw::c_char,
    out_handle: *mut IrohNodeHandle,
) -> i32 {
    debug!("[rust-transport-{}] creating node", transport.0);
    if ed25519_priv_ptr.is_null() || out_handle.is_null() {
        return -1;
    }
    info!(
        "iroh_node_new called from: private_key_bytes_len: {}",
        ed25519_priv_len
    );
    let node_handle = crate::get_next_node_handle();

    let raw_private_key = unsafe { std::slice::from_raw_parts(ed25519_priv_ptr, ed25519_priv_len) };
    let priv_key_bytes: &[u8; 32] = if let Ok(b) = raw_private_key[..32].try_into() {
        b
    } else {
        debug!(
            "[rust-node-{}] invalid ed25519 private key length",
            node_handle
        );
        return -1;
    };
    let iroh_secret = iroh::SecretKey::from_bytes(priv_key_bytes);
    debug!("Iroh pub key z32 encoded: {}", iroh_secret.public());

    let peer_id = if let Ok(peer_id) = unsafe { CStr::from_ptr(peer_id_raw) }.to_str() {
        peer_id
    } else {
        warn!("[rust-node-{}] failed to decode peer id", node_handle);
        return -1;
    };

    if let Some(pub_key) = crate::peer_id_to_ed25519_public_key(peer_id) {
        if iroh_secret.public() != pub_key {
            warn!(
                "[rust-node-{}] provided ed25519 private key does not match peer id",
                node_handle
            );
            return -1;
        }
    } else {
        warn!(
            "[rust-node-{}] failed to decode peer id to valid ed25519 public key",
            node_handle
        );
        return -1;
    }

    match run_on_runtime(Token::Node(node_handle), async move {
        let transport = if let Some(transport) = crate::STATE
            .get_transport_by_transport_handle(transport.0)
            .await
        {
            transport
        } else {
            return Err(anyhow::anyhow!("transport must be created before node"));
        };
        if let Ok(node) = crate::node::IrohNode::new(node_handle.clone(), iroh_secret.clone()).await
        {
            transport.add_node(node.clone()).await;
            Ok(node
                .get_handle()
                .await
                .ok_or(anyhow::anyhow!("no node handle"))?)
        } else {
            Err(anyhow::anyhow!("failed to create IrohNode"))
        }
    }) {
        Ok(_) => {
            debug!(
                "[rust-node-{}] IrohNode created with handle: {}",
                node_handle, node_handle
            );
            unsafe {
                *out_handle = IrohNodeHandle(node_handle);
            }
            0i32
        }
        Err(err) => {
            warn!(
                "[rust-node-{}] failed to create IrohNode: {}",
                node_handle, err
            );
            -1i32
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn iroh_listen(
    node: IrohNodeHandle,
    listen_maddr: *const ::std::os::raw::c_char,
    out_listener: *mut IrohListenerHandle,
) -> i32 {
    if out_listener.is_null() {
        warn!("[rust-node-{}] out_listener is null", node.0);
        return -1;
    }
    debug!("[rust-node-{}] listening on: {:?}", node.0, listen_maddr);

    let listen_addr_str =
        if let Ok(listen_addr_str) = unsafe { CStr::from_ptr(listen_maddr) }.to_str() {
            listen_addr_str
        } else {
            warn!("[rust-node-{}] failed to decode listen address", node.0);
            return -1;
        };
    debug!("[rust-node-{}] listening on: {listen_addr_str}", node.0);

    match run_on_runtime(Token::Node(node.0), async move {
        let transport = crate::STATE
            .get_transport_by_node_handle(node.0)
            .await
            .ok_or_else(|| anyhow::anyhow!("transport must be created before listen"))?;

        let node = transport
            .get_node_by_node_handle(node.0)
            .await
            .ok_or_else(|| anyhow::anyhow!("invalid node handle"))?;

        node.get_handle()
            .await
            .ok_or_else(|| anyhow::anyhow!("failed to get node handle"))
    }) {
        Ok(node_handle) => {
            unsafe { *out_listener = IrohListenerHandle(node_handle) }
            0i32
        }
        Err(err) => {
            warn!("[rust-node-{}] failed to listen: {}", node.0, err);
            -1i32
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn iroh_accept(
    listener: IrohListenerHandle,
    _timeout_ms: u64,
    out_stream: *mut IrohStreamHandle,
) -> i32 {
    debug!("[rust-listener-{}] accepting stream", listener.0);
    match run_on_runtime(Token::Node(listener.0), async move {
        let transport = crate::STATE
            .get_transport_by_node_handle(listener.0)
            .await
            .ok_or_else(|| anyhow::anyhow!("transport must be created before accept"))?;

        let node = transport
            .get_node_by_listener_handle(listener.0)
            .await
            .ok_or_else(|| anyhow::anyhow!("invalid listener handle"))?;

        let stream_handle = loop {
            if let Some(stream_handle) = node.try_accept_next().await {
                break stream_handle;
            } else {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        };

        let stream = node
            .get_stream_by_handle(stream_handle)
            .await
            .ok_or_else(|| anyhow::anyhow!("invalid stream handle"))?;

        stream.get_handle().await
    }) {
        Ok(stream_handle) => {
            debug!(
                "[rust-listener-{}] accepted stream with handle: {}",
                listener.0, stream_handle
            );
            unsafe { *out_stream = IrohStreamHandle(stream_handle) }
            0
        }
        Err(err) => {
            if err.to_string() != "request cancelled" {
                warn!(
                    "[rust-listener-{}] failed to accept stream: {}",
                    listener.0, err
                );
            }
            -1i32
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn iroh_dial(
    node: IrohNodeHandle,
    _remote_maddr: *const ::std::os::raw::c_char,
    out_stream: *mut IrohStreamHandle,
) -> i32 {
    if out_stream.is_null() {
        return -1i32;
    }

    let remote_addr_str =
        if let Ok(remote_addr_str) = unsafe { CStr::from_ptr(_remote_maddr) }.to_str() {
            remote_addr_str
        } else {
            warn!("[rust-node-{}] failed to decode remote address", node.0);
            return -1;
        };
    debug!("[rust-node-{}] dialing: {remote_addr_str}", node.0);

    let node_id = match crate::peer_id_to_ed25519_public_key(remote_addr_str) {
        Some(id) => id,
        _ => {
            warn!(
                "[rust-node-{}] failed to decode peer id from multiaddr",
                node.0
            );
            return -1;
        }
    };

    let stream_handle = crate::get_next_stream_handle();
    match run_on_runtime(Token::Stream(stream_handle), async move {
        let transport = crate::STATE
            .get_transport_by_node_handle(node.0)
            .await
            .ok_or_else(|| anyhow::anyhow!("transport must be created before dial"))?;

        let node = transport
            .get_node_by_node_handle(node.0)
            .await
            .ok_or_else(|| anyhow::anyhow!("invalid node id"))?;

        if node
            .get_stream_handle_by_iroh_node_id(node_id)
            .await
            .is_some()
        {
            return Err(anyhow::anyhow!("already connected to node id"));
        }

        node.connect(node_id).await
    }) {
        Ok(handle) => {
            debug!(
                "[rust-stream-{}] Stream created with handle: {}",
                handle, handle
            );
            unsafe { *out_stream = IrohStreamHandle(handle) }
            0i32
        }
        Err(err) => {
            warn!(
                "[rust-stream-{}] failed to dial remote node: {}",
                stream_handle, err
            );
            -1i32
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn iroh_stream_read(
    stream: IrohStreamHandle,
    buf: *mut u8,
    len: usize,
    timeout: u64,
    out_n: *mut isize,
) -> i32 {
    if buf.is_null() || out_n.is_null() {
        return -1i32;
    }
    debug!(
        "[rust-stream-{}] reading {} bytes from stream",
        stream.0, len
    );
    if len == 0 {
        unsafe {
            *out_n = 0;
        }
        return 0i32;
    }
    let buf = unsafe { std::slice::from_raw_parts_mut(buf, len) };

    match run_on_runtime(Token::Stream(stream.0), async move {
        let read_fut = async {
            let transport = crate::STATE
                .get_transport_by_stream_handle(stream.0)
                .await
                .ok_or_else(|| anyhow::anyhow!("transport must be created before read"))?;

            let stream = transport
                .get_stream_by_handle(stream.0)
                .await
                .ok_or_else(|| anyhow::anyhow!("invalid stream handle"))?;

            let size = stream.read(buf).await?;
            debug!(
                "[rust-stream-{}] read {} bytes from stream",
                stream.get_handle().await.unwrap_or(999),
                size
            );
            Ok(size)
        };

        if timeout == 0 {
            read_fut.await
        } else {
            tokio::time::timeout(Duration::from_millis(timeout), read_fut)
                .await
                .map_err(|_| anyhow::anyhow!("read from stream timed out"))?
        }
    }) {
        Ok(size) => {
            if size == 0 {
                // EOF
                unsafe {
                    *out_n = -1isize;
                }
                0i32
            } else {
                unsafe {
                    *out_n = size as isize;
                }
                0i32
            }
        }
        Err(err) => {
            if err.to_string() != "request cancelled" {
                warn!(
                    "[rust-stream-{}] failed to read from stream: {}",
                    stream.0, err
                );
            }
            -1i32
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn iroh_stream_write(
    stream: IrohStreamHandle,
    buf: *const u8,
    len: usize,
    timeout: u64,
    out_n: *mut isize,
) -> i32 {
    let buf = unsafe { std::slice::from_raw_parts(buf, len) };
    let stream_handle = stream.0;
    debug!(
        "[rust-stream-{}] writing {} bytes to stream",
        stream_handle, len
    );

    match run_on_runtime(Token::Stream(stream.0), async move {
        let write_fut = async {
            let transport = crate::STATE
                .get_transport_by_stream_handle(stream.0)
                .await
                .ok_or_else(|| anyhow::anyhow!("transport must be created before write"))?;

            let stream = transport
                .get_stream_by_handle(stream.0)
                .await
                .ok_or_else(|| anyhow::anyhow!("invalid stream handle"))?;

            let size = stream.write(buf).await?;
            debug!(
                "[rust-stream-{}] wrote {} bytes to stream",
                stream_handle, size
            );
            Ok(size)
        };

        if timeout == 0 {
            write_fut.await
        } else {
            tokio::time::timeout(Duration::from_millis(timeout), write_fut)
                .await
                .map_err(|_| anyhow::anyhow!("write to stream timed out"))?
        }
    }) {
        Ok(size) => {
            unsafe {
                *out_n = size as isize;
            }
            0i32
        }
        Err(err) => {
            if err.to_string() != "request cancelled" {
                warn!(
                    "[rust-stream-{}] failed to write to stream: {}",
                    stream.0, err
                );
            }
            -1i32
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn iroh_stream_close(stream: IrohStreamHandle) -> i32 {
    debug!("[rust-stream-{}] closing stream", stream.0);
    match run_on_runtime(Token::Stream(stream.0), async move {
        if let Some(transport) = crate::STATE.get_transport_by_stream_handle(stream.0).await {
            let node = transport
                .get_node_by_stream_handle(stream.0)
                .await
                .ok_or_else(|| anyhow::anyhow!("invalid stream handle"))?;

            cancel_token_for(Token::Stream(stream.0));

            if node.get_stream_by_handle(stream.0).await.is_some() {
                node.remove_stream_by_handle(stream.0).await;
                debug!(
                    "[rust-stream-{}] closed stream with handle: {}",
                    stream.0, stream.0
                );
            }
        }

        Ok(())
    }) {
        Ok(_) => 0i32,
        Err(err) => {
            if err.to_string() != "request cancelled" {
                warn!("[rust-stream-{}] failed to close stream: {}", stream.0, err);
            }
            -1i32
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn iroh_listen_close(node_handle: IrohNodeHandle) -> i32 {
    debug!("[rust-node-{}] closing listener", node_handle.0);
    match run_on_runtime(Token::Node(node_handle.0), async move {
        if let Some(transport) = crate::STATE
            .get_transport_by_node_handle(node_handle.0)
            .await
        {
            if transport
                .get_node_by_node_handle(node_handle.0)
                .await
                .is_some()
            {
                cancel_token_for(Token::Node(node_handle.0));

                transport.remove_node_by_handle(node_handle.0).await;
                debug!(
                    "[rust-node-{}] closed listener with handle: {}",
                    node_handle.0, node_handle.0
                );
            }
        }
        Ok(())
    }) {
        Ok(_) => 0i32,
        Err(err) => {
            if err.to_string() != "request cancelled" {
                warn!(
                    "[rust-node-{}] failed to close listener: {}",
                    node_handle.0, err
                );
            }
            -1i32
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn iroh_transport_close(transport_handle: IrohTransportHandle) -> i32 {
    debug!("[rust-transport-{}] closing transport", transport_handle.0);
    match run_on_runtime(Token::Transport(transport_handle.0), async move {
        if crate::STATE
            .get_transport_by_transport_handle(transport_handle.0)
            .await
            .is_some()
        {
            crate::STATE.remove_transport(transport_handle.0).await;
            cancel_token_for(Token::Transport(transport_handle.0));
        }
        Ok(())
    }) {
        Ok(_) => 0i32,
        Err(err) => {
            if err.to_string() != "request cancelled" {
                error!(
                    "[rust-transport-{}] failed to close transport: {}",
                    transport_handle.0, err
                );
            }
            -1i32
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn iroh_shutdown() -> i32 {
    match shutdown_runtime() {
        Ok(_) => 0i32,
        Err(err) => {
            error!("[rust-runtime] failed to shutdown runtime: {}", err);
            -1i32
        }
    }
}
