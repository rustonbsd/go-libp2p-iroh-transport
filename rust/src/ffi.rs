use std::{ffi::CStr, time::Duration};
use tracing::{debug, error, info, warn};

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
        warn!("[rust] out_handle is null");
        return -1;
    }

    let (tx, rx) = tokio::sync::oneshot::channel();
    let transport_handle = crate::get_next_transport_handle();
    if let Ok(rt) = crate::runtime_handle() {
        rt.spawn(async move {
            if let Ok(transport) = crate::transport::IrohTransport::new(transport_handle) {
                info!(
                    "[rust] Transport created with handle: {:?}",
                    transport.get_handle().await
                );
                if crate::STATE.add_transport(transport).await.is_err() {
                    error!("[rust] failed to add transport to state");
                    let _ = tx.send(None);
                    return;
                }
                let _ = tx.send(Some(()));
            } else {
                error!("[rust] failed to create transport tokio runtime");
                let _ = tx.send(None);
            }
        })
    } else {
        warn!("[rust] failed to get runtime handle");
        return -1i32;
    };

    if let Ok(Some(_)) = rx.blocking_recv() {
        unsafe {
            *out_handle = IrohTransportHandle(transport_handle);
        }
        0i32
    } else {
        -1i32
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
    if ed25519_priv_ptr.is_null() || out_handle.is_null() {
        return -1;
    }
    info!(
        "iroh_node_new called from: private_key_bytes_len: {}",
        ed25519_priv_len
    );
    let raw_private_key = unsafe { std::slice::from_raw_parts(ed25519_priv_ptr, ed25519_priv_len) };
    let priv_key_bytes: &[u8; 32] = if let Ok(b) = raw_private_key[..32].try_into() {
        b
    } else {
        debug!("[rust] invalid ed25519 private key length");
        return -1;
    };
    let iroh_secret = iroh::SecretKey::from_bytes(priv_key_bytes);
    debug!("Iroh pub key z32 encoded: {}", iroh_secret.public());

    let peer_id = if let Ok(peer_id) = unsafe { CStr::from_ptr(peer_id_raw) }.to_str() {
        peer_id
    } else {
        warn!("[rust] failed to decode peer id");
        return -1;
    };

    if let Some(pub_key) = crate::peer_id_to_ed25519_public_key(peer_id) {
        if iroh_secret.public() != pub_key {
            warn!("[rust] provided ed25519 private key does not match peer id");
            return -1;
        }
    } else {
        warn!("[rust] failed to decode peer id to valid ed25519 public key");
        return -1;
    }

    let (tx, rx) = tokio::sync::oneshot::channel();
    if let Ok(rt) = crate::runtime_handle() {
        rt.spawn(async move {
            let transport = if let Some(transport) = crate::STATE
                .get_transport_by_transport_handle(transport.0)
                .await
            {
                transport
            } else {
                warn!("[rust] transport must be created before node");
                let _ = tx.send(None);
                return;
            };
            if let Ok(node) =
                crate::node::IrohNode::new(crate::get_next_node_handle(), iroh_secret.clone()).await
            {
                transport.add_node(node.clone()).await;
                let _ = tx.send(Some(node.get_handle().await));
            } else {
                warn!("[rust] failed to create IrohNode");
                let _ = tx.send(None);
            }
        })
    } else {
        return -1i32;
    };

    if let Ok(Some(Some(node_handle))) = rx.blocking_recv() {
        debug!("[rust] IrohNode created with handle: {}", node_handle);
        unsafe {
            *out_handle = IrohNodeHandle(node_handle);
        }
        0i32
    } else {
        -1i32
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn iroh_listen(
    node: IrohNodeHandle,
    listen_maddr: *const ::std::os::raw::c_char,
    out_listener: *mut IrohListenerHandle,
) -> i32 {
    if out_listener.is_null() {
        warn!("[rust] out_listener is null");
        return -1;
    }

    let listen_addr_str =
        if let Ok(listen_addr_str) = unsafe { CStr::from_ptr(listen_maddr) }.to_str() {
            listen_addr_str
        } else {
            warn!("[rust] failed to decode listen address");
            return -1;
        };
    debug!("[rust] listening on: {listen_addr_str}");

    let (tx, rx) = tokio::sync::oneshot::channel();
    if let Ok(rt) = crate::runtime_handle() {
        rt.spawn(async move {
            let transport =
                if let Some(transport) = crate::STATE.get_transport_by_node_handle(node.0).await {
                    transport
                } else {
                    warn!("[rust] transport must be created before listen");
                    let _ = tx.send(None);
                    return;
                };

            if let Some(node) = transport.get_node_by_handle(node.0).await {
                let _ = tx.send(Some(node.get_handle().await));
            } else {
                warn!("[rust] invalid node handle");
                let _ = tx.send(None);
            }
        })
    } else {
        warn!("[rust] failed to get runtime handle");
        return -1i32;
    };

    if let Ok(Some(Some(node_handle))) = rx.blocking_recv() {
        unsafe { *out_listener = IrohListenerHandle(node_handle) }
        0i32
    } else {
        -1i32
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn iroh_accept(
    listener: IrohListenerHandle,
    _timeout_ms: u64,
    out_stream: *mut IrohStreamHandle,
) -> i32 {
    let (tx, rx) = tokio::sync::oneshot::channel();
    if let Ok(rt) = crate::runtime_handle() {
        rt.spawn(async move {
            let transport = match crate::STATE.get_transport_by_node_handle(listener.0).await {
                Some(t) => t.clone(),
                _ => {
                    warn!("[rust] iroh_accept transport must be created before accept");
                    let _ = tx.send(None);
                    return;
                }
            };

            let node = if let Some(node) = transport.get_node_by_listener_handle(listener.0).await {
                node
            } else {
                warn!("[rust] invalid listener handle");
                let _ = tx.send(None);
                return;
            };

            let stream_handle = loop {
                if let Some(stream_handle) = node.try_accept_next().await {
                    break stream_handle;
                } else {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            };

            let stream = if let Some(stream) = node.get_stream_by_handle(stream_handle).await {
                stream
            } else {
                warn!("[rust] invalid stream handle");
                let _ = tx.send(None);
                return;
            };
            let _ = tx.send(Some(stream.get_handle().await));
        })
    } else {
        warn!("[rust] failed to get runtime handle");
        return -1i32;
    };

    match rx.blocking_recv() {
        Ok(Some(Ok(stream_handle))) => {
            debug!("[rust] accepted stream with handle: {}", stream_handle);
            unsafe { *out_stream = IrohStreamHandle(stream_handle) }
            0
        }
        _ => -1i32,
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
            warn!("[rust] failed to decode remote address");
            return -1;
        };
    debug!("[rust] dialing: {remote_addr_str}");

    let node_id = match crate::peer_id_to_ed25519_public_key(remote_addr_str) {
        Some(id) => id,
        _ => {
            warn!("[rust] failed to decode peer id from multiaddr");
            return -1;
        }
    };

    let (tx, rx) = tokio::sync::oneshot::channel();
    if let Ok(rt) = crate::runtime_handle() {
        rt.spawn(async move {
            let transport =
                if let Some(transport) = crate::STATE.get_transport_by_node_handle(node.0).await {
                    transport
                } else {
                    warn!("[rust] transport must be created before dial");
                    let _ = tx.send(None);
                    return;
                };
            /*if let Some(stream_handle) = transport.get_stream_handle_by_iroh_node_id(node_id).await {
                info!(
                    "[rust] reusing existing stream with handle: {}",
                    stream_handle
                );
                let _ = tx.send(Some(stream_handle));
                return;
            }*/

            let node = if let Some(node) = transport.get_node_by_handle(node.0).await {
                node
            } else {
                warn!("[rust] invalid node id");
                let _ = tx.send(None);
                return;
            };

            if node
                .get_stream_handle_by_iroh_node_id(node_id)
                .await
                .is_some()
            {
                warn!("[rust] already connected to node id");
                let _ = tx.send(None);
                return;
            }

            if let Ok(stream_handle) = node.connect(node_id).await {
                let _ = tx.send(Some(stream_handle));
            } else {
                warn!("[rust] failed to connect to remote node");
                let _ = tx.send(None);
            }
        })
    } else {
        warn!("[rust] failed to get runtime handle");
        return -1i32;
    };

    match rx.blocking_recv() {
        Ok(Some(handle)) => {
            debug!("[rust] Stream created with handle: {}", handle);
            unsafe { *out_stream = IrohStreamHandle(handle) }
            0i32
        }
        _ => {
            warn!("[rust] failed to dial remote node");
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
    if len == 0 {
        unsafe {
            *out_n = 0;
        }
        return 0i32;
    }
    let buf = unsafe { std::slice::from_raw_parts_mut(buf, len) };

    let (tx, rx) = tokio::sync::oneshot::channel();
    if let Ok(rt) = crate::runtime_handle() {
        rt.spawn(async move {
            let (ltx, lrx) = tokio::sync::oneshot::channel();
            let fut = async move {
                let transport = if let Some(transport) =
                    crate::STATE.get_transport_by_stream_handle(stream.0).await
                {
                    transport
                } else {
                    error!("[rust] iroh_stream_read transport must be created before accept");
                    let _ = ltx.send(None);
                    return;
                };
                let stream = if let Some(stream) = transport.get_stream_by_handle(stream.0).await {
                    stream
                } else {
                    warn!("[rust] invalid stream handle");
                    let _ = ltx.send(None);
                    return;
                };
                if let Ok(size) = stream.read(buf).await {
                    info!("[rust] read {} bytes from stream", size);
                    let _ = ltx.send(Some(size));
                } else {
                    let _ = ltx.send(None);
                }
            };

            if timeout == 0 {
                fut.await;
            } else {
                if tokio::time::timeout(
                    Duration::from_millis(if timeout > 0 { timeout } else { 1000 }),
                    fut,
                )
                .await
                .is_err()
                {
                    warn!("[rust] read from stream timed out");
                    let _ = tx.send(None);
                    return;
                }
            }
            if let Ok(res) = lrx.await {
                let _ = tx.send(res);
            } else {
                let _ = tx.send(None);
            }
        })
    } else {
        warn!("[rust] failed to get runtime handle");
        return -1i32;
    };

    let size = match rx.blocking_recv().unwrap_or(None) {
        Some(size) => size,
        _ => {
            return -1i32;
        }
    };
    if size == 0 {
        // EOF
        unsafe {
            *out_n = -1isize;
        }
        return 0i32;
    }
    unsafe {
        *out_n = size as isize;
    }
    0i32
}

#[unsafe(no_mangle)]
pub extern "C" fn iroh_stream_write(
    stream: IrohStreamHandle,
    buf: *const u8,
    len: usize,
    timeout: u64,
    out_n: *mut isize,
) -> i32 {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let buf = unsafe { std::slice::from_raw_parts(buf, len) };
    if let Ok(rt) = crate::runtime_handle() {
        rt.spawn(async move {
            let (ltx, lrx) = tokio::sync::oneshot::channel();
            let fut = async move {
                let transport = if let Some(transport) =
                    crate::STATE.get_transport_by_stream_handle(stream.0).await
                {
                    transport
                } else {
                    warn!("[rust] iroh_stream_write transport must be created before accept");
                    let _ = ltx.send(None);
                    return;
                };

                let stream = if let Some(stream) = transport.get_stream_by_handle(stream.0).await {
                    stream
                } else {
                    warn!("[rust] invalid stream handle");
                    let _ = ltx.send(None);
                    return;
                };

                if let Ok(size) = stream.write(buf).await {
                    info!("[rust] wrote {} bytes to stream", size);
                    let _ = ltx.send(Some(size));
                } else {
                    let _ = ltx.send(None);
                }
            };

            if timeout == 0 {
                fut.await;
            } else {
                if tokio::time::timeout(Duration::from_millis(timeout), fut)
                    .await
                    .is_err()
                {
                    warn!("[rust] write to stream timed out");
                    let _ = tx.send(None);
                    return;
                }
            }

            if let Ok(res) = lrx.await {
                let _ = tx.send(res);
            } else {
                let _ = tx.send(None);
            }
        })
    } else {
        warn!("[rust] failed to get runtime handle");
        return -1i32;
    };

    if let Some(size) = rx.blocking_recv().unwrap_or(None) {
        unsafe {
            *out_n = size as isize;
        }
        return 0i32;
    } else {
        return -1i32;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn iroh_stream_close(stream: IrohStreamHandle) -> i32 {
    let (tx, rx) = tokio::sync::oneshot::channel();
    if let Ok(rt) = crate::runtime_handle() {
        rt.spawn(async move {
            let transport =
                if let Some(t) = crate::STATE.get_transport_by_stream_handle(stream.0).await {
                    t.clone()
                } else {
                    warn!("[rust] iroh_stream_close transport must be created before accept");
                    let _ = tx.send(None);
                    return;
                };
            if let Some(node) = transport.get_node_by_stream_handle(stream.0).await {
                if let Some(_) = node.get_stream_by_handle(stream.0).await {
                    node.remove_stream_by_handle(stream.0).await;
                    debug!("[rust] closed stream with handle: {}", stream.0);
                }

                let _ = tx.send(Some(()));
            } else {
                warn!("[rust] invalid stream handle");
                let _ = tx.send(None);
            }
        })
    } else {
        warn!("[rust] failed to get runtime handle");
        return -1i32;
    };

    if let Ok(Some(_)) = rx.blocking_recv() {
        0i32
    } else {
        -1i32
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn iroh_shutdown() -> i32 {
    if let Some(runtime) = crate::RUNTIME.get() {
        if let Ok(mut runtime) = runtime.try_lock() {
            if let Some(runtime) = runtime.take() {
                runtime.shutdown_timeout(std::time::Duration::from_secs(10));
                return 0i32;
            }
        }
    }
    return -1i32;
}
