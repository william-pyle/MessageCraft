# Integrated window controls: verification

MessageSolutions 1.6.0 keeps egui/eframe, winit and the existing Go bridge and ContactSolutions store.
The main Windows viewport is undecorated; the existing toolbar owns the window controls.
`src/window_chrome.rs` adds a Win32 subclass solely for DPI-scaled edge/corner hit tests.
It delegates all other messages to winit, including monitor work-area sizing, DPI changes,
minimum size and normal close. If subclass installation fails, a notice explains that the
native frame has been restored. System-menu failures use the existing error notice.

Alpha's `src/desktop/WindowControls.tsx`, `src-tauri/src/window_chrome.rs` and window creation
were inspected read-only as the interaction reference. MessageSolutions uses egui rather than
Alpha's Tauri/web toolbar. Its close command deliberately exits through `App::on_exit` to
save settings/cache and stop its helper; Alpha's close-to-background semantics do not apply.
MessageSolutions has no existing persisted window geometry; this release does not introduce one.
The separate Google sign-in window and all data formats remain unchanged.

## Repeatable checks

Run `cargo test --locked`, `cargo test --manifest-path contactsolutions-store/Cargo.toml`,
`go test ./...` from `bridge/`, and `powershell -NoProfile -ExecutionPolicy Bypass -File build.ps1`.
The Rust tests cover eight edge/corner directions at 100/200/300% geometry scales, interior
exclusion and toolbar spacing at different widths/zoom. CI runs both Rust suites and the
Go check before building MSI and portable packages.
The exact-commit main CI run supplies the full-suite/build evidence; do not rerun
those suites on tags or locally without changed inputs. For release promotion and
failure recovery, follow [release-workflow.md](release-workflow.md). Native checks
are still required when window behavior changes, using only synthetic fixtures.

Development builds also include an opt-in smoke harness. It never runs in release builds.
Use a **fresh isolated fixture**, never an active user's APPDATA:

```powershell
cargo build --locked
$fixture = Join-Path $env:TEMP ('MessageSolutions-window-' + [guid]::NewGuid())
$env:MESSAGESOLUTIONS_WINDOW_SMOKE = $fixture
$env:APPDATA = Join-Path $fixture 'data'
$env:CONTACTSOLUTIONS_DATA = Join-Path $fixture 'contacts'
New-Item -ItemType Directory -Force $env:APPDATA | Out-Null
& ./target/debug/MessageSolutions.exe --demo --dark
```

The harness runs the real app with synthetic demo conversations. It injects egui input to
exercise toolbar buttons and keyboard navigation, queries its own native window and installed
hit-test subclass, captures GPU-rendered frames, and closes through the toolbar. At shutdown
only, it enables saving the synthetic records into the fixture to verify the existing
`on_exit` settings/cache persistence path. No phone bridge or ContactSolutions connection is started.
Check `native-results.json` and `close-observed.json` for `passed: true`, and visually review
`dark.png`, `maximized.png`, `light-minimum.png` and `zoom-200.png`. Missing output is a failure.
These are client-area renders, not desktop captures or proof of physical input behavior.

## Physical desktop checklist

- Click the app icon and right-click blank toolbar space; use Alt+Space, menu mnemonics,
  keyboard Move/Size and Escape. Check the menu's disabled commands when maximized.
- Drag blank toolbar space, double-click maximize/restore, and drag down from maximized.
  Search, Sync, New conversation, account and caption buttons must not initiate dragging.
- Resize each edge and corner; check minimum size and no resize hit zones while maximized.
- Tab to each window control; check focus rings, Enter/Space, accessible names and Alt+F4.
- Minimize and restore using the taskbar; exercise Windows snap and multiple monitors,
  including negative monitor coordinates and different scaling. Check taskbar work-area fit.
- Close and reopen an isolated non-paired fixture; confirm settings/cache and error notices.

## 1.6.0 validation

All 17 native harness checks passed at 125% Windows display scaling, and toolbar
close round-tripped synthetic settings and cached conversations. Dark, light,
maximized and 200% zoom GPU frames were reviewed. The existing minimum-size/high
zoom pane cramping remains. These are injected egui events and native queries;
physical interaction and real-account tests below remain unverified. The local
release EXE/ZIP build passed; MSI packaging is performed by required Windows CI.

## Historical 1.5.0 verification limits (repeat for 1.6.0)

Automated native checks and rendered-frame review run on Windows at 125% display scaling.
All 17 smoke checks passed, including toolbar minimize/maximize/restore, injected double-click,
Tab/Enter/Space, Ctrl+F, all eight installed native hit tests and monitor work-area fit.
The separate close/persistence check also passed with synthetic settings and cached conversations.
Physical pointer dragging/resizing, interactive system-menu navigation, taskbar restore,
screen-reader announcements and mixed-DPI/multi-monitor transitions still require desktop
review: both available computer-use interfaces reported a missing native pipe. Real phone
pairing/sending and installer upgrade/uninstall are not exercised against active sessions.
The existing message panes can be cramped at minimum size, especially at 200% interface
zoom; toolbar controls stay available. No general pane-layout redesign is included.

Local packaging may skip MSI when WiX is unavailable; the Windows CI job installs WiX 5.0.2
and builds the MSI. Keep the stable UpgradeCode. The authorized rename changes asset names, component GUIDs,
Program Files destination, shortcuts and profile path; see docs/rename-migration.md. This is the existing unsigned package/update path;
publication does not mean installation. Verify downloaded MSI metadata/payload and ZIP content
and compare SHA-256 with the release's asset digests before reporting publication complete.
