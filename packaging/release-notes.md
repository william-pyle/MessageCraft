## MessageSolutions 1.6.0

MessageCraft is now MessageSolutions. The Rust window, Go connection helper and
existing messaging and integrated Windows toolbar flows remain in place.

- Download **MessageSolutions-windows-x64.msi** for the per-machine Windows x64
  installer, or **MessageSolutions-windows.zip** for portable use. Keep both
  executables and update.ps1 together.
- The first normal start copies the legacy MessageCraft profile to MessageSolutions
  through a verified staging folder, retaining the original. Conflicts, locked
  files, malformed settings and interrupted saves stop safely for recovery.
- Pairing ciphertext and the Windows DPAPI binding are preserved. Existing
  settings, message IDs and media are retained. Unknown settings fields round-trip.
- Contact integration now uses ContactSolutions 1.3.0's published store, executable
  discovery and existing editor dialogs. Current peer branding uses MailSolutions.
- MSI upgrade-family identity is unchanged; renamed components get new GUIDs.
  The new updater checks the MessageSolutions repository and assets.

**One-time upgrade:** download the new MSI manually; the old MessageCraft updater
expects the old asset name. No historical releases or tags are removed.

See [migration and recovery](https://github.com/william-pyle/MessageSolutions/blob/v1.6.0/docs/rename-migration.md) before first start.
Migration requires the same Windows account for encrypted pairing and an offline
legacy profile. Preserve both profiles if a conflict is reported. A contacts export
is not a backup of messages, pairing or application settings.

No real account or live-data migration, installation/uninstallation, phone
pairing/sending, physical window interaction or mixed-DPI transition is claimed
by publication. Those remain separate validation steps. Previous release notes
are retained in docs/history/release-notes-1.5.0.md.
