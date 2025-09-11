package iroh

import (
	"context"
	"fmt"
	"testing"
	"time"

	libp2p "github.com/libp2p/go-libp2p"
	"github.com/libp2p/go-libp2p/core/crypto"
	corehost "github.com/libp2p/go-libp2p/core/host"
	"github.com/libp2p/go-libp2p/core/network"
	"github.com/libp2p/go-libp2p/core/peer"
	transport2 "github.com/libp2p/go-libp2p/core/transport"
	ma "github.com/multiformats/go-multiaddr"
	"github.com/rustonbsd/go-libp2p-iroh-transport/ffi"
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
	pA := nextSyntheticPort()
	pB := nextSyntheticPort()
	listenA, _ := ma.NewMultiaddr(
		fmt.Sprintf("/ip4/127.0.0.1/tcp/%d", pA),
	)
	listenB, _ := ma.NewMultiaddr(
		fmt.Sprintf("/ip4/127.0.0.1/tcp/%d", pB),
	)

	hA := buildTestHost(t, listenA)
	hB := buildTestHost(t, listenB)

	defer hA.Close()
	defer hB.Close()

	time.Sleep(2 * time.Second)
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
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
		{
			t.Fatalf("connect timed out: %v", ctx.Err())
		}
	}

	// Check connections after successful connect
	fmt.Printf("Connection counts: B->A=%d, A->B=%d\n",
		len(hB.Network().ConnsToPeer(hA.ID())),
		len(hA.Network().ConnsToPeer(hB.ID())))

	if len(hB.Network().ConnsToPeer(hA.ID())) == 0 || len(hA.Network().ConnsToPeer(hB.ID())) == 0 {
		t.Fatalf("expected connection entries on both peers")
	}

	fmt.Printf("Test passed - connections established!\n")

	ffi.Shutdown()
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
