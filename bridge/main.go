// MessageCraft bridge: a small helper process that talks to Google Messages for web on behalf of
// the MessageCraft window. It pairs with the phone (QR code), keeps the session, syncs
// conversations and messages, and sends. It speaks JSON lines: commands on stdin, events on stdout.
//
// The Google Messages protocol itself is handled by libgm (go.mau.fi/mautrix-gmessages/pkg/libgm,
// AGPL-3.0), used here as a library.
package main

import (
	"bufio"
	"context"
	"encoding/base64"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"mime"
	"net/http"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/google/uuid"
	"github.com/rs/zerolog"
	"go.mau.fi/mautrix-gmessages/pkg/libgm"
	"go.mau.fi/mautrix-gmessages/pkg/libgm/events"
	"go.mau.fi/mautrix-gmessages/pkg/libgm/gmproto"
	"go.mau.fi/util/exhttp"
)

// ---------------------------------------------------------------------------------------------
// Output

var outMu sync.Mutex
var out = bufio.NewWriter(os.Stdout)

func emit(ev string, fields map[string]any) {
	if fields == nil {
		fields = map[string]any{}
	}
	fields["ev"] = ev
	b, err := json.Marshal(fields)
	if err != nil {
		return
	}
	outMu.Lock()
	defer outMu.Unlock()
	out.Write(b)
	out.WriteByte('\n')
	out.Flush()
}

func emitState(state, detail string) {
	emit("state", map[string]any{"state": state, "detail": detail})
}

func emitError(context string, err error) {
	log.Err(err).Str("context", context).Msg("error")
	emit("error", map[string]any{"context": context, "message": friendlyError(err)})
}

func friendlyError(err error) string {
	if err == nil {
		return ""
	}
	s := err.Error()
	switch {
	case errors.Is(err, context.DeadlineExceeded):
		return "Your phone didn't answer in time. Check that it's online and try again."
	case strings.Contains(s, "not logged in"), strings.Contains(s, "client is nil"):
		return "MessageCraft isn't paired with your phone yet."
	}
	return s
}

// ---------------------------------------------------------------------------------------------
// State

type bridge struct {
	dataDir  string
	cacheDir string

	mu       sync.Mutex
	client   *libgm.Client
	auth     *libgm.AuthData
	convs    map[string]*gmproto.Conversation
	sims     map[string]*gmproto.SIMCard
	pairing  *pairing
	ctx      context.Context
	stop     context.CancelFunc
	lastSync time.Time
}

type pairing struct {
	client *libgm.Client
	cancel context.CancelFunc
	done   chan struct{}
}

var log zerolog.Logger

func main() {
	dataDir := flag.String("data", "", "folder for the session and cache")
	flag.Parse()
	if *dataDir == "" {
		base, err := os.UserConfigDir()
		if err != nil {
			base = "."
		}
		*dataDir = filepath.Join(base, "MessageCraft")
	}
	_ = os.MkdirAll(*dataDir, 0o700)
	cacheDir := filepath.Join(*dataDir, "media")
	_ = os.MkdirAll(cacheDir, 0o700)

	logFile, err := os.OpenFile(filepath.Join(*dataDir, "bridge.log"), os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0o600)
	if err != nil {
		log = zerolog.New(io.Discard)
	} else {
		defer logFile.Close()
		log = zerolog.New(logFile).With().Timestamp().Logger().Level(zerolog.InfoLevel)
	}

	ctx, cancel := context.WithCancel(context.Background())
	b := &bridge{
		dataDir:  *dataDir,
		cacheDir: cacheDir,
		convs:    map[string]*gmproto.Conversation{},
		sims:     map[string]*gmproto.SIMCard{},
		ctx:      ctx,
		stop:     cancel,
	}
	emit("hello", map[string]any{"version": "1.2.0", "data_dir": *dataDir})

	scanner := bufio.NewScanner(os.Stdin)
	scanner.Buffer(make([]byte, 64*1024), 16*1024*1024)
	for scanner.Scan() {
		line := scanner.Bytes()
		if len(strings.TrimSpace(string(line))) == 0 {
			continue
		}
		var cmd map[string]any
		if err := json.Unmarshal(line, &cmd); err != nil {
			emit("error", map[string]any{"context": "command", "message": "bad command: " + err.Error()})
			continue
		}
		b.handle(cmd)
	}
	// The window closed its end: shut down cleanly.
	b.mu.Lock()
	if b.client != nil {
		b.client.Disconnect()
	}
	b.mu.Unlock()
	cancel()
}

func str(m map[string]any, k string) string {
	if v, ok := m[k].(string); ok {
		return v
	}
	return ""
}

func num(m map[string]any, k string, def int64) int64 {
	if v, ok := m[k].(float64); ok {
		return int64(v)
	}
	return def
}

