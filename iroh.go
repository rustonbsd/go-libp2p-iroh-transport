package iroh

import (
	"context"
	"errors"
	"fmt"
	"go-libp2p-iroh-transport/ffi"
	"net"
	"sync/atomic"
	"time"

	"github.com/libp2p/go-libp2p/core/crypto/pb"
	"github.com/libp2p/go-libp2p/core/host"
	"github.com/libp2p/go-libp2p/core/network"
	"github.com/libp2p/go-libp2p/core/peer"
	"github.com/libp2p/go-libp2p/core/transport"

	logging "github.com/ipfs/go-log/v2"
	ma "github.com/multiformats/go-multiaddr"
	mafmt "github.com/multiformats/go-multiaddr-fmt"
	manet "github.com/multiformats/go-multiaddr/net"
)

var _log = logging.Logger("iroh-transport")

type Option func(*IrohTransport) error

// [!] todo: remove this and use a proper port management system
// or maybe libp2p transport can be configured to not need port at all?
// [!] todo: more libp2p transport research!
var synthPortCounter atomic.Uint32

func nextSyntheticPort() int {
	n := synthPortCounter.Add(1)
	return 40000 + int(n%20000) // always >=40000 and <60000
}

type ContextDialer interface {
	DialContext(ctx context.Context, network, address string) (net.Conn, error)
}

type IrohTransport struct {
	// Connection upgrader for upgrading insecure stream connections to
	// secure multiplex connections.
	upgrader transport.Upgrader

	// Iroh connect timeout
	connectTimeout time.Duration

	rcmgr network.ResourceManager

	handle ffi.TransportHandle
	node   ffi.NodeHandle

	localAddr *net.TCPAddr
}

var _ transport.Transport = &IrohTransport{}
var _ transport.DialUpdater = &IrohTransport{}

// NewIrohTransport creates a iroh transport object that tracks dialers and listeners
// created.
func NewIrohTransport(upgrader transport.Upgrader, rcmgr network.ResourceManager, h host.Host, p peer.ID, opts ...Option) (*IrohTransport, error) {
	if rcmgr == nil {
		rcmgr = &network.NullResourceManager{}
	}
	transportHandle, err := ffi.NewTransport()
	if err != nil {
		return nil, err
	}
	privKey := h.Peerstore().PrivKey(h.ID())
	if privKey == nil {
		return nil, errors.New("host has no private key")
	}
	if privKey.Type() != pb.KeyType_Ed25519 {
		return nil, errors.New("host private key is not ed25519 wich is required for iroh")
	}
	privKeyBytes, err := privKey.Raw()
	if err != nil {
		return nil, fmt.Errorf("failed to get raw private key: %w", err)
	}

	node, err := ffi.NewNode(transportHandle, privKeyBytes, p)
	if err != nil {
		return nil, err
	}

	connectTimeout := time.Duration(30 * time.Second)
	tr := &IrohTransport{
		upgrader:       upgrader,
		connectTimeout: connectTimeout, // can be set by using the WithConnectionTimeout option
		rcmgr:          rcmgr,
		handle:         transportHandle,
		node:           node,
	}

	// [!] todo: make propper use of libp2p multiaddr etc. (research needed)
	tr.localAddr = &net.TCPAddr{
		IP:   net.IPv4zero,
		Port: nextSyntheticPort(), // stable per node, not per dial
	}
	for _, o := range opts {
		if err := o(tr); err != nil {
			return nil, err
		}
	}
	return tr, nil
}

// CanDial returns true if this transport believes it can dial the given
// multiaddr.
// IrohTransport is only based on the peerId so in theory
// (or at least my very limeted tests so far)
// it could dial TCP, UDP, nothing else tested yet.
// [!] todo: test all defradb available transports and see if it works univerally on all platforms (so far only tested on linux/amd64)
func (t *IrohTransport) CanDial(addr ma.Multiaddr) bool {
	return mafmt.And(mafmt.Base(ma.P_IP4), mafmt.Base(ma.P_TCP)).Matches(addr)
}

