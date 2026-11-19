# MessageSolutions release sequence

Publication and installation are separate. The workflow optimization task allows
scoped commits/pushes only: no new tag/release, asset replacement, installation,
restart or real-profile access. Published v1.6.0 stays unchanged. A future release
requires its own current authorization; historical rename permission is not enough.

## Prepare once, promote once

1. Finish source, versions (`Cargo.toml`, lockfile and `package.json`), notes and
   workflow changes before the release commit. Preserve architecture, store
   snapshot/license, session entropy, identities, MSI UpgradeCode and updater names.
2. Run focused local checks for changed behavior. Workflow changes require
   `python -m unittest discover -s packaging/ci -p "test_*.py" -v` and actionlint.
   Review complete outgoing commit subjects and bodies; use current names or
   neutral wording. Push to the verified owner origin without rewriting history.
3. Require the successful **Build / windows** job and completed green **main push
   run on that exact commit**. It runs `cargo test --locked`,
   `cargo test --manifest-path contactsolutions-store/Cargo.toml`, `go test ./...`
   inside bridge, and `build.ps1`, including WiX MSI packaging. This is the required
   full-suite/build evidence; repeating it locally adds no gate. Source, lockfile,
   workflow, toolchain, flags or packaging changes require new matching evidence.
4. Inspect candidate packages and run relevant synthetic native checks when app or
   installer behavior changed. Preserve existing signing/approval/security gates.
   The package remains unsigned; promotion does not introduce signing or installation.
5. Only with release authorization, tag the verified commit as `v<package-version>`.
   The tag job selects the newest main push run for that exact SHA, requires success
   including the Windows job, and downloads the unique artifact by immutable ID.
   It checks repository ID/name, workflow ID/path, run/attempt, commit, version,
   target, toolchain, features, build inputs, archive digest and every payload hash.
   PR/manual/fork candidates cannot be released. No tag build fallback exists.
6. Publication refuses an existing release. It attaches the already-verified MSI
   and ZIP, using the producer's notes, then downloads the public assets and checks
   sizes and SHA-256 against the manifest and GitHub digests. Failure here means
   publication may have occurred but is **not verified**; inspect without replacing
   assets. Report implemented, tested, pushed, published, installed and observed
   states separately. Installer upgrade/uninstall and real account behavior remain
   separate, authorization-dependent checks.

## Recovery and read-only rehearsal

Candidates expire after 30 days. Missing/expired/mismatched evidence stops release.
Investigate mismatches; never substitute a different SHA or unverified local file.
For a legitimately expired candidate, rerun the original main push run at the same
commit, wait for its new attempt to pass, then retry the tag job. If source changes
are needed, prepare a new version/commit under release authorization; never move a
published tag. Candidate names include the attempt to prevent stale artifact reuse.

After a green main run, rehearsal needs no tag, release, build or install:

```powershell
python packaging/ci/release.py verify-run <run-id> <fresh-output-directory>
```

The checked-out HEAD must match the run. The consumer verifies the same provenance,
archive and file hashes used by release preparation. Existing output directories
are refused. Synthetic unit tests cover failure paths without network or user data.
For the same check on the actual Ubuntu consumer, dispatch Build on the matching
branch with `candidate_run_id=<main-run-id>`. That read-only job skips Windows
compilation and cannot publish; an empty input retains the ordinary manual build.

## Workflow inventory and performance baseline

Audited 2026-11-19 UTC (2026-11-18 America/Chicago). There is one active workflow:
`.github/workflows/build.yml`, ID 415196938, in repository ID 1454531959.
Originally its single Windows job handled main pushes, `v*` tags, all PRs and manual
runs; there were no job dependencies, concurrency controls or other workflows.
There is no website deployment trigger. Repository ruleset API returned none and
main protection returned 404; the documented gate is now also enforced by promotion.
No repository protection settings were changed.

Measured GitHub job durations (not estimates):

| Run | Ref | Windows job | Rust tests | Go tests | Build/package |
| --- | --- | ---: | ---: | ---: | ---: |
| [44440222653](https://github.com/william-pyle/MessageSolutions/actions/runs/44440222653) | main | 317 s | 35 s | 22 s | 124 s |
| [44440788865](https://github.com/william-pyle/MessageSolutions/actions/runs/44440788865) | main, release SHA | 255 s | 35 s | 20 s | 107 s |
| [44441518336](https://github.com/william-pyle/MessageSolutions/actions/runs/44441518336) | v1.6.0, same SHA | 752 s | 218 s | 28 s | 350 s |
| [44443622319](https://github.com/william-pyle/MessageSolutions/actions/runs/44443622319) | main, policy docs | 725 s | 194 s | 28 s | 368 s |

The same-SHA tag consumed another 752 Windows seconds after the 255-second main
gate. Promotion removes that repeated job; its replacement download/verification
time must be measured separately. Do not call the entire 752 seconds a measured
net saving until a tag promotion has actually run.

Rust cache logs show a hit on the release main run and misses on the tag and next
main run. API inventory showed identical logical keys with different cache versions;
the repository rename/path change is a possible cause, not proven. Eight caches
were present; current Rust caches were about 917 MB and Go about 63 MB. Previously
only the root target was cached, excluding the standalone store target. Both are
now configured. Rust is pinned to the observed 1.100.0 toolchain; Go remains pinned
by go.mod (1.27.4), WiX remains 5.0.2. Rust cache invalidation retains toolchain,
manifests/locks, configuration and environment hashing. Only main push runs save
Rust caches. Go's setup action retains its toolchain/go.sum cache; cache hits never
skip required tests or become release evidence.

Main pushes and PR events are already disjoint for normal feature branches, so no
path filter is added that could hide required checks or fail to produce an
exact-commit candidate. Manual runs are inspection-only. Superseded nonrelease
runs cancel by event and branch/PR; tag runs never cancel each other. Compilation
and tests stay on one Windows runner to share build caches. Splitting Go's roughly
20–28 seconds into another runner would add setup/transfer overhead; parallel
app/store compilation would duplicate work and contend for the same runner.
The separate tag consumer needs no compiler, Go or WiX setup. Precompressed
packages upload without redundant ZIP compression.

Action behavior references: [Rust cache inputs](https://github.com/Swatinem/rust-cache),
[artifact IDs/digests](https://docs.github.com/en/rest/actions/artifacts),
[concurrency](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency).

## Shared rule correction for Alpha

This project's guidance now treats exact-commit CI as full-suite evidence and avoids
repeat local/tag builds. Alpha's global scope should apply the same evidence-based
rule to common release preparation rather than mandate repeated full local suites,
and distinguish a tag's publication permission from installation/restart permission.
No shared root, shared skill, registry or other project's records were edited here.