func (b *bridge) handle(cmd map[string]any) {
	switch str(cmd, "cmd") {
	case "connect":
		go b.connect()
	case "reconnect":
		go b.reconnect()
	case "pair":
		go b.startPairing()
	case "pair_google":
		cookies := map[string]string{}
		if m, ok := cmd["cookies"].(map[string]any); ok {
			for k, v := range m {
				if s, ok := v.(string); ok && s != "" {
					cookies[k] = s
				}
			}
		}
		go b.startGooglePairing(cookies)
	case "cancel_pair":
		b.cancelPairing()
	case "sync":
		go b.syncConversations(str(cmd, "folder"), num(cmd, "count", 100))
	case "messages":
		var cursor *gmproto.Cursor
		if id := str(cmd, "before_id"); id != "" {
			cursor = &gmproto.Cursor{LastItemID: id, LastItemTimestamp: num(cmd, "before_ts", 0)}
		}
		go b.fetchMessages(str(cmd, "conversation_id"), num(cmd, "count", 40), cursor)
	case "send":
		var files []string
		if arr, ok := cmd["files"].([]any); ok {
			for _, f := range arr {
				if s, ok := f.(string); ok {
					files = append(files, s)
				}
			}
		}
		go b.send(str(cmd, "conversation_id"), str(cmd, "text"), str(cmd, "tmp_id"), str(cmd, "reply_to"), files)
	case "mark_read":
		go b.markRead(str(cmd, "conversation_id"), str(cmd, "message_id"))
	case "new_chat":
		var numbers []string
		if arr, ok := cmd["numbers"].([]any); ok {
			for _, n := range arr {
				if s, ok := n.(string); ok && strings.TrimSpace(s) != "" {
					numbers = append(numbers, strings.TrimSpace(s))
				}
			}
		}
		go b.newChat(numbers, str(cmd, "group_name"))
	case "media":
		go b.downloadMedia(str(cmd, "media_id"), str(cmd, "key"), str(cmd, "mime"))
	case "delete_message":
		go b.deleteMessage(str(cmd, "message_id"))
	case "typing":
		go b.typing(str(cmd, "conversation_id"))
	case "unpair":
		go b.unpair()
	case "ping":
		emit("pong", nil)
	default:
		emit("error", map[string]any{"context": "command", "message": "unknown command " + str(cmd, "cmd")})
	}
}

// ---------------------------------------------------------------------------------------------
// Session

func (b *bridge) sessionPath() string { return filepath.Join(b.dataDir, "session.bin") }

func (b *bridge) loadSession() (*libgm.AuthData, error) {
	raw, err := os.ReadFile(b.sessionPath())
	if err != nil {
		return nil, err
	}
	plain, err := unprotect(raw)
	if err != nil {
		return nil, fmt.Errorf("couldn't unlock the saved session: %w", err)
	}
	auth := libgm.NewAuthData()
	if err := json.Unmarshal(plain, auth); err != nil {
		return nil, err
	}
	return auth, nil
}

func (b *bridge) saveSession(auth *libgm.AuthData) {
	if auth == nil {
		return
	}
	plain, err := json.Marshal(auth)
	if err != nil {
		log.Err(err).Msg("marshal session")
		return
	}
	enc, err := protect(plain)
	if err != nil {
		log.Err(err).Msg("protect session")
		return
	}
	tmp := b.sessionPath() + ".tmp"
	if err := os.WriteFile(tmp, enc, 0o600); err != nil {
		log.Err(err).Msg("write session")
		return
	}
	_ = os.Rename(tmp, b.sessionPath())
}

func (b *bridge) newClient(auth *libgm.AuthData) *libgm.Client {
	cli := libgm.NewClient(auth, nil, log.With().Str("component", "libgm").Logger(), exhttp.SensibleClientSettings)
	cli.SetEventHandler(b.onEvent)
	return cli
}

func (b *bridge) connect() {
	b.mu.Lock()
	if b.client != nil && b.client.IsConnected() {
		b.mu.Unlock()
		emitState("connected", "")
		return
	}
	b.mu.Unlock()

	auth, err := b.loadSession()
	if err != nil || auth == nil || (auth.TachyonAuthToken == nil && !auth.HasCookies()) {
		emitState("no_session", "")
		return
	}
	emitState("connecting", "")
	cli := b.newClient(auth)
	b.mu.Lock()
	b.client = cli
	b.auth = auth
	b.mu.Unlock()
	if err := cli.Connect(b.ctx); err != nil {
		log.Err(err).Msg("connect failed")
		if pairingLost(err) {
			b.forgetPairing("Your phone no longer recognises this computer. Pair again to keep using MessageCraft.")
			return
		}
		emitState("error", friendlyError(err))
		return
	}
	b.saveSession(auth)
	emitState("connected", "")
	if auth.Mobile != nil {
		emit("phone", map[string]any{"id": auth.Mobile.GetSourceID()})
	}
	b.syncConversations("inbox", 100)
}

// reconnect makes this window the active Google Messages session again (after another browser
// took over, or after an error), or connects from scratch when there is no client yet.
func (b *bridge) reconnect() {
	b.mu.Lock()
	cli := b.client
	b.mu.Unlock()
	if cli == nil {
		b.connect()
		return
	}
	emitState("connecting", "")
	if err := cli.Reconnect(b.ctx); err != nil {
		emitState("error", friendlyError(err))
		log.Err(err).Msg("reconnect failed")
		return
	}
	emitState("connected", "")
	b.syncConversations("inbox", 100)
}

