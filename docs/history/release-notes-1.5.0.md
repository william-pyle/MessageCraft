## MessageCraft 1.5.0

Your Android phone's text messages on Windows, in the style of MailCraft.

### Download

- **MessageCraft-windows-x64.msi**: the installer (recommended). It installs into Program Files,
  adds a Start Menu shortcut, and offers to launch MessageCraft when it finishes.
- **MessageCraft-windows.zip**: the portable version. Unzip it and keep both programs in the same
  folder.

### New in 1.5.0

- The existing top toolbar now includes the app icon/system menu and minimize,
  maximize/restore and close buttons, replacing Windows' separate title bar and control box.
- Drag empty toolbar space, double-click to maximize/restore, or resize from all edges and
  corners. Use Alt+Space for the native window menu and keyboard Move/Size; Alt+F4 closes normally.
- Controls have accessible names and visible keyboard focus. Search, quick actions and account
  access remain available, with compact layout at higher interface zoom.
- Native sizing uses the monitor work area and DPI-scaled edge targets. Close preserves the
  existing save-and-exit behavior. Pairing, messaging and ContactCraft storage are unchanged.

### New in 1.4.1

- New and Edit actions for ContactCraft contacts and groups open editor dialogs when used with
  ContactCraft 1.2.3 or later. MessageCraft refreshes shared groups after a saved change.

### New in 1.4.0

- Sent messages now reconcile with the phone's confirmation, so the conversation shows one bubble.
- Sent and received pictures show previews when available. Save any attachment to a chosen file,
  or click it to open it. Drag files into the conversation to attach them.
- ContactCraft contact groups supply conversation names and themes, including banner pictures.
  Right-click a conversation to add or edit its contact or group in ContactCraft 1.2.0.
- Right-click a message to delete it, or right-click a conversation to delete its history from the
  phone after confirmation.

### Everything else

- Pair with Google Messages on your Android phone: sign in with your Google account and tap the
  matching emoji on your phone (or scan a QR code on phones that still offer one). You only pair once.
- Sync the Inbox, Archived, and Spam & blocked folders, with unread counts and pinned conversations.
- Read conversations with group names, reactions, typing indicators and delivery status.
- Send texts, pictures and files. Start new conversations and groups.
- See and search ContactCraft contacts, start a text, and open ContactCraft to edit a contact's theme.
- Check for updates when the app opens or from File > About.
- MailCraft's look: ribbon, folder pane, conversation list and reading pane, in light or dark.

### Getting started

Open MessageCraft, click **Sign in with Google**, and sign in with the Google account your phone's
Google Messages uses. Then tap the emoji MessageCraft shows on your phone. Your phone needs to stay
on and online. See the [README](https://github.com/william-pyle/MessageCraft#readme) for more.

To use your ContactCraft contacts, click **People** on the left. MessageCraft reads ContactCraft's
shared contacts and groups even when ContactCraft is closed; adding or editing opens ContactCraft.

MessageCraft isn't code-signed yet, so Windows SmartScreen may warn the first time you run the
installer. Choose **More info › Run anyway**.

### Known issues

This is an early build. Known problems and fixes in progress are tracked in
[Issues](https://github.com/william-pyle/MessageCraft/issues); please report anything else you
find there.
