# MessageCraft

**Your phone's text messages on Windows, in the style of [MailCraft](https://github.com/UCS-Care-Group/mailcraft).**

MessageCraft is a desktop app for reading and sending the SMS, MMS and RCS chat messages on your
Android phone from a Windows PC. It pairs with Google Messages on your phone the same way Google
Messages for web does (by scanning a QR code), then keeps your conversations in sync.

MessageCraft is an independent project. It is not made by, endorsed by or affiliated with Google.

---

## Features

- **Pair once with a QR code.** The pairing is saved on your PC and encrypted for your Windows
  account (DPAPI), so MessageCraft reconnects on its own the next time it opens.
- **Sync:** Inbox, Archived, and Spam & blocked folders, with unread counts, pinned conversations
  and grouping by date (Today, Yesterday, Earlier this week…).
- **Read:** chat-style conversation view with day separators, group-chat sender names, reactions,
  typing indicators, and sent / delivered / read status.
- **Send:** text messages, plus pictures and files (attach button, or drag and drop onto the
  window). Press Enter to send, Shift+Enter for a new line (configurable).
- **Pictures and attachments** download from your phone on demand. Click one to open it full size.
- **New conversations** with one number, or several for a group.
- **The MailCraft look:** title bar with search, File / Home / View / Help ribbon, module rail,
  folder pane, three-line conversation rows and a status bar. Light, dark or follow-Windows themes,
  interface zoom, and tighter spacing.
- **Works offline for reading:** a copy of recent conversations is kept locally, so the window opens
  instantly and shows what it last synced.
- Keyboard: **Ctrl+N** new conversation, **F5** sync, **Ctrl+F** / **Ctrl+E** search,
  **↑ / ↓** move between conversations, **Esc** close dialogs.
- Accessible: every control has a name for screen readers (UI Automation).

## Getting started

1. Download **`MessageCraft-windows-x64.msi`** from the [latest release](../../releases/latest) and
   run it. Like MailCraft's installer, it installs into Program Files, adds a Start Menu shortcut,
   and offers to launch MessageCraft when it finishes. To remove MessageCraft later, use Windows
   **Settings › Apps**.
   *Portable instead:* download `MessageCraft-windows.zip`, unzip it anywhere, and keep
   `MessageCraft.exe` and `messagecraft-bridge.exe` in the same folder.
2. Open **MessageCraft** from the Start menu. It shows a QR code.
3. On your Android phone, open **Google Messages**, tap your profile picture (or the **⋮** menu),
   choose **Device pairing**, then **QR code scanner**, and scan the code.

Your conversations appear within a few seconds. Your phone does the actual sending, so it needs to
stay on and connected to the internet.

> Only one Google Messages web session can be active at a time. If you open Google Messages for
> web in a browser, MessageCraft shows "open in another window". Click **Use here** in the status
> bar to switch back.

To try the window without a phone, run `MessageCraft.exe --demo` (add `--light` or `--dark` to pick
a theme). It shows made-up sample conversations and saves nothing.

### Unpairing

Go to **File › Info › Unpair this computer**. Your phone forgets this PC, and MessageCraft deletes
its local copy of your conversations. Your messages stay on your phone.

## Where data is kept

Everything lives in `%APPDATA%\MessageCraft`:

| File | What it is |
| --- | --- |
| `session.bin` | The pairing keys, encrypted for your Windows account (DPAPI) |
| `cache.json` | A copy of your conversation list and recent messages, for a fast start |
| `ui.json` | Your settings (theme, zoom, panes) |
| `media\` | Pictures and attachments you've opened |
| `bridge.log` | The connection helper's log (no message text), useful when reporting a problem |

## How it works

MessageCraft is two programs that talk to each other over JSON lines on standard input and output:

```
MessageCraft.exe  (Rust, egui)   ── commands ─▶  messagecraft-bridge.exe  (Go, libgm)  ──▶  Google Messages
     the window                  ◀── events ───     pairing, sync, send                    ⇅ your phone
```

- **`src/`: the window.** Rust with [egui/eframe](https://github.com/emilk/egui). The palette, type
  scale and layout follow MailCraft's theme so the two apps look like a family.
- **`bridge/`: the connection helper.** Go, using
  [libgm](https://github.com/mautrix/gmessages/tree/main/pkg/libgm) from mautrix-gmessages for the
  Google Messages for web protocol (pairing, long-polling for updates, end-to-end encrypted
  requests to the phone, media upload and download).
- **`web/`: the download page** (static HTML), published alongside each build.
- **`packaging/`: the Windows installer** (WiX: `windows/messagecraft.wxs` and `windows/package.ps1`)
  and the icon generator.

### Bridge protocol (summary)

Commands (window → helper): `connect`, `reconnect`, `pair`, `cancel_pair`, `sync {folder,count}`,
`messages {conversation_id,count,before_id?,before_ts?}`, `send {conversation_id,text,tmp_id,files?,reply_to?}`,
`mark_read`, `typing`, `new_chat {numbers,group_name?}`, `media {media_id,key,mime}`,
`delete_message`, `unpair`.

Events (helper → window): `state`, `qr`, `qr_expired`, `paired`, `unpaired`, `phone`, `sims`,
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

`MessageCraft.exe` links the C runtime statically, so neither the installer nor the zip needs the
Visual C++ redistributable. The icon comes from `packaging/make_icon.py`, which writes
`assets/messagecraft.ico`.

```powershell
npm run build
# or, without Node:
powershell -ExecutionPolicy Bypass -File build.ps1
```

The output goes to `dist\`:

- `MessageCraft.exe`
- `messagecraft-bridge.exe`
- `MessageCraft-windows-x64.msi` (the installer, built when [WiX v5](https://wixtoolset.org) is
  installed; see `packaging/windows/package.ps1`)
- `MessageCraft-windows.zip` (portable: both programs plus the notices)
- `index.html` and `help.html` (the download page)

### Continuous builds and releases

`.github/workflows/build.yml` builds every push and pull request on Windows and uploads `dist\` as
an artifact. To publish a release, push a version tag:

```powershell
git tag v1.0.0
git push origin v1.0.0
```

The workflow then creates a GitHub Release with the installer (`MessageCraft-windows-x64.msi`) and
the portable zip attached.

## Troubleshooting

- **The QR code never connects.** Make sure the phone is online and Google Messages is up to date.
  Pairing has to be done from Google Messages' **Device pairing** screen.
- **"Your phone isn't responding."** The phone is off, asleep without data, or has no connection.
  MessageCraft reconnects on its own when the phone answers again.
- **A message says "Not sent".** Right-click it and choose **Try sending again**. If it keeps
  failing, check that Google Messages is your phone's default SMS app.
- **Anything else:** look in `bridge.log` (File › Info › Open data folder).

## Licences

- **`bridge/` (messagecraft-bridge.exe)** links libgm, which is **AGPL-3.0**. The helper is
  therefore distributed under the AGPL-3.0, and its source is this folder.
- **The window (`src/`, `web/`)** is MIT OR Apache-2.0. It follows MailCraft's design values (MIT
  OR Apache-2.0).
- **Inter** typeface: SIL Open Font License 1.1 (`assets/fonts/OFL-Inter.txt`).

See [NOTICE.md](NOTICE.md) for details.
