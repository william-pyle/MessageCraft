"""Seal and promote exact-commit Windows CI packages. No build or install fallback."""
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[2]
REPOSITORY = "william-pyle/MessageSolutions"
REPOSITORY_ID = 1454531959
WORKFLOW = ".github/workflows/build.yml"
WORKFLOW_ID = 415196938
TARGET = "x86_64-pc-windows-msvc"
RUST = "1.100.0"
ASSETS = ("MessageSolutions-windows-x64.msi", "MessageSolutions-windows.zip")
FILES = (*ASSETS, "release-notes.md")
PROMOTION = ROOT / "dist/promotion"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def command(*args):
    return subprocess.check_output(args, cwd=ROOT)


def api(path):
    return json.loads(command("gh", "api", f"repos/{REPOSITORY}/{path}"))


def pages(path, key):
    result = []
    for page in range(1, 101):
        batch = api(f"{path}{'&' if '?' in path else '?'}per_page=100&page={page}")[key]
        result.extend(batch)
        if len(batch) < 100:
            return result
    raise ValueError("API pagination limit reached; refusing incomplete evidence")


def sha(data):
    return hashlib.sha256(data).hexdigest()


def version():
    value = re.search(r'^version\s*=\s*"([^"]+)"', (ROOT / "Cargo.toml").read_text(), re.M)[1]
    require(re.fullmatch(r"\d+\.\d+\.\d+", value), "Expected a stable three-part version")
    require(json.loads((ROOT / "package.json").read_text())["version"] == value, "Package versions differ")
    return value


def contract():
    # These hashes are normalized for checkout line endings (Windows -> Linux).
    paths = ("Cargo.toml", "Cargo.lock", ".cargo/config.toml", "bridge/go.mod", "bridge/go.sum",
             "build.ps1", "build.rs", "packaging/windows/package.ps1", "packaging/windows/messagesolutions.wxs",
             WORKFLOW, "packaging/ci/release.py")
    return {p: sha((ROOT / p).read_text(encoding="utf-8-sig").encode()) for p in paths}


def go_version():
    return re.search(r"^go (\S+)", (ROOT / "bridge/go.mod").read_text(), re.M)[1]


def verify_tag(commit):
    obj = api(f"git/ref/tags/v{version()}")["object"]
    for _ in range(5):
        if obj["type"] != "tag":
            break
        obj = api(f"git/tags/{obj['sha']}")["object"]
    require(obj["type"] == "commit" and obj["sha"] == commit, "Remote tag commit mismatch")


def verify_run(run, commit):
    require(run["repository"]["id"] == REPOSITORY_ID and
            run["repository"]["full_name"] == REPOSITORY, "Wrong producer repository")
    require(run["head_repository"]["id"] == REPOSITORY_ID, "Fork producer refused")
    require(run["workflow_id"] == WORKFLOW_ID and run["path"] == WORKFLOW, "Wrong producer workflow")
    require(run["head_sha"] == commit and re.fullmatch(r"[a-f0-9]{40}", commit), "Wrong producer commit")
    require(run["event"] == "push" and run["head_branch"] == "main", "Producer must be a main push")
    require(run["status"] == "completed" and run["conclusion"] == "success", "Producer is not green")
    require(isinstance(run["id"], int) and run["id"] > 0 and run["run_attempt"] > 0, "Invalid run identity")


def verify_artifact(artifact, run):
    require(artifact["name"] == f"release-{run['head_sha']}-{run['run_attempt']}", "Wrong artifact name/attempt")
    require(artifact["expired"] is False, "Artifact expired; rerun the exact main producer")
    require(re.fullmatch(r"sha256:[a-f0-9]{64}", artifact.get("digest") or ""), "Missing archive digest")
    origin = artifact["workflow_run"]
    require(origin["id"] == run["id"] and origin["head_sha"] == run["head_sha"] and
            origin["head_branch"] == "main" and origin["repository_id"] == REPOSITORY_ID and
            origin["head_repository_id"] == REPOSITORY_ID, "Artifact origin mismatch")


def verify_manifest(manifest, run):
    expected = {"schema": 1, "repository": REPOSITORY, "repository_id": REPOSITORY_ID,
                "workflow": WORKFLOW, "workflow_id": WORKFLOW_ID, "run_id": run["id"],
                "run_attempt": run["run_attempt"], "commit": run["head_sha"], "version": version(),
                "target": TARGET, "features": "Cargo defaults", "profile": "release",
                "rust": RUST, "go": go_version(), "wix": "5.0.2", "inputs": contract()}
    for key, value in expected.items():
        require(manifest.get(key) == value, f"Manifest mismatch: {key}")
    require(set(manifest["files"]) == set(FILES), "Unexpected or missing manifest files")