func (b *bridge) startPairing() {
	b.cancelPairing()
	emitState("pairing", "")
	auth := libgm.NewAuthData()
	cli := libgm.NewClient(auth, nil, log.With().Str("component", "libgm-pair").Logger(), exhttp.SensibleClientSettings)
	cli.SetEventHandler(func(evt any) {
		log.Debug().Type("type", evt).Msg("pre-pair event")
	})
	ctx, cancel := context.WithCancel(b.ctx)
	p := &pairing{client: cli, cancel: cancel, done: make(chan struct{})}
	success := make(chan *gmproto.PairedData, 1)
	cb := func(data *gmproto.PairedData) {
		select {
		case success <- data:
		default:
		}
	}
	cli.PairCallback.Store(&cb)
	b.mu.Lock()
	b.pairing = p
	b.mu.Unlock()

	if err := cli.FetchConfig(ctx); err != nil {
		cancel()
		emitError("pair", fmt.Errorf("couldn't reach Google Messages: %w", err))
		emitState("no_session", "")
		return
	}
	qr, err := cli.StartLogin(ctx)
	if err != nil {
		cancel()
		emitError("pair", fmt.Errorf("couldn't start pairing: %w", err))
		emitState("no_session", "")
		return
	}
	emit("qr", map[string]any{"url": qr, "expires_in": 30})

	attempts := 8
	timer := time.NewTimer(30 * time.Second)
	defer timer.Stop()
	for {
		select {
		case data := <-success:
			log.Info().Str("phone", data.GetMobile().GetSourceID()).Msg("paired")
			b.saveSession(cli.AuthData)
			emit("paired", map[string]any{"phone": data.GetMobile().GetSourceID()})
			cli.Disconnect()
			cancel()
			b.mu.Lock()
			if b.pairing == p {
				b.pairing = nil
			}
			b.mu.Unlock()
			// Let the phone store the pairing before the first real connection.
			time.Sleep(2 * time.Second)
			b.connect()
			return
		case <-timer.C:
			attempts--
			if attempts <= 0 {
				cli.Disconnect()
				cancel()
				emit("qr_expired", nil)
				emitState("no_session", "")
				return
			}
			newQR, err := cli.RefreshPhoneRelay()
			if err != nil {
				cli.Disconnect()
				cancel()
				emitError("pair", fmt.Errorf("couldn't refresh the QR code: %w", err))
				emitState("no_session", "")
				return
			}
			emit("qr", map[string]any{"url": newQR, "expires_in": 30})
			timer.Reset(30 * time.Second)
		case <-ctx.Done():
			cli.Disconnect()
			return
		}
	}
}

// startGooglePairing pairs the way current Google Messages does: with the Google account the phone
// uses. The window signs in and hands over the session cookies; Google finds the phone, which
// shows a choice of emoji; the person taps the one shown here.
func (b *bridge) startGooglePairing(cookies map[string]string) {
	b.cancelPairing()
	emitState("pairing", "")
	if len(cookies) == 0 {
		emit("pair_failed", map[string]any{"message": "The Google sign-in didn't finish. Try signing in again."})
		emitState("no_session", "")
		return
	}
	auth := libgm.NewAuthData()
	auth.SetCookies(cookies)
	cli := libgm.NewClient(auth, nil, log.With().Str("component", "libgm-gaia").Logger(), exhttp.SensibleClientSettings)
	cli.SetEventHandler(func(evt any) {
		log.Debug().Type("type", evt).Msg("pre-pair event")
	})
	ctx, cancel := context.WithCancel(b.ctx)
	p := &pairing{client: cli, cancel: cancel, done: make(chan struct{})}
	b.mu.Lock()
	b.pairing = p
	b.mu.Unlock()
	defer func() {
		b.mu.Lock()
		if b.pairing == p {
			b.pairing = nil
		}
		b.mu.Unlock()
	}()
	fail := func(msg string, err error) {
		log.Err(err).Msg("google pairing failed")
		cli.Disconnect()
		cancel()
		emit("pair_failed", map[string]any{"message": msg})
		emitState("no_session", "")
	}

	if err := cli.FetchConfig(ctx); err != nil {
		fail("Google didn't accept the sign-in. Try signing in again.", err)
		return
	}
	email := cli.Config.GetDeviceInfo().GetEmail()
	emit("google_account", map[string]any{"email": email})
	startCtx, startCancel := context.WithTimeout(ctx, 90*time.Second)
	emoji, sess, err := cli.StartGaiaPairing(startCtx, ctx)
	startCancel()
	if err != nil {
		switch {
		case errors.Is(err, libgm.ErrNoDevicesFound):
			fail("Google couldn't find a phone using Google Messages with this account. On your phone, open Google Messages › Device pairing and check it's signed in to the same Google account.", err)
		case errors.Is(err, libgm.ErrPhoneNotFound), errors.Is(err, libgm.ErrPhoneInactive):
			fail("Your phone hasn't been online recently. Open Google Messages on your phone, then try again.", err)
		case errors.Is(err, libgm.ErrPairingInitTimeout):
			fail("Your phone didn't answer. Open Google Messages on your phone, then try again.", err)
		case pairingLost(err) || strings.Contains(err.Error(), "401"):
			fail("Google didn't accept the sign-in. Click Sign in with Google and sign in again.", err)
		default:
			fail("Pairing couldn't start: "+friendlyError(err), err)
		}
		return
	}
	emit("emoji", map[string]any{"emoji": emoji, "image_url": libgm.GetEmojiSVG(emoji)})
	go b.fetchEmojiImage(emoji)

	phoneID, err := cli.FinishGaiaPairing(ctx, sess)
	if err != nil {
		switch {
		case errors.Is(err, libgm.ErrIncorrectEmoji):
			fail("A different emoji was tapped on your phone. Try again and tap the one MessageCraft shows.", err)
		case errors.Is(err, libgm.ErrPairingCancelled):
			fail("Pairing was cancelled on your phone.", err)
		case errors.Is(err, libgm.ErrPairingTimeout):
			fail("Your phone wasn't confirmed in time. Try again.", err)
		case errors.Is(err, context.Canceled):
			cli.Disconnect()
			emitState("no_session", "")
		default:
			fail("Pairing didn't finish: "+friendlyError(err), err)
		}
		return
	}
	log.Info().Str("phone", phoneID).Msg("paired with google account")
	b.saveSession(cli.AuthData)
	label := email
	if label == "" {
		label = phoneID
	}
	emit("paired", map[string]any{"phone": label})
	cli.Disconnect()
	cancel()
	b.connect()
}

