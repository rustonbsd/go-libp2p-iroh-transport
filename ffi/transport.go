package libirohffi

// #cgo CFLAGS: -I${SRCDIR}/include
// #cgo linux,amd64 LDFLAGS: -L${SRCDIR}/lib/linux_amd64 -lirohffi -ldl -lpthread
// #cgo linux,arm64 LDFLAGS: -L${SRCDIR}/lib/linux_arm64 -lirohffi -ldl -lpthread
// #cgo darwin,arm64 LDFLAGS: -L${SRCDIR}/lib/darwin_arm64 -lirohffi
// #cgo darwin,amd64 LDFLAGS: -L${SRCDIR}/lib/darwin_amd64 -lirohffi
// #include "../include/libirohffi.h"
import "C"
import (
	"errors"
	"fmt"
	"time"
	"unsafe"

	"github.com/libp2p/go-libp2p/core/peer"
)

type TransportHandle = C.IrohTransportHandle
type NodeHandle = C.IrohNodeHandle
type ListenerHandle = C.IrohListenerHandle
type StreamHandle = C.IrohStreamHandle

func NewTransport() (TransportHandle, error) {
	var out TransportHandle
	rc := C.iroh_transport_new(&out)
	if rc != 0 {
		return out, fmt.Errorf("iroh_transport_new rc=%d", int(rc))
	}
	return out, nil
}

func NewNode(ed25519Priv []byte, p peer.ID) (NodeHandle, error) {
	var out NodeHandle
	if len(ed25519Priv) != 64 {
		return out, errors.New("invalid ed25519 private key")
	}
	pStr := p.String()
	cs := C.CString(pStr)
	defer C.free(unsafe.Pointer(cs))
	rc := C.iroh_node_new((*C.uint8_t)(&ed25519Priv[0]), C.size_t(len(ed25519Priv)), cs, &out)
	if rc != 0 {
		return out, fmt.Errorf("iroh_node_new rc=%d", int(rc))
	}
	return out, nil
}

func Listen(node NodeHandle, maddr string) (ListenerHandle, error) {
	var h ListenerHandle
	cs := C.CString(maddr)
	defer C.free(unsafe.Pointer(cs))
	rc := C.iroh_listen(node, cs, &h)
	if rc != 0 {
		return h, fmt.Errorf("iroh_listen rc=%d", int(rc))
	}
	return h, nil
}

func Accept(l ListenerHandle, timeout time.Duration) (StreamHandle, error) {
	var s StreamHandle
	rc := C.iroh_accept(l, C.uint64_t(timeout.Milliseconds()), &s)
	if rc != 0 {
		return s, fmt.Errorf("iroh_accept rc=%d", int(rc))
	}
	return s, nil
}

func Dial(node NodeHandle, p peer.ID) (StreamHandle, error) {
	var s StreamHandle
	cs := C.CString(p.String())
	defer C.free(unsafe.Pointer(cs))
	rc := C.iroh_dial(node, cs, &s)
	if rc != 0 {
		return s, fmt.Errorf("iroh_dial rc=%d", int(rc))
	}
	return s, nil
}