def unpack_verified(data, artifact, run, destination):
    verify_artifact(artifact, run)
    require(f"sha256:{sha(data)}" == artifact["digest"], "Archive digest mismatch")
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        names = archive.namelist()
        require(len(names) == 4 and set(names) == {*FILES, "provenance.json"}, "Unsafe or unexpected archive entries")
        require(all(i.file_size < 250_000_000 for i in archive.infolist()), "Oversized archive entry")
        manifest = json.loads(archive.read("provenance.json"))
        verify_manifest(manifest, run)
        payload = {name: archive.read(name) for name in FILES}
        for name, content in payload.items():
            require(manifest["files"][name] == {"size": len(content), "sha256": sha(content)} and len(content) > 0,
                    f"Payload mismatch: {name}")
        # Do not extract anything until every entry and every digest is validated.
        destination.mkdir(parents=True, exist_ok=False)
        for name, content in payload.items():
            (destination / name).write_bytes(content)
        (destination / "provenance.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def stage():
    env = os.environ
    require(env.get("GITHUB_REPOSITORY") == REPOSITORY and env.get("GITHUB_REPOSITORY_ID") == str(REPOSITORY_ID), "Wrong repository")
    require(env.get("GITHUB_EVENT_NAME") == "push" and env.get("GITHUB_REF") == "refs/heads/main", "Only main push produces release candidates")
    commit = command("git", "rev-parse", "HEAD").decode().strip()
    require(commit == env["GITHUB_SHA"], "Checkout differs from run commit")
    require(not command("git", "status", "--porcelain", "--untracked-files=no").strip(), "Tracked sources changed during build")
    rust = command("rustc", "-Vv").decode()
    require(rust.startswith(f"rustc {RUST} ") and f"host: {TARGET}" in rust, "Wrong Rust toolchain/host")
    require(command("go", "version").decode().strip() == f"go version go{go_version()} windows/amd64", "Wrong Go toolchain/host")
    require(not env.get("CARGO_BUILD_TARGET") and not env.get("RUSTFLAGS") and not env.get("CARGO_ENCODED_RUSTFLAGS"), "Unexpected target/flags override")
    require(command("go", "env", "GOOS", "GOARCH").decode().split() == ["windows", "amd64"], "Wrong Go target")
    PROMOTION.mkdir(parents=True, exist_ok=False)
    for name in ASSETS:
        data = (ROOT / "dist" / name).read_bytes()
        require(data, f"Empty asset: {name}")
        (PROMOTION / name).write_bytes(data)
    (PROMOTION / "release-notes.md").write_bytes((ROOT / "packaging/release-notes.md").read_bytes())
    manifest = {"schema": 1, "repository": REPOSITORY, "repository_id": REPOSITORY_ID,
                "workflow": WORKFLOW, "workflow_id": WORKFLOW_ID, "run_id": int(env["GITHUB_RUN_ID"]),
                "run_attempt": int(env["GITHUB_RUN_ATTEMPT"]), "commit": commit, "version": version(),
                "target": TARGET, "features": "Cargo defaults", "profile": "release", "rust": RUST,
                "go": go_version(), "wix": "5.0.2", "inputs": contract(),
                "files": {name: {"size": (PROMOTION / name).stat().st_size, "sha256": sha((PROMOTION / name).read_bytes())} for name in FILES}}
    (PROMOTION / "provenance.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps(manifest, indent=2))


def prepare(run_id=None, destination=PROMOTION):
    commit = command("git", "rev-parse", "HEAD").decode().strip()
    publishing = run_id is None
    if run_id is None:
        require(os.environ.get("GITHUB_EVENT_NAME") == "push" and
                os.environ.get("GITHUB_REF") == f"refs/tags/v{version()}" and
                os.environ.get("GITHUB_SHA") == commit and
                os.environ.get("GITHUB_REPOSITORY") == REPOSITORY, "Tag/version/checkout mismatch")
        runs = pages(f"actions/workflows/{WORKFLOW_ID}/runs?head_sha={commit}&event=push&branch=main", "workflow_runs")
        require(runs, "No exact-commit main producer; push main and wait for green CI first")
        # A newer failed/incomplete run must not be hidden by an older green run.
        run_id = max(runs, key=lambda r: r["id"])["id"]
    run = api(f"actions/runs/{int(run_id)}")
    verify_run(run, commit)
    jobs = pages(f"actions/runs/{run['id']}/attempts/{run['run_attempt']}/jobs", "jobs")
    windows = [j for j in jobs if j["name"] == "windows"]
    require(len(windows) == 1 and windows[0]["conclusion"] == "success", "Windows gate missing or unsuccessful")
    artifacts = pages(f"actions/runs/{run['id']}/artifacts", "artifacts")
    candidates = [a for a in artifacts if a["name"] == f"release-{commit}-{run['run_attempt']}"]
    require(len(candidates) == 1, "Missing or ambiguous immutable candidate; no rebuild fallback")
    artifact = candidates[0]
    verify_artifact(artifact, run)
    data = command("gh", "api", f"repos/{REPOSITORY}/actions/artifacts/{artifact['id']}/zip")
    manifest = unpack_verified(data, artifact, run, destination)
    if publishing:
        verify_tag(commit)
    print(json.dumps({"producer": run["html_url"], "artifact_id": artifact["id"], "archive_digest": artifact["digest"],
                      "commit": commit, "files": manifest["files"]}, indent=2))
    return manifest


def published():
    manifest = json.loads((PROMOTION / "provenance.json").read_text())
    verify_tag(manifest["commit"])
    release = api(f"releases/tags/v{version()}")
    require(not release["draft"] and not release["prerelease"], "Release is not public/stable")
    require({a["name"] for a in release["assets"]} == set(ASSETS) and len(release["assets"]) == 2, "Release assets differ")
    for asset in release["assets"]:
        expected = manifest["files"][asset["name"]]
        require(asset["digest"] == f"sha256:{expected['sha256']}" and asset["size"] == expected["size"], "Published metadata mismatch")
        with urllib.request.urlopen(asset["browser_download_url"], timeout=120) as response:
            content = response.read()
        require(len(content) == expected["size"] and sha(content) == expected["sha256"], "Public download bytes differ")
    print("Public MSI and ZIP bytes match the trusted main candidate; no installation performed.")


if __name__ == "__main__":
    mode = sys.argv[1]
    if mode == "version":
        print(version())
    elif mode == "stage":
        stage()
    elif mode == "prepare":
        prepare()
    elif mode == "verify-run":
        # Read-only rehearsal; writes only to the explicitly supplied fresh directory.
        prepare(int(sys.argv[2]), Path(sys.argv[3]))
    elif mode == "published":
        published()
    else:
        raise ValueError("Unknown command")
