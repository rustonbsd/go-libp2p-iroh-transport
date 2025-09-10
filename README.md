go-libp2p-iroh-transport
A minimal libp2p Transport that routes all connections over the iroh stack (QUIC) via a tiny Rust FFI. Built to plug iroh into libp2p/DefraDB with a clean Go API and a small Rust staticlib.

What this is
- A libp2p Transport implementation backed by iroh (0.91)
- Rust static library + cbindgen-generated header, linked via cgo
- Ed25519-only identity (required by iroh); we validate the host’s PeerID matches the provided key
- Uses an ALPN of /libp2p/iroh/0.1.0

Why
- Reuse iroh’s QUIC stack and discovery while keeping libp2p’s host/swarm APIs
- Drop-in transport for DefraDB experiments with iroh as the network substrate

Status
- Works for basic dialing/listening, stream open/accept, read/write, close
- Experimental: Linux/amd64 first; others welcome via additional staticlib builds
- Multiaddr is used for protocol bookkeeping; peer identity (Ed25519) is what actually matters on dial
- No SO_REUSEPORT; a synthetic local TCP addr is used internally for upgrader bookkeeping
- One global iroh runtime/transport per process (today)

Repo layout
- Go transport: package iroh (module: github.com/rustonbsd/go-libp2p-iroh-transport)
- Rust FFI crate: irohffi (staticlib + rlib)
- Generated header: include/libirohffi.h
- Static lib output: lib/linux_amd64/libirohffi.a

Prereqs
- Go 1.24+ with CGO_ENABLED=1
- Rust toolchain + cbindgen
- A working C toolchain (clang/gcc)

Build
- First time (generate header + staticlib, then build Go):
  - Rust staticlib + header
    ```bash
    cargo install cbindgen
    make build-rust
    ```
  - Full build (Rust + Go example binary)
    ```bash
    make
    ```
- In your own project, run make build-rust once (or vendor the lib) so cgo can link against libirohffi.a.

Install
```bash
go get github.com/rustonbsd/go-libp2p-iroh-transport
```
Make sure you’ve produced the Rust static lib and header (make build-rust) before building your app.

Quick start (libp2p host with iroh transport)
```go
package main

import (
	"context"
	"log"
	"time"

	irohtr "github.com/rustonbsd/go-libp2p-iroh-transport/iroh"
	libp2p "github.com/libp2p/go-libp2p"
	"github.com/libp2p/go-libp2p/core/host"
	"github.com/libp2p/go-libp2p/core/network"
	"github.com/libp2p/go-libp2p/core/peer"
	"github.com/libp2p/go-libp2p/core/transport"
)

func main() {
	// Use an Ed25519 key; iroh requires this.
	// e.g., sk := someEd25519PrivKey

	h, err := libp2p.New(
		// libp2p.Identity(sk),
		libp2p.Transport(func(h host.Host, u transport.Upgrader, r network.ResourceManager) (transport.Transport, error) {
			// NewIrohTransport validates the host key is Ed25519 and bootstraps iroh.
			return irohtr.NewIrohTransport(u, r, h, h.ID())
		}),
	)
	if err != nil {
		log.Fatal(err)
	}

	// Listen anywhere; multiaddr is tracked for upgrader bookkeeping.
	if err := h.Network().Listen("/ip4/0.0.0.0/tcp/0"); err != nil {
		log.Fatal(err)
	}

	// Dial by peer ID (iroh uses identity; the multiaddr address itself is not used for routing).
	remote, _ := peer.Decode("12D3KooW...") // remote Ed25519 peer id
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	if err := h.Connect(ctx, peer.AddrInfo{ID: remote}); err != nil {
		log.Fatal(err)
	}

	// Use libp2p as usual (new streams, protocols, etc.)
	_ = h
}
```

How it works (short version)
- Rust side:
  - Builds a global iroh runtime and transport (one per process)
  - Creates per-host iroh nodes from the host’s Ed25519 private key
  - Maps libp2p Dial/Listen to iroh connect/accept using a small actor model
  - Exposes C ABI: iroh_transport_new, iroh_node_new, iroh_listen, iroh_accept, iroh_dial, iroh_stream_{read,write,close}
- Go side:
  - Implements transport.Transport using the Rust FFI
  - Wraps iroh bidi streams as net.Conn for libp2p upgrader
  - Uses a synthetic local TCP addr for the upgrader’s expectations

Notes and current limits
- Ed25519 only. We verify the private key matches the Peer ID to avoid mismatches.
- Dialing is peer-ID driven; the remote “address” component isn’t used by iroh yet.
- Linux/amd64 staticlib path is set to lib/linux_amd64; add other targets as needed.
- No port reuse. A synthetic ephemeral port (40000–59999) is used internally.
- FFI is intentionally small and may evolve.

License
- MIT (same as the Rust crate)