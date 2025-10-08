# Start with Linux targets only
RUST_TARGETS := \
	x86_64-unknown-linux-gnu \
	aarch64-unknown-linux-gnu

LIBS_DIR := ffi/libs
INCLUDE_DIR := ffi/include

build: install-tools install-targets build-rust-linux

.PHONY: install-targets
install-targets:
	@for target in $(RUST_TARGETS); do \
		rustup target add $$target >/dev/null 2>&1; \
	done

.PHONY: install-tools
install-tools:
	@sudo apt-get update >/dev/null && \
	sudo apt-get install -y gcc-aarch64-linux-gnu >/dev/null >/dev/null 2>&1

.PHONY: build-rust-linux
build-rust-linux: 
	@mkdir -p $(LIBS_DIR)
	@cd rust && \
	export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc && \
	export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc && \
	for target in $(RUST_TARGETS); do \
		cargo build --release --target $$target >/dev/null 2>&1; \
		cp target/$$target/release/libirohffi.so ../$(LIBS_DIR)/libiroh_$$(echo $$target | tr '-' '_').so; \
	done && \
	mkdir -p ../$(INCLUDE_DIR) && \
	cp target/include/libirohffi.h ../$(INCLUDE_DIR)/libirohffi.h

.PHONY: test
test:
	go test .
	cd rust && \
	cargo test