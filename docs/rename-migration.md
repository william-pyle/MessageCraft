# MessageSolutions 1.6.0 migration and recovery

MessageSolutions retains MessageCraft's Rust/egui window, Go helper, JSON-line
protocol and integrated toolbar behavior. No UI framework or service rewrite is
part of this release. The existing MailCraft design attribution is retained;
current peer branding is MailSolutions. The renamed contacts integration uses
ContactSolutions 1.3.0. Old running Craft consumers are outside this maintenance
cutover contract.

## Profile and credentials

The new profile is `%APPDATA%/MessageSolutions`. On normal startup, before loading
settings, starting the helper, checking updates or opening sign-in, the app checks
for `%APPDATA%/MessageCraft`. It holds every legacy file against writes/deletion
while copying to `MessageSolutions.migration-staging`, streams and verifies SHA-256
for every file, validates settings/cache types, and activates the completed copy
by renaming the staging folder. The original is retained unchanged. Existing
conversation/message IDs and media filenames are copied, not imported or recreated.
Unknown settings fields survive saving. Migration itself does not decrypt pairing.

The receipt `migration-receipt.json` contains the source path and file hashes,
never credential values. A repeat startup checks the retained source against it;
divergent legacy data stops startup rather than choosing or merging profiles.
Do not continue using old apps against that retained source. Copying requires
enough free space for the complete profile. Source files are streamed rather than
loading all media into RAM. Symbolic links/reparse points are refused.

`session.bin` is copied byte-for-byte. DPAPI remains tied to the same Windows user
and machine context, with permanent entropy **`MessageCraft session v1`**. This
intentional old string must never be renamed. The display description on newly
encrypted sessions is MessageSolutions; this does not change the entropy. There
is no Windows Credential Manager entry to rename. The temporary WebView sign-in
profile is part of the copied profile; no accounts are contacted by migration.
A failed session decrypt/read now reports an error and preserves the file rather
than presenting the failure as an unpaired account.

The helper requires an explicit prepared `--data` path from the window. It cannot
silently create an empty replacement profile when run by itself. `--demo` skips
migration, stored settings, contacts, helper and update requests; release validation
uses only isolated synthetic APPDATA and LOCALAPPDATA roots.

## Conflicts, interruption and rollback

- Existing old/new folders without a matching receipt: stop and preserve both.
  There is no automatic overwrite or merge.
- Open/inaccessible legacy file: startup reports the failure. Coordinate closing
  consumers yourself during authorized maintenance; the app never closes them.
- Staging/lock leftover: preserve it, verify no migration is running, then move it
  aside for review before retry. Do not blindly delete failed attempts.
- Invalid settings/cache or pending `cache.tmp`, `session.bin.tmp` or
  `ui.json.pending`: recover from a verified backup; no empty profile replaces it.
- Insufficient space, copy/hash failure or invalid receipt: originals remain in
  place; a failed stage is retained for investigation.

Before rolling back binaries, preserve the new profile and every post-migration
write. The old folder contains only the pre-migration state; replacing the new
profile with it would lose subsequent changes. Installer rollback does not roll
back user data. Do not assume a ContactCraft export backs up this application's
messages, pairing, media or settings. No real profile was used by this release's
tests, and no installer or live-data migration was run on the owner's computer.

## Contacts contract

`contactsolutions-store/` is the byte-identical `store/` tree from the published
ContactSolutions v1.3.0 commit `17245278eef33d2be5fd57f227dadcd6b941bcf3`.
It preserves the existing shared-library boundary; no second data implementation
is introduced. The store selects `%APPDATA%/ContactSolutions` or
`CONTACTSOLUTIONS_DATA`, performs its own guarded ContactCraft migration, and
retains schema/contact/group identities. Legacy `CONTACTCRAFT_DATA` is a migration
source only and requires a separate `CONTACTSOLUTIONS_DATA` destination.

Executable discovery reads metadata key `contactsolutions.exe`, then standard
`ContactSolutions/ContactSolutions.exe` installation paths. Dialog arguments remain
`--edit-phone`, `--edit-contact`, `--new-contact`, `--new-group`, `--open-group` and
the existing theme/editor arguments. Exit 0 is saved (contacts print `saved <id>`),
1 is cancelled, and failures print `error ...` or return another exit code. Store
revision polling and refresh after dialog closure are retained. Synthetic tests
exercise the released store's contacts/groups/theme paths and executable metadata.
No peer checkout or peer canonical record is edited by this migration.

## Installer and updater

The MSI remains unsigned, per-machine, x64, and uses UpgradeCode
`43FAC59B-7634-4B81-9389-22FD44451B14`. MajorUpgrade behavior is retained, including
the prior AllowDowngrades policy. ProductCode is generated for the new package.
The renamed executable/install directory component has new GUID
`237ED275-7736-4C2D-8C99-F48681328BF5`; the renamed shortcut/registry component uses
`F986339F-6AF3-444B-8026-A41F63D73614`. Changing key paths requires these new GUIDs.
The old installed product is found by the unchanged upgrade family and its MSI
removes old application files/shortcut before the new files are installed under
Program Files/MessageSolutions. Neither package owns APPDATA profile files.

The interactive finish page offers launch, unchanged from the old installer;
unattended mode does not run that UI event. Actual upgrade/uninstall, pinned
shortcut repair and installed-location behavior require a disposable Windows
environment and remain unverified here. No installation is authorized for this task.

New assets are `MessageSolutions-windows-x64.msi` and
`MessageSolutions-windows.zip`; the ZIP includes `MessageSolutions.exe`,
`messagesolutions-bridge.exe`, `update.ps1` and notices. The new updater resolves
the renamed repository and exact MSI name. Old 1.5.0 updater lookup expects the
old asset name, so this rename requires a one-time manual download of the new MSI;
no old-name asset alias is published. All historical releases/tags remain.

## Source and Workspace cutover

The in-place folder move was blocked by Windows. The owner then explicitly
authorized copying the checkout and retaining the old directory for their later
cleanup. C:/Projects/MessageSolutions is authoritative; C:/Projects/MessageCraft
is retained untouched. Matching baseline commits/refs and a clean tracked tree
were verified before changes; the recovery Git bundle and Workspace snapshots
are in AlphaWorkspace/projects/messagecraft/recovery/rename-1.6.0.

Workspace ID `messagecraft`, canonical folder `projects/messagecraft`, routes
`#/messagecraft/0` and `#/messagecraft/1`, category/status/pins are stable. Display
name and generated folder become MessageSolutions and
`workspace/2026-11-14_messagesolutions`. Historical evidence and legal/source
attributions retain old names. The existing GitHub repository is renamed only
after local validation and exact-commit main CI; its repository ID stays 1454531959.

## Historical 1.6.0 release gate

Finish versions, this guide, notes and workflow before the release commit. Run
`cargo test --locked`, `cargo test --manifest-path contactsolutions-store/Cargo.toml`,
`go test ./...` in bridge, and `build.ps1`. Run the isolated native smoke harness
in packaging/window-verification.md and review the actual frames. Push main and
require a successful Windows Build on that exact commit before creating/pushing
`v1.6.0`. Verify release/tag SHA, public MSI/ZIP hashes, versions/resources, MSI
upgrade/component identity, extracted payload and packaged updater lookup. Build,
publish, install and observed native interaction are separate lifecycle states.

For future releases, use [the current release sequence](../packaging/release-workflow.md).
The exact-commit main Windows producer satisfies the required suites/build; tags
promote its verified immutable bytes instead of rebuilding. The 1.6.0 procedure
and published assets above remain historical evidence, not a request to retag.
