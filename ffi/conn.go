package libirohffi

// #cgo CFLAGS: -I${SRCDIR}/include
// #cgo linux,amd64 LDFLAGS: -L${SRCDIR}/lib/linux_amd64 -lirohffi -ldl -lpthread
// #cgo linux,arm64 LDFLAGS: -L${SRCDIR}/lib/linux_arm64 -lirohffi -ldl -lpthread
// #cgo darwin,arm64 LDFLAGS: -L${SRCDIR}/lib/darwin_arm64 -lirohffi
// #cgo darwin,amd64 LDFLAGS: -L${SRCDIR}/lib/darwin_amd64 -lirohffi
// #include "../include/libirohffi.h"
import "C"
import (
	"fmt"
	"io"
	"net"
	"sync"
	"time"
)

type irohConn struct {
	h          StreamHandle
	ra         net.Addr
	la         net.Addr
	rd         time.Time
	wd         time.Time
	rdMu, wdMu sync.Mutex
}

func (c *irohConn) Read(p []byte) (int, error) {
	if len(p) == 0 {
		return 0, nil
	}
	var out C.ptrdiff_t
	to := c.readTimeoutMs()
	rc := C.iroh_stream_read(c.h, (*C.uint8_t)(&p[0]), C.size_t(len(p)), C.uint64_t(to), &out)
	if rc != 0 {
		return int(out), fmt.Errorf("iroh_stream_read rc=%d", int(rc))
	}
	n := int(out)
	if n < 0 {
		return 0, io.EOF
	}
	return n, nil
}
func (c *irohConn) Write(p []byte) (int, error) {
	if len(p) == 0 {
		return 0, nil
	}
	var out C.ptrdiff_t
	to := c.writeTimeoutMs()
	rc := C.iroh_stream_write(c.h, (*C.uint8_t)(&p[0]), C.size_t(len(p)), C.uint64_t(to), &out)
	if rc != 0 {
		return int(out), fmt.Errorf("iroh_stream_write rc=%d", int(rc))
	}
	return int(out), nil
}

func (c *irohConn) Close() error {
	rc := C.iroh_stream_close(c.h)
	if rc != 0 {
		fmt.Printf("iroh_stream_close rc=%d\n", int(rc))
		return fmt.Errorf("iroh_stream_close rc=%d", int(rc))
	}
	return nil
}

func (c *irohConn) LocalAddr() net.Addr  { return c.la }
func (c *irohConn) RemoteAddr() net.Addr { return c.ra }
func (c *irohConn) SetReadDeadline(t time.Time) error {
	c.rdMu.Lock()
	c.rd = t
	c.rdMu.Unlock()
	return nil
}
func (c *irohConn) SetWriteDeadline(t time.Time) error {
	c.wdMu.Lock()
	c.wd = t
	c.wdMu.Unlock()
	return nil
}
func (c *irohConn) SetDeadline(t time.Time) error {
	c.SetReadDeadline(t)
	c.SetWriteDeadline(t)
	return nil
}

func (c *irohConn) readTimeoutMs() uint64 {
	c.rdMu.Lock()
	defer c.rdMu.Unlock()
	if c.rd.IsZero() {
		return 0
	}
	d := time.Until(c.rd)
	if d <= 0 {
		return 1
	}
	return uint64(d.Milliseconds())
}
func (c *irohConn) writeTimeoutMs() uint64 {
	c.wdMu.Lock()
	defer c.wdMu.Unlock()
	if c.wd.IsZero() {
		return 0
	}
	d := time.Until(c.wd)
	if d <= 0 {
		return 1
	}
	return uint64(d.Milliseconds())
}
func WrapStream(h StreamHandle, la, ra net.Addr) net.Conn {
	return &irohConn{h: h, la: la, ra: ra}
}
