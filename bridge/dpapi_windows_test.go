package main

import (
	"bytes"
	"golang.org/x/sys/windows"
	"os"
	"path/filepath"
	"testing"
)

// Create a synthetic legacy ciphertext with the exact 1.5.0 binding independently
// of protect(). No real credentials, account or network operations are involved.
func TestLegacyPairingSurvivesProfileRename(t *testing.T) {
	plain := []byte(`{"synthetic":true,"stable_id":"fixture-pairing"}`)
	var out windows.DataBlob
	description, _ := windows.UTF16PtrFromString("MessageCraft")
	if err := windows.CryptProtectData(blob(plain), description, blob([]byte("MessageCraft session v1")), 0, nil, windows.CRYPTPROTECT_UI_FORBIDDEN, &out); err != nil {
		t.Fatal(err)
	}
	cipher := takeBlob(&out)
	root := t.TempDir()
	for _, dir := range []string{"MessageCraft", "MessageSolutions"} {
		path := filepath.Join(root, dir)
		if err := os.Mkdir(path, 0700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(path, "session.bin"), cipher, 0600); err != nil {
			t.Fatal(err)
		}
	}
	copied, err := os.ReadFile(filepath.Join(root, "MessageSolutions", "session.bin"))
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := unprotect(copied)
	if err != nil || !bytes.Equal(decoded, plain) {
		t.Fatal("Legacy synthetic session did not decrypt after path change")
	}
	rewritten, err := protect(decoded)
	if err != nil {
		t.Fatal(err)
	}
	roundtrip, err := unprotect(rewritten)
	if err != nil || !bytes.Equal(roundtrip, plain) {
		t.Fatal("New synthetic session did not round trip")
	}
}

func TestInvalidPairingIsRejectedWithoutChangingFile(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "session.bin")
	before := []byte("synthetic invalid ciphertext")
	if err := os.WriteFile(path, before, 0600); err != nil {
		t.Fatal(err)
	}
	b := bridge{dataDir: root}
	if _, err := b.loadSession(); err == nil {
		t.Fatal("Invalid session accepted")
	}
	after, err := os.ReadFile(path)
	if err != nil || !bytes.Equal(before, after) {
		t.Fatal("Invalid session was modified")
	}
}