// fetchEmojiImage saves the colour picture of the pairing emoji (Google's Noto emoji) for the window.
func (b *bridge) fetchEmojiImage(emoji string) {
	url := strings.Replace(libgm.GetEmojiSVG(emoji), "/emoji.svg", "/512.png", 1)
	ctx, cancel := context.WithTimeout(b.ctx, 20*time.Second)
	defer cancel()
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return
	}
	data, err := io.ReadAll(io.LimitReader(resp.Body, 4<<20))
	if err != nil {
		return
	}
	path := filepath.Join(b.cacheDir, "pair-emoji.png")
	if err := os.WriteFile(path, data, 0o600); err != nil {
		return
	}
	emit("emoji_image", map[string]any{"emoji": emoji, "path": path})
}

func (b *bridge) cancelPairing() {
	b.mu.Lock()
	p := b.pairing
	b.pairing = nil
	b.mu.Unlock()
	if p != nil {
		p.cancel()
	}
}

func (b *bridge) unpair() {
	b.mu.Lock()
	cli := b.client
	b.client = nil
	b.convs = map[string]*gmproto.Conversation{}
	b.mu.Unlock()
	if cli != nil {
		if err := cli.Unpair(b.ctx); err != nil {
			log.Err(err).Msg("unpair")
		}
		cli.Disconnect()
	}
	_ = os.Remove(b.sessionPath())
	emit("unpaired", nil)
	emitState("no_session", "")
}

// forgetPairing drops a pairing the phone or Google no longer accepts, so a new one can be made.
func (b *bridge) forgetPairing(reason string) {
	b.mu.Lock()
	cli := b.client
	b.client = nil
	b.convs = map[string]*gmproto.Conversation{}
	b.mu.Unlock()
	if cli != nil {
		go cli.Disconnect()
	}
	_ = os.Remove(b.sessionPath())
	emit("unpaired", map[string]any{"reason": reason})
	emitState("no_session", "")
}

// pairingLost: the error says the saved pairing's credentials are no longer valid (as libgm
// treats a fatal token refresh).
func pairingLost(err error) bool {
	if err == nil {
		return false
	}
	if errors.Is(err, events.ErrInvalidCredentials) || errors.Is(err, events.ErrRequestedEntityNotFound) {
		return true
	}
	var httpErr events.HTTPError
	if errors.As(err, &httpErr) && httpErr.Resp != nil {
		switch httpErr.Resp.StatusCode {
		case 401, 403, 404:
			return true
		}
	}
	return false
}

func (b *bridge) cli() (*libgm.Client, error) {
	b.mu.Lock()
	defer b.mu.Unlock()
	if b.client == nil {
		return nil, errors.New("not logged in")
	}
	return b.client, nil
}

// ---------------------------------------------------------------------------------------------
// Events from the phone

func (b *bridge) onEvent(rawEvt any) {
	switch evt := rawEvt.(type) {
	case *gmproto.Conversation:
		b.mu.Lock()
		b.convs[evt.GetConversationID()] = evt
		b.mu.Unlock()
		emit("conversation", map[string]any{"item": convJSON(evt)})
	case *libgm.WrappedMessage:
		emit("message", map[string]any{"item": msgJSON(evt.Message), "old": evt.IsOld})
	case *gmproto.TypingData:
		emit("typing", map[string]any{
			"conversation_id": evt.GetConversationID(),
			"number":          evt.GetUser().GetNumber(),
			"typing":          evt.GetType() == gmproto.TypingTypes_STARTED_TYPING,
		})
	case *gmproto.Settings:
		b.mu.Lock()
		for _, sim := range evt.GetSIMCards() {
			b.sims[sim.GetSIMParticipant().GetID()] = sim
		}
		var sims []map[string]any
		for id, sim := range b.sims {
			sims = append(sims, map[string]any{"participant_id": id, "carrier": sim.GetSIMData().GetCarrierName(), "number": sim.GetSIMData().GetFormattedPhoneNumber()})
		}
		b.mu.Unlock()
		emit("sims", map[string]any{"items": sims})
	case *gmproto.UserAlertEvent:
		switch evt.GetAlertType() {
		case gmproto.AlertType_BROWSER_INACTIVE, gmproto.AlertType_BROWSER_INACTIVE_FROM_TIMEOUT, gmproto.AlertType_BROWSER_INACTIVE_FROM_INACTIVITY:
			emitState("other_session", "Google Messages is open in another window or browser.")
		case gmproto.AlertType_BROWSER_ACTIVE:
			emitState("connected", "")
			b.mu.Lock()
			stale := time.Since(b.lastSync) > 20*time.Second
			b.mu.Unlock()
			if stale {
				go b.syncConversations("inbox", 100)
			}
		case gmproto.AlertType_MOBILE_BATTERY_LOW:
			emit("phone_alert", map[string]any{"text": "Your phone's battery is low."})
		case gmproto.AlertType_MOBILE_DATABASE_SYNC_COMPLETE:
			go b.syncConversations("inbox", 100)
		}
	case *events.PhoneNotResponding:
		emitState("phone_offline", "Your phone isn't responding. Check that it's on and connected.")
	case *events.PhoneRespondingAgain:
		emitState("connected", "")
	case *events.ListenTemporaryError:
		emitState("reconnecting", friendlyError(evt.Error))
	case *events.ListenRecovered:
		emitState("connected", "")
		go b.syncConversations("inbox", 100)
	case *events.ListenFatalError:
		if pairingLost(evt.Error) {
			b.forgetPairing("Your phone no longer recognises this computer. Pair again to keep using MessageCraft.")
			return
		}
		emitState("error", friendlyError(evt.Error))
	case *events.GaiaLoggedOut:
		b.forgetPairing("Google signed this computer out of Google Messages. Pair again to keep using MessageCraft.")
	case *gmproto.RevokePairData:
		log.Info().Msg("pairing revoked by phone")
		b.forgetPairing("Your phone removed this computer from its paired devices.")
	case *events.AuthTokenRefreshed:
		b.mu.Lock()
		auth := b.auth
		b.mu.Unlock()
		b.saveSession(auth)
	case *events.NoDataReceived:
		go b.syncConversations("inbox", 100)
	default:
		log.Debug().Type("type", rawEvt).Msg("unhandled event")
	}
}

