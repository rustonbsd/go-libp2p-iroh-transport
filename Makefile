# Start with Linux targets only
RUST_TARGETS := \
	x86_64-unknown-linux-gnu \
	aarch64-unknown-linux-gnu

LIBS_DIR := ffi/libs
INCLUDE_DIR := ffi/include

all: install-tools install-targets build-rust-linux

.PHONY: install-targets
install-targets:
	@echo "Installing Rust targets..."
	@for target in $(RUST_TARGETS); do \
		rustup target add $$target; \
	done

.PHONY: install-tools
install-tools:
	@echo "Installing cross-compilation tools..."
	sudo apt-get update
	sudo apt-get install -y gcc-aarch64-linux-gnu

.PHONY: build-rust-linux
build-rust-linux: 
	@mkdir -p $(LIBS_DIR)
	@cd rust && \
	export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc && \
	export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc && \
	for target in $(RUST_TARGETS); do \
		echo "Building for $$target..."; \
		cargo build --release --target $$target; \
		cp target/$$target/release/libirohffi.so ../$(LIBS_DIR)/libiroh_$$(echo $$target | tr '-' '_').so; \
	done && \
	mkdir -p ../$(INCLUDE_DIR) && \
	cp target/include/libirohffi.h ../$(INCLUDE_DIR)/libirohffi.h

.PHONY: test
test:
	@go test .
	@cd rust && \
	cargo test