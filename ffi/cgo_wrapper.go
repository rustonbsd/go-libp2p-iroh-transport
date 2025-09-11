//go:build cgo

package ffi

/*
#cgo CFLAGS: -I${SRCDIR}/include
#cgo LDFLAGS: -ldl -lpthread -lm
#include <stdlib.h>
#include <stdint.h>
#include <stdio.h>
#include <dlfcn.h>
#include "libirohffi.h"

typedef int32_t (*iroh_transport_new_func)(struct IrohTransportHandle*);
typedef int32_t (*iroh_node_new_func)(struct IrohTransportHandle,
                                      const uint8_t*, size_t,
                                      const char*, struct IrohNodeHandle*);
typedef int32_t (*iroh_listen_func)(struct IrohNodeHandle,
                                    const char*, struct IrohListenerHandle*);
typedef int32_t (*iroh_accept_func)(struct IrohListenerHandle,
                                    uint64_t, struct IrohStreamHandle*);
typedef int32_t (*iroh_dial_func)(struct IrohNodeHandle,
                                  const char*, struct IrohStreamHandle*);
typedef int32_t (*iroh_stream_read_func)(struct IrohStreamHandle,
                                         uint8_t*, size_t, uint64_t, ptrdiff_t*);
typedef int32_t (*iroh_stream_write_func)(struct IrohStreamHandle,
                                          const uint8_t*, size_t, uint64_t, ptrdiff_t*);
typedef int32_t (*iroh_stream_close_func)(struct IrohStreamHandle);
typedef int32_t (*iroh_shutdown_func)(void);

iroh_transport_new_func iroh_transport_new_ptr = NULL;
iroh_node_new_func iroh_node_new_ptr = NULL;
iroh_listen_func iroh_listen_ptr = NULL;
iroh_accept_func iroh_accept_ptr = NULL;
iroh_dial_func iroh_dial_ptr = NULL;
iroh_stream_read_func iroh_stream_read_ptr = NULL;
iroh_stream_write_func iroh_stream_write_ptr = NULL;
iroh_stream_close_func iroh_stream_close_ptr = NULL;
iroh_shutdown_func iroh_shutdown_ptr = NULL;

void* lib_handle = NULL;

static int load_symbol(void** target, const char* name) {
    *target = dlsym(lib_handle, name);
    if (!*target) {
        const char* err = dlerror();
        fprintf(stderr, "dlsym failed for %s: %s\n", name, err ? err : "(nil)");
        return -1;
    }
    return 0;
}

int load_library(const char* path) {
    lib_handle = dlopen(path, RTLD_NOW | RTLD_LOCAL);
    if (!lib_handle) {
        fprintf(stderr, "dlopen failed: %s\n", dlerror());
        return -1;
    }
    return 0;
}

int load_library_and_init_symbols(const char* path) {
    if (load_library(path) != 0) return -1;

    if (load_symbol((void**)&iroh_transport_new_ptr, "iroh_transport_new")) return -1;
    if (load_symbol((void**)&iroh_node_new_ptr, "iroh_node_new")) return -1;
    if (load_symbol((void**)&iroh_listen_ptr, "iroh_listen")) return -1;
    if (load_symbol((void**)&iroh_accept_ptr, "iroh_accept")) return -1;
    if (load_symbol((void**)&iroh_dial_ptr, "iroh_dial")) return -1;
    if (load_symbol((void**)&iroh_stream_read_ptr, "iroh_stream_read")) return -1;
    if (load_symbol((void**)&iroh_stream_write_ptr, "iroh_stream_write")) return -1;
    if (load_symbol((void**)&iroh_stream_close_ptr, "iroh_stream_close")) return -1;
    if (load_symbol((void**)&iroh_shutdown_ptr, "iroh_shutdown")) return -1;

    return 0;
}

void* get_symbol(const char* name) {
    if (!lib_handle) return NULL;
    return dlsym(lib_handle, name);
}

int32_t iroh_transport_new(struct IrohTransportHandle *out_handle) {
    if (!iroh_transport_new_ptr) return -1;
    return iroh_transport_new_ptr(out_handle);
}

int32_t iroh_node_new(struct IrohTransportHandle transport,
                      const uint8_t* ed25519_priv_ptr, size_t ed25519_priv_len,
                      const char* peer_id_raw, struct IrohNodeHandle *out_handle) {
    if (!iroh_node_new_ptr) return -1;
    return iroh_node_new_ptr(transport, ed25519_priv_ptr, ed25519_priv_len, peer_id_raw, out_handle);
}

int32_t iroh_listen(struct IrohNodeHandle node, const char* listen_maddr, struct IrohListenerHandle *out_listener) {
    if (!iroh_listen_ptr) return -1;
    return iroh_listen_ptr(node, listen_maddr, out_listener);
}

int32_t iroh_accept(struct IrohListenerHandle listener, uint64_t timeout_ms, struct IrohStreamHandle *out_stream) {
    if (!iroh_accept_ptr) return -1;
    return iroh_accept_ptr(listener, timeout_ms, out_stream);
}

int32_t iroh_dial(struct IrohNodeHandle node, const char* remote_maddr, struct IrohStreamHandle *out_stream) {
    if (!iroh_dial_ptr) return -1;
    return iroh_dial_ptr(node, remote_maddr, out_stream);
}

int32_t iroh_stream_read(struct IrohStreamHandle stream, uint8_t* buf, size_t len, uint64_t timeout_ms, ptrdiff_t* out) {
    if (!iroh_stream_read_ptr) return -1;
    return iroh_stream_read_ptr(stream, buf, len, timeout_ms, out);
}

int32_t iroh_stream_write(struct IrohStreamHandle stream, const uint8_t* buf, size_t len, uint64_t timeout_ms, ptrdiff_t* out) {
    if (!iroh_stream_write_ptr) return -1;
    return iroh_stream_write_ptr(stream, buf, len, timeout_ms, out);
}

int32_t iroh_stream_close(struct IrohStreamHandle stream) {
    if (!iroh_stream_close_ptr) return -1;
    return iroh_stream_close_ptr(stream);
}

int32_t iroh_shutdown() {
	printf("[C] iroh_shutdown called, ptr=%p\n", iroh_shutdown_ptr);
	fflush(stdout);
    if (!iroh_shutdown_ptr) return -1;
    return iroh_shutdown_ptr();
}
*/
import "C"

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"sync"
	"unsafe"

	_ "embed"
)

