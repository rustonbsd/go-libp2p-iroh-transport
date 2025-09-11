package iroh

import (
	"fmt"
	"net"
	"time"

	ma "github.com/multiformats/go-multiaddr"
	manet "github.com/multiformats/go-multiaddr/net"
	libirohffi "github.com/rustonbsd/go-libp2p-iroh-transport/ffi"
)

type IrohListener struct {
	h   libirohffi.ListenerHandle
	lma ma.Multiaddr
}

func (l IrohListener) Accept() (manet.Conn, error) {
	// Pick a sane timeout; or block forever if you prefer
	h, err := libirohffi.Accept(l.h, 30*time.Second)
	if err != nil {
		return nil, err
	}

	laNet, err := manet.ToNetAddr(l.lma)
	if err != nil {
		return nil, err
	}
	ltcp, ok := laNet.(*net.TCPAddr)
	if !ok {
		return nil, fmt.Errorf("listener local addr not tcp: %T", laNet)
	}

	// Synthesize a local TCP addr (libp2p only needs protocol + IP family)
	// IP zero + port 0 is acceptable; upgrader will embed it into a /ip4/0.0.0.0/tcp/0 multiaddr.
	rm := &net.TCPAddr{
		IP:   net.IPv4zero,
		Port: nextSyntheticPort(),
	}
	// Wrap Rust stream handle as net.Conn
	nc := libirohffi.WrapStream(h, ltcp, rm)

	return manet.WrapNetConn(nc)
}

func (l IrohListener) Close() error {
	return nil
}

func (l IrohListener) Addr() net.Addr {
	na, _ := manet.ToNetAddr(l.lma)
	return na
}

func (l IrohListener) Multiaddr() ma.Multiaddr {
	return l.lma
}
