# MessageSolutions

Keep the existing Rust egui/eframe window, Go JSON-line helper and bundled shared
store architecture. The authoritative checkout is C:/Projects/MessageSolutions;
the owner authorized copying the locked MessageCraft checkout and will clean up
that old folder. Do not edit or delete the retained old checkout.

Read README.md, docs/rename-migration.md, packaging/window-verification.md and
.github/workflows/build.yml before release work. ContactSolutions' store snapshot
is byte-identical to its published v1.3.0 store; keep its MIT license and history.
DPAPI entropy `MessageCraft session v1` is a permanent credential format contract.
Legacy migration readers and legal/history references are intentional exceptions.

For releases require cargo test --locked, cargo test --manifest-path
contactsolutions-store/Cargo.toml, go test ./... inside bridge and build.ps1.
The successful exact-commit main Windows CI run supplies this evidence; do not
repeat those full suites locally or on a tag without a changed-input reason.
Run focused local checks for changes. Follow packaging/release-workflow.md:
tags promote the immutable main candidate after producer and final-byte checks,
and fail closed rather than rebuilding when evidence is missing or mismatched.
Use isolated synthetic profiles for native/credential tests. Never read live
profiles or close a user's session. Before tagging, finish versions/docs/workflow,
push main and require green Windows CI for that exact commit. Tagging v1.6.0 is
authorized by the rename task, as are commit/push and existing repository rename
after validation. Preserve all prior releases and tags. No install/uninstall,
real-profile launch, login/send or restart is authorized by this task.

Update Alpha's projects/messagecraft/project.json and build.mjs output as facts
change, retaining stable project ID/routes and canonical ID-based folder. The
MailSolutions checkout and canonical records are handled by a separate terminal.

For every new commit subject and body, use MessageSolutions, MailSolutions or
neutral wording. Never use MessageCraft or MailCraft in a new commit message.
Before pushing, inspect the complete messages of every outgoing commit, including
their bodies. This rule is prospective: do not amend published historical commits,
rewrite existing history or force-push to remove earlier product names.
