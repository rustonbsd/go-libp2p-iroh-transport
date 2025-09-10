# Iroh integration

Integrates iroh into defradb via ffi bindings. Iroh is then used as a new libp2p transport layer for multiaddr connections.

## Testing
Right now, with the docker compose setup in the main folder (`../`) libp2p is not capable of finding and consequently replicating to that peer with vanilla defradb and no custom configuration.

**Test Success:** if defradb nodeA can connect and subsequently replicate via `[..] p2p connect [..]` and `[..] p2p replicate set -c Artivle [..]` to nodeB. The new transport logs directly to stdout and can be used to validate the iroh integration.

## Setup

```sh
cd defradb/
sudo docker compose build
sudo docker compose up --force-recreate

sudo docker compose exec nodeA defradb client p2p connect 'P2P_INFO_COPIED_FROM_DOCKER_COMPOSE_OUTPUT'

sudo docker compose exec nodeA defradb client p2p replicator set -c Article 'P2P_INFO_COPIED_FROM_DOCKER_COMPOSE_OUTPUT'

# if no errors all test pass
```

## Setup 

```sh
# rustup.rs to get rustc and cargo installed

cargo install cbindgen
```

## Build ffi bindings

To build the Rust<>C<>Go FFI bindings:

```sh
cd defradb/iroh_integration/
cargo build --release

cbindgen --config cbindgen.toml --crate irohffi --output libirohffi.h
mkdir -p ../transport/irohffi/include/
cp libirohffi.h ../transport/irohffi/include/libirohffi.h

# On Linux amd64
mkdir -p ../transport/irohffi/lib/linux_amd64/
cp target/release/libirohffi.a ../transport/irohffi/lib/linux_amd64/libirohffi.a

```