// ---------------------------------------------------------------------------------------------
// Requests

func (b *bridge) syncConversations(folder string, count int64) {
	cli, err := b.cli()
	if err != nil {
		return
	}
	f := gmproto.ListConversationsRequest_INBOX
	switch folder {
	case "archive":
		f = gmproto.ListConversationsRequest_ARCHIVE
	case "spam":
		f = gmproto.ListConversationsRequest_SPAM_BLOCKED
	default:
		folder = "inbox"
	}
	emit("syncing", map[string]any{"folder": folder})
	ctx, cancel := context.WithTimeout(b.ctx, 60*time.Second)
	defer cancel()
	resp, err := cli.ListConversations(ctx, &gmproto.ListConversationsRequest{Count: count, Folder: f})
	if err != nil {
		emitError("sync", err)
		emit("synced", map[string]any{"folder": folder, "ok": false})
		return
	}
	items := make([]map[string]any, 0, len(resp.GetConversations()))
	b.mu.Lock()
	for _, c := range resp.GetConversations() {
		b.convs[c.GetConversationID()] = c
		items = append(items, convJSON(c))
	}
	if folder == "inbox" {
		b.lastSync = time.Now()
	}
	b.mu.Unlock()
	emit("conversations", map[string]any{"folder": folder, "items": items})
	emit("synced", map[string]any{"folder": folder, "ok": true, "at": time.Now().UnixMilli()})
}

func (b *bridge) fetchMessages(convID string, count int64, cursor *gmproto.Cursor) {
	cli, err := b.cli()
	if err != nil {
		emitError("messages", err)
		return
	}
	ctx, cancel := context.WithTimeout(b.ctx, 45*time.Second)
	defer cancel()
	resp, err := cli.FetchMessages(ctx, convID, count, cursor)
	if err != nil {
		emitError("messages", err)
		emit("messages", map[string]any{"conversation_id": convID, "items": []any{}, "failed": true, "older": cursor != nil})
		return
	}
	msgs := resp.GetMessages()
	sort.Slice(msgs, func(i, j int) bool { return msgs[i].GetTimestamp() < msgs[j].GetTimestamp() })
	items := make([]map[string]any, 0, len(msgs))
	for _, m := range msgs {
		items = append(items, msgJSON(m))
	}
	ev := map[string]any{
		"conversation_id": convID,
		"items":           items,
		"older":           cursor != nil,
		"has_more":        resp.GetCursor() != nil && resp.GetCursor().GetLastItemID() != "",
	}
	if c := resp.GetCursor(); c != nil {
		ev["cursor_id"] = c.GetLastItemID()
		ev["cursor_ts"] = c.GetLastItemTimestamp()
	}
	emit("messages", ev)
}

func (b *bridge) conv(ctx context.Context, cli *libgm.Client, convID string) (*gmproto.Conversation, error) {
	b.mu.Lock()
	c := b.convs[convID]
	b.mu.Unlock()
	if c != nil {
		return c, nil
	}
	c, err := cli.GetConversation(ctx, convID)
	if err != nil {
		return nil, err
	}
	b.mu.Lock()
	b.convs[convID] = c
	b.mu.Unlock()
	return c, nil
}

