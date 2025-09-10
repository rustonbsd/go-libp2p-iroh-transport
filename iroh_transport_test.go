package iroh

import (
	"context"
	"fmt"
	"os"
	"testing"
	"time"

	libp2p "github.com/libp2p/go-libp2p"
	"github.com/libp2p/go-libp2p/core/crypto"
	corehost "github.com/libp2p/go-libp2p/core/host"
	"github.com/libp2p/go-libp2p/core/network"
	"github.com/libp2p/go-libp2p/core/peer"
	transport2 "github.com/libp2p/go-libp2p/core/transport"
	ma "github.com/multiformats/go-multiaddr"
)

// buildTestHost creates a libp2p host that ONLY uses the iroh transport.
func buildTestHost(t *testing.T, listen ma.Multiaddr) corehost.Host {
	t.Helper()

	// Custom transport constructor passed into libp2p stack.
	ctor := func(upgrader transport2.Upgrader, rcmgr network.ResourceManager, h corehost.Host) (transport2.Transport, error) {
		return NewIrohTransport(upgrader, rcmgr, h, h.ID())
	}

	// Explicit Ed25519 identity to satisfy transport ed25519 requirement.
	sk, pk, err := crypto.GenerateEd25519Key(nil)
	if err != nil {
		t.Fatalf("ed25519 key gen failed: %v", err)
	}
	_ = pk // silence unused (host creation validates internally)

	// Disable all default transports so only ours is active.
	opts := []libp2p.Option{
		libp2p.NoTransports,
		libp2p.Transport(ctor),
		libp2p.ListenAddrs(listen),
		libp2p.Identity(sk),
	}

	h, err := libp2p.New(opts...)
	if err != nil {
		t.Fatalf("failed to build host: %v", err)
	}
	return h
}

// TestIrohTransportPeerConnection spins up two libp2p hosts using only the
// iroh transport and ensures they can connect & then closes them.
func TestIrohTransportPeerConnection(t *testing.T) {

	// Use dynamic synthetic ports (not actually bound by iroh yet, but keeps intent clear)
	pA := nextSyntheticPort()
	pB := nextSyntheticPort()
	listenA, _ := ma.NewMultiaddr(
		fmt.Sprintf("/ip4/127.0.0.1/tcp/%d", pA),
	)
	listenB, _ := ma.NewMultiaddr(
		fmt.Sprintf("/ip4/127.0.0.1/tcp/%d", pB),
	)

	hA := buildTestHost(t, listenA)
	defer hA.Close()
	hB := buildTestHost(t, listenB)
	defer hB.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	ai := peer.AddrInfo{ID: hA.ID(), Addrs: hA.Addrs()}
	errCh := make(chan error, 1)
	go func() { errCh <- hB.Connect(ctx, ai) }()

	select {
	case err := <-errCh:
		if err != nil {
			t.Fatalf("connect failed: %v", err)
		}
	case <-ctx.Done():
		t.Fatalf("connect timed out: %v", ctx.Err())
	}

	if len(hB.Network().ConnsToPeer(hA.ID())) == 0 || len(hA.Network().ConnsToPeer(hB.ID())) == 0 {
		t.Fatalf("expected connection entries on both peers")
	}

	// Validate we can construct proper /p2p/ multiaddr for A.
	if len(hA.Addrs()) == 0 {
		t.Fatalf("no listen addrs for host A")
	}
	full := hA.Addrs()[0].Encapsulate(ma.StringCast("/p2p/" + hA.ID().String()))
	if _, err := peer.AddrInfoFromP2pAddr(full); err != nil {
		t.Fatalf("failed to parse p2p multiaddr for A: %v", err)
	}
}

// TestEd25519AndP2PAddr ensures host identity uses ed25519 and p2p multiaddr parses.
func TestEd25519AndP2PAddr(t *testing.T) {
	if os.Getenv("IROH_ENABLE_IDENTITY") == "" {
		t.Skip("set IROH_ENABLE_IDENTITY=1 to run identity + p2p addr test")
	}
	listen, _ := ma.NewMultiaddr("/ip4/127.0.0.1/tcp/40123")
	h := buildTestHost(t, listen)
	defer h.Close()
	pk := h.Peerstore().PrivKey(h.ID())
	if pk == nil || pk.Type() != crypto.Ed25519 {
		t.Fatalf("expected ed25519 private key")
	}
	// Compose full p2p address and parse back
	if len(h.Addrs()) == 0 {
		t.Fatalf("expected at least one listen addr")
	}
	full := h.Addrs()[0].Encapsulate(ma.StringCast("/p2p/" + h.ID().String()))
	if _, err := peer.AddrInfoFromP2pAddr(full); err != nil {
		t.Fatalf("failed to parse constructed p2p addr: %v", err)
	}
}

// TestCanDialBasic ensures CanDial accepts TCP/UDP IP multiaddrs and rejects others.
func TestCanDialBasic(t *testing.T) {
	tr := &IrohTransport{}
	goodAddrs := []string{
		"/ip4/127.0.0.1/tcp/1234",
	}
	badAddrs := []string{
		"/ip4/127.0.0.1/udp/5678", // udp currently not supported by CanDial
		"/ip6/::1/tcp/9999",       // ipv6 not yet whitelisted
		"/dns4/example.com/tcp/80",
		"/unix/tmp/socket",
	}
	for _, s := range goodAddrs {
		m, _ := ma.NewMultiaddr(s)
		if !tr.CanDial(m) {
			t.Fatalf("expected CanDial true for %s", s)
		}
	}
	for _, s := range badAddrs {
		m, _ := ma.NewMultiaddr(s)
		if tr.CanDial(m) {
			t.Fatalf("expected CanDial false for %s", s)
		}
	}
}

// TestSyntheticPortRange ensures synthetic ports fall within expected range and change.
func TestSyntheticPortRange(t *testing.T) {
	p1 := nextSyntheticPort()
	p2 := nextSyntheticPort()
	if p1 == p2 {
		t.Fatalf("expected different synthetic ports got %d and %d", p1, p2)
	}
	if p1 < 40000 || p1 >= 60000 || p2 < 40000 || p2 >= 60000 {
		t.Fatalf("ports out of expected range: %d %d", p1, p2)
	}
}
