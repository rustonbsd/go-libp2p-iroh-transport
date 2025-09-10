//go:build cgo

package libirohffi

/*
#cgo CFLAGS: -I${SRCDIR}/../include
#cgo linux,amd64 LDFLAGS: -L${SRCDIR}/../lib/linux_amd64 -lirohffi -ldl -lpthread -lm
#cgo linux,arm64 LDFLAGS: -L${SRCDIR}/../lib/linux_arm64 -lirohffi -ldl -lpthread -lm
#cgo darwin,arm64 LDFLAGS: -L${SRCDIR}/../lib/darwin_arm64 -lirohffi -lm
#cgo darwin,amd64 LDFLAGS: -L${SRCDIR}/../lib/darwin_amd64 -lirohffi -lm
#include "../include/libirohffi.h"
*/
import "C"
