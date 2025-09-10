.PHONY: all build-rust build-go

.DEFAULT_GOAL := build

CARGO := cargo
CBINDGEN := cbindgen
CBINDGEN_CFG := cbindgen.toml
CRATE := irohffi
OUT_H := libirohffi.h
DEST_INCLUDE := ../ffi/include
DEST_LIB := ../ffi/lib/linux_amd64
TARGET_LIB := target/release/libirohffi.a
GO_BUILD_FLAGS=""
RUST_BUILD_FLAGS ?= -C link-arg=-Wl,--gc-sections -C link-arg=-Wl,--icf=all
all: build-rust build-go

.PHONY: build-rust
build-rust:
	@cd rust && \
	RUST_BUILD_FLAGS="$(RUST_BUILD_FLAGS)" $(CARGO) build --release && \
	$(CBINDGEN) --config $(CBINDGEN_CFG) --crate $(CRATE) --output $(OUT_H) && \
	mkdir -p $(DEST_INCLUDE) && \
	cp $(OUT_H) $(DEST_INCLUDE)/$(OUT_H) && \
	mkdir -p $(DEST_LIB) && \
	cp $(TARGET_LIB) $(DEST_LIB)/libirohffi.a

build-go: build-rust
	@go build $(GO_BUILD_FLAGS)