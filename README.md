# go-libp2p-iroh-transport

A tiny libp2p Transport that routes all traffic over iroh (QUIC) via a small Rust FFI. Built for plugging iroh into libp2p and DefraDB.

## Befor any adoption TODOs
- [x] Make working go package with ffi bindings
- [x] create two libp2p nodes with iroh transport only and have them connect successfully
- [x] Centeralize all ffi bindings in cgo_wrapper.go and only import wrapped handles in conn and transport (fine for this prototype for now)
- [ ] fix listener closed bug (implement propper listener close)
- [ ] add docker local isolated network integration test (in defradb) (proof new capability vs vanilla defradb in cold bootstrapping)
- [ ] add cicd pipeline
- [ ] benchmark
- [ ] 3rd party code review
- [ ] professionalize go package (README, docs, examples, etc.)
- [ ] publish to go module proxy

## Next up run TODO's
- [ ] add more targets and platforms (aarch64, musl, windows, macos, etc.)
- [ ] find a better way then to wrap the Rust<>C<(>Go<>C<)>Go handles in go again
- [ ] refactor with [kameo](https://github.com/tqwewe/kameo/tree/main) (tiny actor framework) and benchmark
- [ ] research more go ffi package best practices (see research secion)

## Tests
```bash
> go test .
ok      github.com/rustonbsd/go-libp2p-iroh-transport   2.166s
```
## Install

```bash
go get github.com/rustonbsd/go-libp2p-iroh-transport@v0.0.2
```

## Build

Build the rust .so and header (required for cgo):
```bash
make build-rust-linux
```

(note: only linux supported at this moment)

Artifacts:
- ffi/include/libirohffi.h
- ffi/lib/linux_amd64/libiroh_aarch64_unknown_linux_gnu.so
- ffi/lib/linux_arm64/libiroh_x86_64_unknown_linux_gnu.so



## Research

- rust compile to wasm no static or dyn lib:
  - https://blog.arcjet.com/calling-rust-ffi-libraries-from-go/
  - https://blog.arcjet.com/webassembly-on-the-server-compiling-rust-to-wasm-and-executing-it-from-go/

- dynamic embed in go from this guide used:
  - https://github.com/mediremi/rust-plus-golang/tree/master

- look into [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild) for cross-compilation (maybe help rust land in the default compile chain of defradb :) )