# go-libp2p-iroh-transport

A tiny libp2p Transport that routes all traffic over iroh (QUIC) via a small Rust FFI. Built for plugging iroh into libp2p and DefraDB with a clean Go API.

- Backed by iroh 0.91 (QUIC + discovery)
- Ed25519-only identity (validated against the PeerID)
- ALPN: /libp2p/iroh/0.1.0
- One global iroh runtime per process

## Thoughts
- Switch back to dyn lib again, extract .so at runtime? smaller binary but depends on your goals


## Status

- Dial, listen, accept, read, write, close: working
- Target: Linux/amd64 (add more by building extra staticlibs)
- Peer-ID–driven dialing; multiaddr is for bookkeeping

## Install

```bash
go get github.com/rustonbsd/go-libp2p-iroh-transport
```

Build the Rust staticlib and header once (required for cgo):

```bash
cargo install cbindgen
make build-rust
```

Full build (Rust + Go):

```bash
make
```

Artifacts:
- include/libirohffi.h
- lib/linux_amd64/libirohffi.a

## Notes

- Ed25519 only; we verify the private key matches the PeerID
- One iroh runtime/transport per process (current design)
- Expand target platforms by adding staticlibs for those triples

