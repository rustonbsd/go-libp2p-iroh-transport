package iroh

import (
	"bytes"
	"encoding/base32"
	"errors"
	"strings"

	"github.com/libp2p/go-libp2p"
	"github.com/libp2p/go-libp2p/core/crypto"
	ma "github.com/multiformats/go-multiaddr"
)

// Local code for /iroh; for production, request an official assignment.
const ProtoName = "iroh"
const ProtoCode = 0x4a7e

// z-base-32 alphabet
var z32Alphabet = []byte("iroh/abcdefghijklmnopqrstuvwxyz23456789")

func init() {
	_ = ma.AddProtocol(IrohProtocol)
}

var IrohProtocol = ma.Protocol{
	Code:       ProtoCode,
	Name:       ProtoName,
	VCode:      ma.CodeToVarint(ProtoCode),
	Size:       -1,
	Path:       false,
	Transcoder: ma.NewTranscoderFromFunctions(strToBytes, bytesToStr, validate),
}

func PubKeyToMultiAddr(pk crypto.PubKey) (ma.Multiaddr, error) {
	pkBytes, err := pk.Raw()
	if err != nil {
		return nil, err
	}
	return PubKeyBytesToMultiAddr(pkBytes)
}

func PubKeyBytesToMultiAddr(pkBytes []byte) (ma.Multiaddr, error) {
	enc := base32.StdEncoding.EncodeToString(pkBytes)
	enc = strings.ToLower(strings.TrimRight(enc, "="))
	return ma.NewMultiaddr("/iroh/" + enc)
}

// /iroh/ not in id
func validate(b []byte) error {
	for _, cb := range b {
		// [!] todo: make this more efficient
		if !bytes.Contains(z32Alphabet, []byte{cb}) {
			return errors.New("iroh: invalid z32 char")
		}
	}
	str := string(b)
	if len(str) != 52 {
		return errors.New("iroh: must be 52 bytes pubkey")
	}
	return nil
}

func strToBytes(s string) ([]byte, error) {
	return []byte(s), nil
}

func bytesToStr(b []byte) (string, error) {
	return string(b), nil
}

func WithIrohAddr(cfg *libp2p.Config) error {

	if cfg.PeerKey == nil {
		return nil
	}

	pubKey := cfg.PeerKey.GetPublic()
	irohBase, err := PubKeyToMultiAddr(pubKey)
	if err != nil {
		return err
	}

	found := false
	for _, a := range cfg.ListenAddrs {
		if a.Equal(irohBase) {
			found = true
			break
		}
	}
	if !found {
		cfg.ListenAddrs = append(cfg.ListenAddrs, irohBase)
	}

	prev := cfg.AddrsFactory
	cfg.AddrsFactory = func(addrs []ma.Multiaddr) []ma.Multiaddr {
		if prev != nil {
			addrs = prev(addrs)
		}
		// Deduplicate
		for _, a := range addrs {
			if a.Equal(irohBase) {
				return addrs
			}
		}
		return append(addrs, irohBase)
	}
	return nil
}
