# MessageSolutions

**Your phone's text messages on Windows, in the style of [MailSolutions](https://github.com/william-pyle/MailSolutions).**

MessageSolutions is a desktop app for reading and sending the SMS, MMS and RCS chat messages on your
Android phone from a Windows PC. It pairs with Google Messages on your phone the same way Google
Messages for web does (by signing in with your Google account and confirming with an emoji on your
phone, or with a QR code on phones that still offer one), then keeps your conversations in sync.

MessageSolutions is an independent project. It is not made by, endorsed by or affiliated with Google.

---

## Features

- **Pair once with your Google account** (sign in, then tap the matching emoji on your phone), or
  with a QR code on phones that still show a QR code scanner. The pairing is saved on your PC and
  encrypted for your Windows account (DPAPI), so MessageSolutions reconnects on its own the next time it
  opens.
- **Sync:** Inbox, Archived, and Spam & blocked folders, with unread counts, pinned conversations
  and grouping by date (Today, Yesterday, Earlier this week…).
- **Read:** chat-style conversation view with day separators, group-chat sender names, reactions,
  typing indicators, and sent / delivered / read status.
- **Send:** text messages, plus pictures and files (attach button, or drag and drop onto the
  conversation). Press Enter to send, Shift+Enter for a new line (configurable).
- **Pictures and attachments** show previews when available. Click one to open it, or choose
  **Save** to download it to a file. A local picture shows while your phone confirms the send.
- **New conversations** with one number, or several for a group.
- **The MailSolutions look:** title bar with search, File / Home / View / Help ribbon, module rail,
  folder pane, three-line conversation rows and a status bar. Light, dark or follow-Windows themes,
  interface zoom, and tighter spacing.
- **Integrated Windows controls:** the existing top toolbar replaces the native title bar.
  Drag empty toolbar space to move; double-click it to maximize or restore. Resize from any
  edge or corner. Click the app icon, right-click empty toolbar space, or press **Alt+Space**
  for the Windows system menu (including keyboard Move and Size). Minimize, maximize/restore
  and close are at the top right, with Tab focus, Enter/Space activation and accessible names.
  **Alt+F4** closes normally, saving settings and cached messages. Maximizing uses the current
  monitor's work area; edge hit targets follow Windows DPI. At narrow widths or high interface
  zoom, phone status remains in the status bar and account access stays on the avatar.
- **ContactSolutions contacts and groups:** use shared names, pictures, themes and banner images in
  conversations. Right-click a conversation to add or edit its contact or group.
- **Delete:** right-click a message to delete it, or right-click a conversation to delete its
  full message history from your phone after confirmation.
- **Works offline for reading:** a copy of recent conversations is kept locally, so the window opens
  instantly and shows what it last synced.
- **Updates:** checks GitHub Releases when the app opens and offers to download and install a newer Windows release. You can also check in File > About. Your local data stays in place.
- Keyboard: **Ctrl+N** new conversation, **F5** sync, **Ctrl+F** / **Ctrl+E** search,
  **↑ / ↓** move between conversations, **Esc** close dialogs.
- Accessible: every control has a name for screen readers (UI Automation).

## Getting started

1. Download **`MessageSolutions-windows-x64.msi`** from the [latest release](../../releases/latest) and
   run it. Like MailSolutions's installer, it installs into Program Files, adds a Start Menu shortcut,
   and offers to launch MessageSolutions when it finishes. To remove MessageSolutions later, use Windows
   **Settings › Apps**.
   *Portable instead:* download `MessageSolutions-windows.zip`, unzip it anywhere, and keep
   `MessageSolutions.exe` and `messagesolutions-bridge.exe` in the same folder.