func (b *bridge) send(convID, text, tmpID, replyTo string, files []string) {
	fail := func(err error) {
		log.Err(err).Msg("send failed")
		emit("send_result", map[string]any{"tmp_id": tmpID, "conversation_id": convID, "ok": false, "error": friendlyError(err)})
	}
	cli, err := b.cli()
	if err != nil {
		fail(err)
		return
	}
	if tmpID == "" {
		tmpID = "tmp_" + uuid.NewString()[:12]
	}
	ctx, cancel := context.WithTimeout(b.ctx, 120*time.Second)
	defer cancel()
	c, err := b.conv(ctx, cli, convID)
	if err != nil {
		fail(err)
		return
	}
	b.mu.Lock()
	sim := b.sims[c.GetDefaultOutgoingID()]
	b.mu.Unlock()

	var infos []*gmproto.MessageInfo
	for _, path := range files {
		data, err := os.ReadFile(path)
		if err != nil {
			fail(fmt.Errorf("couldn't read %s: %w", filepath.Base(path), err))
			return
		}
		mt := mime.TypeByExtension(strings.ToLower(filepath.Ext(path)))
		if mt == "" {
			mt = "application/octet-stream"
		}
		if i := strings.Index(mt, ";"); i > 0 {
			mt = mt[:i]
		}
		media, err := cli.UploadMedia(ctx, data, filepath.Base(path), mt)
		if err != nil {
			fail(fmt.Errorf("couldn't upload %s: %w", filepath.Base(path), err))
			return
		}
		infos = append(infos, &gmproto.MessageInfo{Data: &gmproto.MessageInfo_MediaContent{MediaContent: media}})
	}
	if strings.TrimSpace(text) != "" {
		infos = append(infos, &gmproto.MessageInfo{Data: &gmproto.MessageInfo_MessageContent{MessageContent: &gmproto.MessageContent{Content: text}}})
	}
	if len(infos) == 0 {
		fail(errors.New("there's nothing to send"))
		return
	}
	req := &gmproto.SendMessageRequest{
		ConversationID: convID,
		MessagePayload: &gmproto.MessagePayload{
			TmpID:          tmpID,
			ConversationID: convID,
			ParticipantID:  c.GetDefaultOutgoingID(),
			TmpID2:         tmpID,
			MessageInfo:    infos,
		},
		SIMPayload: sim.GetSIMData().GetSIMPayload(),
		TmpID:      tmpID,
	}
	if replyTo != "" {
		req.Reply = &gmproto.ReplyPayload{MessageID: replyTo}
	}
	resp, err := cli.SendMessage(ctx, req)
	if err != nil {
		fail(err)
		return
	}
	if resp.GetStatus() != gmproto.SendMessageResponse_SUCCESS {
		reason := "Your phone couldn't send this message."
		if resp.GetStatus() == gmproto.SendMessageResponse_FAILURE_4 {
			reason = "Your phone couldn't send this message. Is Google Messages your phone's default messaging app?"
		}
		emit("send_result", map[string]any{"tmp_id": tmpID, "conversation_id": convID, "ok": false, "error": reason})
		return
	}
	emit("send_result", map[string]any{"tmp_id": tmpID, "conversation_id": convID, "ok": true})
}

func (b *bridge) markRead(convID, msgID string) {
	cli, err := b.cli()
	if err != nil || convID == "" || msgID == "" {
		return
	}
	ctx, cancel := context.WithTimeout(b.ctx, 30*time.Second)
	defer cancel()
	if err := cli.MarkRead(ctx, convID, msgID); err != nil {
		log.Err(err).Msg("mark read")
	}
}

func (b *bridge) typing(convID string) {
	cli, err := b.cli()
	if err != nil || convID == "" {
		return
	}
	ctx, cancel := context.WithTimeout(b.ctx, 15*time.Second)
	defer cancel()
	b.mu.Lock()
	var simPayload *gmproto.SIMPayload
	if c := b.convs[convID]; c != nil {
		simPayload = b.sims[c.GetDefaultOutgoingID()].GetSIMData().GetSIMPayload()
	}
	b.mu.Unlock()
	_ = cli.SetTyping(ctx, convID, simPayload)
}

func (b *bridge) newChat(numbers []string, groupName string) {
	cli, err := b.cli()
	if err != nil {
		emitError("new_chat", err)
		return
	}
	if len(numbers) == 0 {
		emit("error", map[string]any{"context": "new_chat", "message": "Type a phone number first."})
		return
	}
	req := &gmproto.GetOrCreateConversationRequest{}
	for _, n := range numbers {
		req.Numbers = append(req.Numbers, &gmproto.ContactNumber{MysteriousInt: 2, Number: n, Number2: n})
	}
	if len(numbers) > 1 && groupName != "" {
		req.RCSGroupName = &groupName
	}
	ctx, cancel := context.WithTimeout(b.ctx, 45*time.Second)
	defer cancel()
	resp, err := cli.GetOrCreateConversation(ctx, req)
	if err == nil && resp.GetStatus() == gmproto.GetOrCreateConversationResponse_CREATE_RCS && len(numbers) > 1 {
		t := true
		req.CreateRCSGroup = &t
		if req.RCSGroupName == nil {
			name := ""
			req.RCSGroupName = &name
		}
		resp, err = cli.GetOrCreateConversation(ctx, req)
	}
	if err != nil {
		emitError("new_chat", err)
		return
	}
	c := resp.GetConversation()
	if c == nil {
		emit("error", map[string]any{"context": "new_chat", "message": "Your phone couldn't start that conversation."})
		return
	}
	b.mu.Lock()
	b.convs[c.GetConversationID()] = c
	b.mu.Unlock()
	emit("conversation", map[string]any{"item": convJSON(c), "opened": true})
}

func (b *bridge) deleteMessage(msgID string) {
	cli, err := b.cli()
	if err != nil {
		emitError("delete", err)
		return
	}
	ctx, cancel := context.WithTimeout(b.ctx, 30*time.Second)
	defer cancel()
	if _, err := cli.DeleteMessage(ctx, msgID); err != nil {
		emitError("delete", err)
		return
	}
	emit("deleted", map[string]any{"message_id": msgID})
}