//go:embed libs/libiroh_x86_64_unknown_linux_gnu.so
var libAmd64 []byte

//go:embed libs/libiroh_aarch64_unknown_linux_gnu.so
var libArm64 []byte

var (
	initOnce    sync.Once
	extractOnce sync.Once
	libPath     string
)

func GetSymbol(name string) unsafe.Pointer {
	cname := C.CString(name)
	defer C.free(unsafe.Pointer(cname))
	return C.get_symbol(cname)
}

func LoadLibrary(path string) error {
	cpath := C.CString(path)
	defer C.free(unsafe.Pointer(cpath))
	if C.load_library(cpath) != 0 {
		return fmt.Errorf("failed to load library: %s", path)
	}
	return nil
}

func LoadLibraryAndInitSymbols(path string) error {
	cpath := C.CString(path)
	defer C.free(unsafe.Pointer(cpath))
	if C.load_library_and_init_symbols(cpath) != 0 {
		return fmt.Errorf("failed to load library and init symbols: %s", path)
	}
	return nil
}

func Init() {
	extractOnce.Do(func() {
		var libData []byte
		var filename string

		fmt.Printf("goruntime: %s", runtime.GOARCH)

		switch runtime.GOARCH {
		case "amd64":
			libData = libAmd64
			filename = "libiroh_x86_64_unknown_linux_gnu.so"
		case "arm64":
			libData = libArm64
			filename = "libiroh_aarch64_unknown_linux_gnu.so"
		default:
			panic("unsupported architecture")
		}

		tmpDir := os.TempDir()
		libPath = filepath.Join(tmpDir, filename)

		if err := os.WriteFile(libPath, libData, 0755); err != nil {
			panic("failed to extract library: " + err.Error())
		}
	})
	initOnce.Do(func() {
		if err := LoadLibraryAndInitSymbols(libPath); err != nil {
			panic(err)
		}
	})
}