2. Open **MessageSolutions** from the Start menu and click **Sign in with Google**.
3. In the window that opens, sign in with the Google account your phone's Google Messages uses.
   (On the phone, Google Messages › profile picture › **Device pairing** says "sign in to the same
   Google Account".)
4. Google Messages on your phone shows a few emoji. Tap the one MessageSolutions shows.

*Older phones:* if your phone's Device pairing screen has a **QR code scanner**, you can choose
**Use a QR code instead** and scan the code MessageSolutions shows.

Your conversations appear within a few seconds. Your phone does the actual sending, so it needs to
stay on and connected to the internet.

> Only one Google Messages web session can be active at a time. If you open Google Messages for
> web in a browser, MessageSolutions shows "open in another window". Click **Use here** in the status
> bar to switch back.

To try the window without a phone, run `MessageSolutions.exe --demo` (add `--light` or `--dark` to pick
a theme). It shows made-up sample conversations and saves nothing.

### Unpairing

Go to **File › Info › Unpair this computer**. Your phone forgets this PC, and MessageSolutions deletes
its local copy of your conversations. Your messages stay on your phone.

## ContactSolutions contacts

MessageSolutions reads the shared ContactSolutions database at `%APPDATA%\ContactSolutions\contacts.db` (or `CONTACTSOLUTIONS_DATA`) even when ContactSolutions is closed. It matches phone numbers to names and photos, matches group members to group conversations, and uses their themes and banner pictures in chats. The People screen shows contacts and opens ContactSolutions dialogs to create, edit, or change a theme. Right-click a conversation to add or edit its contact or contact group; MessageSolutions refreshes the shared store when the dialog saves.

## Where data is kept

Everything lives in `%APPDATA%\MessageSolutions`. First start copies the legacy
MessageCraft profile, retaining the original and refusing conflicts. Pairing
remains tied to the same Windows account. Read [migration and recovery](docs/rename-migration.md)
before upgrading. The 1.5.0 updater requires a one-time manual download of the renamed MSI.

| File | What it is |
| --- | --- |
| `session.bin` | The pairing keys (and, for Google-account pairing, the Google session), encrypted for your Windows account (DPAPI) |
| `cache.json` | A copy of your conversation list and recent messages, for a fast start |
| `ui.json` | Your settings (theme, zoom, panes) |
| `media\` | Pictures and attachments you've opened |
| `bridge.log` | The connection helper's log (no message text), useful when reporting a problem |

## How it works

MessageSolutions is two programs that talk to each other over JSON lines on standard input and output:

```
MessageSolutions.exe  (Rust, egui)   ── commands ─▶  messagesolutions-bridge.exe  (Go, libgm)  ──▶  Google Messages
     the window                  ◀── events ───     pairing, sync, send                    ⇅ your phone
```

- **`src/`: the window.** Rust with [egui/eframe](https://github.com/emilk/egui). The palette, type
  scale and layout follow MailSolutions's theme so the two apps look like a family.
- **Sign in with Google** runs `MessageSolutions.exe --google-signin`: a small Edge WebView2 window where
  you sign in. It hands the Google session cookies that Google Messages for web uses to the helper,
  then deletes its own browser profile. The helper keeps them, encrypted, with the pairing.
- **`bridge/`: the connection helper.** Go, using
  [libgm](https://github.com/mautrix/gmessages/tree/main/pkg/libgm) from mautrix-gmessages for the
  Google Messages for web protocol (pairing, long-polling for updates, end-to-end encrypted
  requests to the phone, media upload and download).
- **`web/`: the download page** (static HTML), published alongside each build.
- **`packaging/`: the Windows installer** (WiX: `windows/messagesolutions.wxs` and `windows/package.ps1`)
  and the icon generator.

### Bridge protocol (summary)

Commands (window → helper): `connect`, `reconnect`, `pair` (QR), `pair_google {cookies}`, `cancel_pair`, `sync {folder,count}`,
`messages {conversation_id,count,before_id?,before_ts?}`, `send {conversation_id,text,tmp_id,files?,reply_to?}`,
`mark_read`, `typing`, `new_chat {numbers,group_name?}`, `media {media_id,key,mime}`,
`delete_message`, `delete_history`, `unpair`.

Events (helper → window): `state`, `qr`, `qr_expired`, `google_account`, `emoji`, `emoji_image`,
`pair_failed`, `paired`, `unpaired`, `phone`, `sims`,
`syncing`, `synced`, `conversations`, `conversation`, `messages`, `message`, `send_result`,
`typing`, `media`, `deleted`, `phone_alert`, `error`.

## Building from source

Requirements (Windows 10/11, x64):

- [Rust](https://rustup.rs) (stable, MSVC toolchain) with the Visual Studio C++ build tools
- [Go](https://go.dev/dl/) 1.27 or later (or unzip Go into `.tools\go`, which the build script uses
  when Go isn't on PATH)
- Node.js is optional: it only provides the `npm run build` shortcut
- For the installer: [WiX v5](https://wixtoolset.org)
  (`dotnet tool install --global wix --version 5.0.2`). Without WiX, the build skips the MSI and
  still makes everything else.

`MessageSolutions.exe` links the C runtime statically, so neither the installer nor the zip needs the
Visual C++ redistributable. The icon comes from `packaging/make_icon.py`, which writes
`assets/messagesolutions.ico`.

```powershell
npm run build
# or, without Node:
powershell -ExecutionPolicy Bypass -File build.ps1
```

The output goes to `dist\`:

- `MessageSolutions.exe`
- `messagesolutions-bridge.exe`
- `update.ps1` (release check and installer download)
- `MessageSolutions-windows-x64.msi` (the installer, built when [WiX v5](https://wixtoolset.org) is
  installed; see `packaging/windows/package.ps1`)
- `MessageSolutions-windows.zip` (portable: both programs plus the notices)
- `index.html` and `help.html` (the download page)

### Continuous builds and releases

`.github/workflows/build.yml` tests and packages main pushes, pull requests and manual branch runs
on Windows. Successful main pushes produce immutable release candidates. A version tag promotes
the exact commit's verified main candidate into a GitHub Release with the MSI and portable ZIP;
it does not repeat compilation, tests or packaging. Existing releases are never overwritten.

Before tagging, finish source/version/release notes and require successful exact-commit main CI:
`cargo test --locked`, `cargo test --manifest-path contactsolutions-store/Cargo.toml`,
`go test ./...` in `bridge/`, and `build.ps1`. These commands run once in that producer; local
checks should target changed behavior rather than repeat already-green suites. Promotion checks
repository/workflow/run/attempt/commit, version, toolchain/target/features and SHA-256 hashes.
Missing or expired candidates fail closed. See [the release sequence](packaging/release-workflow.md)
for recovery, retained gates and public final-byte verification. Published 1.6.0 is unchanged.
See [the window verification guide](packaging/window-verification.md) for isolated native checks
and the distinction between automated window checks and physical desktop interaction.

## Troubleshooting

- **Pairing doesn't finish.** Make sure you signed in with the same Google account the phone's
  Google Messages uses, the phone is online, and Google Messages is up to date. If Google says it
  can't find your phone, open Google Messages › Device pairing on the phone first.
- **Google won't let you sign in** in the sign-in window ("this browser may not be secure"): try
  again later, or pair with a QR code if your phone offers one.
- **"Your phone isn't responding."** The phone is off, asleep without data, or has no connection.
  MessageSolutions reconnects on its own when the phone answers again.
- **A message says "Not sent".** Right-click it and choose **Try sending again**. If it keeps
  failing, check that Google Messages is your phone's default SMS app.
- **Anything else:** look in `bridge.log` (File › Info › Open data folder).

## Licences

- **`bridge/` (messagesolutions-bridge.exe)** links libgm, which is **AGPL-3.0**. The helper is
  therefore distributed under the AGPL-3.0, and its source is this folder.
- **The window (`src/`, `web/`)** is MIT OR Apache-2.0. It follows the original MailCraft design values (MIT
  OR Apache-2.0).
- **contactsolutions-store** is MIT, with its license in `contactsolutions-store/LICENSE`.
- **Inter** typeface: SIL Open Font License 1.1 (`assets/fonts/OFL-Inter.txt`).

See [NOTICE.md](NOTICE.md) for details.
