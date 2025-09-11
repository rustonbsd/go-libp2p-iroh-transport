# go-libp2p-iroh-transport

A tiny libp2p Transport that routes all traffic over iroh (QUIC) via a small Rust FFI. Built for plugging iroh into libp2p and DefraDB with a clean Go API.

- Backed by iroh 0.91 (QUIC + discovery)
- Ed25519-only identity (validated against the PeerID)
- ALPN: /libp2p/iroh/0.1.0
- One global iroh runtime per process

# Research

- rust compile to wasm no static or dyn lib:
  - https://blog.arcjet.com/calling-rust-ffi-libraries-from-go/
  - https://blog.arcjet.com/webassembly-on-the-server-compiling-rust-to-wasm-and-executing-it-from-go/

- dynamic embed in go from this guide used:
  - https://github.com/mediremi/rust-plus-golang/tree/master

## Thoughts
- This seems to be best practice in go ffi -> Switch back to dyn lib again, extract .so at runtime? smaller binary but depends on your goals
- Centeralize all ffi bindings in cgo_wrapper.go and only import wrapped handles in conn and transport (fine for this prototype for now)

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