func (b *bridge) downloadMedia(mediaID, keyB64, mimeType string) {
	fail := func(err error) {
		log.Err(err).Str("media", mediaID).Msg("media download")
		emit("media", map[string]any{"media_id": mediaID, "ok": false, "error": friendlyError(err)})
	}
	if mediaID == "" {
		fail(errors.New("no media id"))
		return
	}
	ext := ".bin"
	if exts, _ := mime.ExtensionsByType(mimeType); len(exts) > 0 {
		ext = exts[len(exts)-1]
	}
	switch mimeType {
	case "image/jpeg":
		ext = ".jpg"
	case "image/png":
		ext = ".png"
	case "image/gif":
		ext = ".gif"
	case "image/webp":
		ext = ".webp"
	}
	name := safeName(mediaID) + ext
	path := filepath.Join(b.cacheDir, name)
	if st, err := os.Stat(path); err == nil && st.Size() > 0 {
		emit("media", map[string]any{"media_id": mediaID, "ok": true, "path": path})
		return
	}
	cli, err := b.cli()
	if err != nil {
		fail(err)
		return
	}
	key, err := base64.StdEncoding.DecodeString(keyB64)
	if err != nil {
		fail(err)
		return
	}
	ctx, cancel := context.WithTimeout(b.ctx, 120*time.Second)
	defer cancel()
	rc, err := cli.DownloadMedia(ctx, mediaID, key)
	if err != nil {
		fail(err)
		return
	}
	defer rc.Close()
	tmp := path + ".part"
	f, err := os.Create(tmp)
	if err != nil {
		fail(err)
		return
	}
	_, err = io.Copy(f, rc)
	f.Close()
	if err != nil {
		_ = os.Remove(tmp)
		fail(err)
		return
	}
	_ = os.Rename(tmp, path)
	emit("media", map[string]any{"media_id": mediaID, "ok": true, "path": path})
}

func safeName(s string) string {
	var sb strings.Builder
	for _, r := range s {
		if (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') || (r >= '0' && r <= '9') || r == '-' || r == '_' {
			sb.WriteRune(r)
		} else {
			sb.WriteRune('_')
		}
	}
	out := sb.String()
	if len(out) > 120 {
		out = out[:120]
	}
	return out
}

// ---------------------------------------------------------------------------------------------
// Shapes sent to the window

func convJSON(c *gmproto.Conversation) map[string]any {
	parts := make([]map[string]any, 0, len(c.GetParticipants()))
	for _, p := range c.GetParticipants() {
		parts = append(parts, map[string]any{
			"id":     p.GetID().GetParticipantID(),
			"name":   p.GetFullName(),
			"first":  p.GetFirstName(),
			"number": firstNonEmpty(p.GetFormattedNumber(), p.GetID().GetNumber()),
			"is_me":  p.GetIsMe(),
			"color":  p.GetAvatarHexColor(),
		})
	}
	folder := "inbox"
	switch c.GetStatus() {
	case gmproto.ConversationStatus_ARCHIVED, gmproto.ConversationStatus_KEEP_ARCHIVED:
		folder = "archive"
	case gmproto.ConversationStatus_SPAM_FOLDER, gmproto.ConversationStatus_BLOCKED_FOLDER:
		folder = "spam"
	case gmproto.ConversationStatus_DELETED, gmproto.ConversationStatus_TRASH_FOLDER:
		folder = "deleted"
	}
	kind := "sms"
	if c.GetType() == gmproto.ConversationType_RCS {
		kind = "rcs"
	}
	lm := c.GetLatestMessage()
	return map[string]any{
		"id":            c.GetConversationID(),
		"name":          c.GetName(),
		"snippet":       lm.GetDisplayContent(),
		"snippet_me":    lm.GetFromMe() != 0,
		"snippet_from":  lm.GetDisplayName(),
		"ts":            c.GetLastMessageTimestamp() / 1000,
		"unread":        c.GetUnread(),
		"group":         c.GetIsGroupChat(),
		"pinned":        c.GetPinned(),
		"read_only":     c.GetReadOnly(),
		"color":         c.GetAvatarHexColor(),
		"folder":        folder,
		"kind":          kind,
		"outgoing_id":   c.GetDefaultOutgoingID(),
		"latest_msg_id": c.GetLatestMessageID(),
		"participants":  parts,
	}
}

func firstNonEmpty(s ...string) string {
	for _, v := range s {
		if v != "" {
			return v
		}
	}
	return ""
}

