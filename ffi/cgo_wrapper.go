//go:build cgo

package ffi

/*
#cgo CFLAGS: -I${SRCDIR}/include
#cgo linux,amd64 LDFLAGS: ${SRCDIR}/libs/libiroh_x86_64_unknown_linux_gnu.so -ldl -lpthread -lm
#cgo linux,arm64 LDFLAGS: ${SRCDIR}/libs/libiroh_aarch64_unknown_linux_gnu.so -ldl -lpthread -lm
#include "libirohffi.h"
*/
import "C"

import (
	"os"
	"path/filepath"
	"runtime"
	"sync"

	_ "embed"
)

//go:embed libs/libiroh_x86_64_unknown_linux_gnu.so
var libAmd64 []byte

//go:embed libs/libiroh_aarch64_unknown_linux_gnu.so
var libArm64 []byte

var (
	extractOnce sync.Once
	libPath     string
)

func Init() {
	extractOnce.Do(func() {
		var libData []byte
		var filename string

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
}
