# MessageCraft — notices

MessageCraft is a Windows app for the text messages on your Android phone. It pairs with Google
Messages the same way Google Messages for web does. It is not made by or affiliated with Google.

## Parts and their licences

- **messagecraft-bridge.exe** uses **libgm** from mautrix-gmessages
  (https://github.com/mautrix/gmessages), licensed **AGPL-3.0**. Because the helper includes it,
  the helper is covered by the AGPL too: if you share MessageCraft with others, share the helper's
  source (the `bridge` folder) with it.
- The look follows **MailCraft** (https://github.com/UCS-Care-Group/mailcraft): its colour
  palette, type scale and layout values, MIT OR Apache-2.0.
- **egui / eframe** (MIT OR Apache-2.0) draw the window.
- **Inter** typeface (SIL Open Font License 1.1, see `assets/fonts/OFL-Inter.txt`), used where
  Windows' Segoe UI isn't available.

## Where things are kept

`%APPDATA%\MessageCraft`: the pairing (`session.bin`, encrypted for your Windows account with
DPAPI), settings (`ui.json`), a copy of recent conversations (`cache.json`), downloaded pictures
(`media\`) and the helper's log (`bridge.log`).