func msgJSON(m *gmproto.Message) map[string]any {
	status := m.GetMessageStatus().GetStatus()
	fromMe := status > 0 && status < 100
	var text []string
	var media []map[string]any
	for _, info := range m.GetMessageInfo() {
		switch d := info.GetData().(type) {
		case *gmproto.MessageInfo_MessageContent:
			if t := d.MessageContent.GetContent(); t != "" {
				text = append(text, t)
			}
		case *gmproto.MessageInfo_MediaContent:
			media = append(media, mediaJSON(d.MediaContent))
		case *gmproto.MessageInfo_LocationContent:
			if t := d.LocationContent.GetMessageContent().GetContent(); t != "" {
				text = append(text, t)
			}
			if mc := d.LocationContent.GetMediaContent(); mc != nil {
				media = append(media, mediaJSON(mc))
			}
		}
	}
	var reactions []map[string]any
	for _, r := range m.GetReactions() {
		emoji := r.GetData().GetUnicode()
		if emoji == "" {
			emoji = r.GetData().GetType().Unicode()
		}
		reactions = append(reactions, map[string]any{"emoji": emoji, "count": len(r.GetParticipantIDs())})
	}
	statusText := m.GetMessageStatus().GetStatusText()
	failed := false
	switch status {
	case gmproto.MessageStatusType_OUTGOING_FAILED_GENERIC, gmproto.MessageStatusType_OUTGOING_FAILED_EMERGENCY_NUMBER,
		gmproto.MessageStatusType_OUTGOING_FAILED_TOO_LARGE, gmproto.MessageStatusType_OUTGOING_FAILED_RECIPIENT_LOST_RCS,
		gmproto.MessageStatusType_OUTGOING_FAILED_NO_RETRY_NO_FALLBACK, gmproto.MessageStatusType_OUTGOING_FAILED_RECIPIENT_DID_NOT_DECRYPT,
		gmproto.MessageStatusType_OUTGOING_FAILED_RECIPIENT_LOST_ENCRYPTION, gmproto.MessageStatusType_OUTGOING_FAILED_RECIPIENT_DID_NOT_DECRYPT_NO_MORE_RETRY,
		gmproto.MessageStatusType_OUTGOING_FAILED_RECIPIENT_NEGATIVE_DELIVERY, gmproto.MessageStatusType_OUTGOING_FAILED_TO_ENCRYPT,
		gmproto.MessageStatusType_OUTGOING_CANCELED:
		failed = true
	}
	deleted := status == gmproto.MessageStatusType_MESSAGE_DELETED || status == gmproto.MessageStatusType_OUTGOING_DELETED || status == gmproto.MessageStatusType_INCOMING_DELETED
	out := map[string]any{
		"id":              m.GetMessageID(),
		"conversation_id": m.GetConversationID(),
		"participant_id":  m.GetParticipantID(),
		"ts":              m.GetTimestamp() / 1000,
		"from_me":         fromMe,
		"text":            strings.Join(text, "\n"),
		"status":          statusName(status),
		"status_text":     statusText,
		"failed":          failed,
		"deleted":         deleted,
		"tmp_id":          m.GetTmpID(),
		"media":           media,
		"reactions":       reactions,
		"subject":         m.GetSubject(),
	}
	if sp := m.GetSenderParticipant(); sp != nil {
		out["sender_name"] = firstNonEmpty(sp.GetFullName(), sp.GetFirstName(), sp.GetFormattedNumber())
	}
	if r := m.GetReplyMessage(); r != nil {
		out["reply_to"] = r.GetMessageID()
	}
	return out
}

func mediaJSON(mc *gmproto.MediaContent) map[string]any {
	return map[string]any{
		"id":        mc.GetMediaID(),
		"key":       base64.StdEncoding.EncodeToString(mc.GetDecryptionKey()),
		"thumb_id":  mc.GetThumbnailMediaID(),
		"thumb_key": base64.StdEncoding.EncodeToString(mc.GetThumbnailDecryptionKey()),
		"name":      mc.GetMediaName(),
		"mime":      mc.GetMimeType(),
		"size":      mc.GetSize(),
		"w":         mc.GetDimensions().GetWidth(),
		"h":         mc.GetDimensions().GetHeight(),
	}
}

func statusName(s gmproto.MessageStatusType) string {
	switch s {
	case gmproto.MessageStatusType_OUTGOING_YET_TO_SEND, gmproto.MessageStatusType_OUTGOING_SENDING,
		gmproto.MessageStatusType_OUTGOING_RESENDING, gmproto.MessageStatusType_OUTGOING_AWAITING_RETRY,
		gmproto.MessageStatusType_OUTGOING_SEND_AFTER_PROCESSING, gmproto.MessageStatusType_OUTGOING_VALIDATING:
		return "sending"
	case gmproto.MessageStatusType_OUTGOING_COMPLETE:
		return "sent"
	case gmproto.MessageStatusType_OUTGOING_DELIVERED:
		return "delivered"
	case gmproto.MessageStatusType_OUTGOING_DISPLAYED:
		return "read"
	case gmproto.MessageStatusType_OUTGOING_SCHEDULED:
		return "scheduled"
	case gmproto.MessageStatusType_OUTGOING_NOT_DELIVERED_YET:
		return "not_delivered"
	case gmproto.MessageStatusType_INCOMING_YET_TO_MANUAL_DOWNLOAD, gmproto.MessageStatusType_INCOMING_AUTO_DOWNLOADING,
		gmproto.MessageStatusType_INCOMING_MANUAL_DOWNLOADING, gmproto.MessageStatusType_INCOMING_RETRYING_AUTO_DOWNLOAD,
		gmproto.MessageStatusType_INCOMING_RETRYING_MANUAL_DOWNLOAD, gmproto.MessageStatusType_INCOMING_AWAITING_AUTO_DOWNLOAD:
		return "downloading"
	case gmproto.MessageStatusType_INCOMING_DOWNLOAD_FAILED, gmproto.MessageStatusType_INCOMING_EXPIRED_OR_NOT_AVAILABLE,
		gmproto.MessageStatusType_INCOMING_DOWNLOAD_FAILED_TOO_LARGE, gmproto.MessageStatusType_INCOMING_DOWNLOAD_FAILED_SIM_HAS_NO_DATA,
		gmproto.MessageStatusType_INCOMING_FAILED_TO_DECRYPT:
		return "download_failed"
	}
	if s >= 100 {
		return "received"
	}
	return "unknown"
}
