package iroh

import (
	"net"
	"time"

	"github.com/rustonbsd/go-libp2p-iroh-transport/ffi"

	ma "github.com/multiformats/go-multiaddr"
	manet "github.com/multiformats/go-multiaddr/net"
)

type IrohListener struct {
	h   ffi.ListenerHandle
	lma ma.Multiaddr
}

func (l IrohListener) Accept() (manet.Conn, error) {
	// 30 seconds seems reasonable with the added cancelation token
	h, err := ffi.Accept(l.h, 30*time.Second)
	if err != nil {
		return nil, err
	}

	ltcp := &net.TCPAddr{
		IP:   net.IPv4zero,
		Port: 0,
	}

	rm := &net.TCPAddr{
		IP:   net.IPv4zero,
		Port: nextSyntheticPort(),
	}

	nc := ffi.WrapStream(h, ltcp, rm)
	return manet.WrapNetConn(nc)
}

func (l IrohListener) Close() error {
	return ffi.ListenClose(l.h)
}

func (l IrohListener) Addr() net.Addr {
	na, _ := manet.ToNetAddr(l.lma)
	return na
}

func (l IrohListener) Multiaddr() ma.Multiaddr {
	return l.lma
}