func (t *IrohTransport) maDial(ctx context.Context, raddr ma.Multiaddr, p peer.ID) (manet.Conn, error) {

	if t.connectTimeout > 0 {
		var cancel context.CancelFunc
		_, cancel = context.WithTimeout(ctx, t.connectTimeout)
		defer cancel()
	}

	h, err := ffi.Dial(t.node, p)
	if err != nil {
		return nil, err
	}

	// Remote net.Addr derived from multiaddr (must be *net.TCPAddr for /tcp)
	rna, err := manet.ToNetAddr(raddr)
	if err != nil {
		return nil, fmt.Errorf("convert remote addr: %w", err)
	}
	/*
		rtcp, ok := rna.(*net.TCPAddr)
		if !ok {
			return nil, fmt.Errorf("remote addr not tcp: %T", rna)
		}
	*/

	// [!] todo: clean this up after some more research
	// Synthesize a local TCP addr (libp2p only needs protocol + IP family)
	// IP zero + port 0 is acceptable; upgrader will embed it into a /ip4/0.0.0.0/tcp/0 multiaddr.
	la := t.localAddr

	nc := ffi.WrapStream(h, la, rna)
	return manet.WrapNetConn(nc)
}

// Dial dials the peer at the remote address.
func (t *IrohTransport) Dial(ctx context.Context, raddr ma.Multiaddr, p peer.ID) (transport.CapableConn, error) {
	if p.Size() == 0 {
		return nil, errors.New("no peer ID specified")
	}
	return t.DialWithUpdates(ctx, raddr, p, nil)
}

func (t *IrohTransport) DialWithUpdates(ctx context.Context, raddr ma.Multiaddr, p peer.ID, updateChan chan<- transport.DialUpdate) (transport.CapableConn, error) {
	connScope, err := t.rcmgr.OpenConnection(network.DirOutbound, true, raddr)
	if err != nil {
		_log.Debugw("resource manager blocked outgoing connection", "peer", p, "addr", raddr, "error", err)
		return nil, err
	}

	c, err := t.dialWithScope(ctx, raddr, p, connScope, updateChan)
	if err != nil {
		connScope.Done()
		return nil, err
	}
	return c, nil
}

func (t *IrohTransport) dialWithScope(ctx context.Context, raddr ma.Multiaddr, p peer.ID, connScope network.ConnManagementScope, updateChan chan<- transport.DialUpdate) (transport.CapableConn, error) {
	if err := connScope.SetPeer(p); err != nil {
		_log.Debugw("resource manager blocked outgoing connection for peer", "peer", p, "addr", raddr, "error", err)
		return nil, err
	}
	conn, err := t.maDial(ctx, raddr, p)
	if err != nil {
		return nil, err
	}

	c := conn
	if updateChan != nil {
		select {
		case updateChan <- transport.DialUpdate{Kind: transport.UpdateKindHandshakeProgressed, Addr: raddr}:
		default:
			// It is better to skip the update than to delay upgrading the connection
		}
	}
	direction := network.DirOutbound
	if ok, isClient, _ := network.GetSimultaneousConnect(ctx); ok && !isClient {
		direction = network.DirInbound
	}
	return t.upgrader.Upgrade(ctx, t, c, direction, p, connScope)
}

// [!] todo: UseReuseport returns true if reuseport is enabled and available.
// for now we just do false (no port reuse)
func (t *IrohTransport) UseReuseport() bool {
	return false
}

func (t *IrohTransport) Listen(laddr ma.Multiaddr) (transport.Listener, error) {
	h, err := ffi.Listen(t.node, laddr.String())
	if err != nil {
		return nil, err
	}

	listen := IrohListener{h: h, lma: laddr}

	// Upgrade the manet listener (connection gating handled by libp2p stack if configured).
	maListener := manet.Listener(listen)
	return t.upgrader.UpgradeListener(t, maListener), nil
}

// Protocols returns the list of terminal protocols this transport can dial.
// [!] todo: adjust protocols [blocked]: refactor with mutiaddress for iroh
func (t *IrohTransport) Protocols() []int {
	return []int{ma.P_TCP}
}

// [!] todo: is this comment still right for iroh? it is a proxy sooo?:
// "Proxy always returns false for the TCP transport.""
func (t *IrohTransport) Proxy() bool {
	return false
}

func (t *IrohTransport) String() string {
	return "IROH"
}

func WithIrohNode(h ffi.NodeHandle) Option {
	return func(tr *IrohTransport) error {
		tr.node = h
		return nil
	}
}
