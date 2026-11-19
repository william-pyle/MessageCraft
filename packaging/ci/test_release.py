"""Synthetic promotion tests: no credentials, network, app launch or installation."""
import copy
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import release as r


class PromotionTests(unittest.TestCase):
    def setUp(self):
        self.commit = "a" * 40
        self.run = {"id": 123, "run_attempt": 2, "repository": {"id": r.REPOSITORY_ID, "full_name": r.REPOSITORY},
                    "head_repository": {"id": r.REPOSITORY_ID}, "workflow_id": r.WORKFLOW_ID, "path": r.WORKFLOW,
                    "head_sha": self.commit, "event": "push", "head_branch": "main", "status": "completed",
                    "conclusion": "success", "html_url": "https://github.com/example/run/123"}
        self.files = {name: f"synthetic {name}".encode() for name in r.FILES}
        self.manifest = {"schema": 1, "repository": r.REPOSITORY, "repository_id": r.REPOSITORY_ID,
                         "workflow": r.WORKFLOW, "workflow_id": r.WORKFLOW_ID, "run_id": 123, "run_attempt": 2,
                         "commit": self.commit, "version": r.version(), "target": r.TARGET, "profile": "release",
                         "features": "Cargo defaults", "rust": r.RUST, "go": r.go_version(), "wix": "5.0.2",
                         "inputs": r.contract(), "files": {n: {"size": len(b), "sha256": r.sha(b)} for n, b in self.files.items()}}
        self.artifact = {"id": 456, "name": f"release-{self.commit}-2", "expired": False,
                         "workflow_run": {"id": 123, "head_sha": self.commit, "head_branch": "main",
                                          "repository_id": r.REPOSITORY_ID, "head_repository_id": r.REPOSITORY_ID}}

    def archive(self, manifest=None, files=None):
        stream = io.BytesIO()
        with zipfile.ZipFile(stream, "w") as archive:
            for name, value in (self.files if files is None else files).items():
                archive.writestr(name, value)
            archive.writestr("provenance.json", json.dumps(manifest or self.manifest))
        data = stream.getvalue()
        self.artifact["digest"] = f"sha256:{r.sha(data)}"
        return data

    def test_valid_exact_commit_candidate(self):
        r.verify_run(self.run, self.commit)
        with tempfile.TemporaryDirectory() as tmp:
            destination = Path(tmp) / "candidate"
            r.unpack_verified(self.archive(), self.artifact, self.run, destination)
            self.assertEqual(set(p.name for p in destination.iterdir()), {*r.FILES, "provenance.json"})
            for name, data in self.files.items():
                self.assertEqual((destination / name).read_bytes(), data)

    def test_reject_producer_mismatches(self):
        changes = {"repository": {"id": 9, "full_name": r.REPOSITORY}, "head_repository": {"id": 9},
                   "workflow_id": 9, "path": "other.yml", "head_sha": "b" * 40, "event": "pull_request",
                   "head_branch": "feature", "status": "in_progress", "conclusion": "failure", "run_attempt": 0}
        for key, value in changes.items():
            with self.subTest(key=key), self.assertRaises(ValueError):
                r.verify_run({**self.run, key: value}, self.commit)

    def test_reject_artifact_mismatches(self):
        self.archive()
        for key, value in {"name": f"release-{self.commit}-1", "expired": True, "digest": None}.items():
            with self.subTest(key=key), self.assertRaises(ValueError):
                r.verify_artifact({**self.artifact, key: value}, self.run)
        for key in self.artifact["workflow_run"]:
            changed = copy.deepcopy(self.artifact)
            changed["workflow_run"][key] = "wrong"
            with self.subTest(key=key), self.assertRaises(ValueError):
                r.verify_artifact(changed, self.run)

    def test_reject_every_manifest_identity_mismatch(self):
        for key in self.manifest.keys() - {"files"}:
            with self.subTest(key=key), self.assertRaises(ValueError):
                r.verify_manifest({**self.manifest, key: "wrong"}, self.run)

    def test_reject_archive_digest_before_writes(self):
        data = self.archive() + b"tampered"
        with tempfile.TemporaryDirectory() as tmp:
            dest = Path(tmp) / "candidate"
            with self.assertRaisesRegex(ValueError, "Archive digest"):
                r.unpack_verified(data, self.artifact, self.run, dest)
            self.assertFalse(dest.exists())

    def test_reject_missing_extra_or_traversal_entries_before_writes(self):
        variants = [dict(list(self.files.items())[1:]), {**self.files, "extra": b"x"},
                    {**self.files, "../outside": b"x"}]
        for files in variants:
            with self.subTest(names=list(files)), tempfile.TemporaryDirectory() as tmp:
                dest = Path(tmp) / "candidate"
                with self.assertRaisesRegex(ValueError, "archive entries"):
                    r.unpack_verified(self.archive(files=files), self.artifact, self.run, dest)
                self.assertFalse(dest.exists())

    def test_reject_changed_payload_and_size(self):
        for field, value in {"sha256": "0" * 64, "size": 1}.items():
            changed = copy.deepcopy(self.manifest)
            changed["files"][r.ASSETS[0]][field] = value
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                dest = Path(tmp) / "candidate"
                with self.assertRaisesRegex(ValueError, "Payload mismatch"):
                    r.unpack_verified(self.archive(manifest=changed), self.artifact, self.run, dest)
                self.assertFalse(dest.exists())

    def test_refuse_overwrite(self):
        with tempfile.TemporaryDirectory() as tmp, self.assertRaises(FileExistsError):
            r.unpack_verified(self.archive(), self.artifact, self.run, Path(tmp))

    def test_prepare_requires_successful_windows_gate_and_unique_artifact(self):
        for jobs, artifacts in [([], []), ([{"name": "windows", "conclusion": "skipped"}], []),
                                ([{"name": "windows", "conclusion": "success"}], []),
                                ([{"name": "windows", "conclusion": "success"}], [self.artifact, self.artifact])]:
            with self.subTest(jobs=jobs, count=len(artifacts)), patch.object(r, "command", return_value=self.commit.encode()), \
                    patch.object(r, "api", return_value=self.run), patch.object(r, "pages", side_effect=[jobs, artifacts]), \
                    self.assertRaises(ValueError):
                r.prepare(123)

    def test_prepare_downloads_selected_id_and_checks_bytes(self):
        data = self.archive()
        with tempfile.TemporaryDirectory() as tmp, patch.object(r, "command", side_effect=[self.commit.encode(), data]) as cmd, \
                patch.object(r, "api", return_value=self.run), \
                patch.object(r, "pages", side_effect=[[{"name": "windows", "conclusion": "success"}], [self.artifact]]):
            r.prepare(123, Path(tmp) / "candidate")
            self.assertEqual(cmd.call_args.args, ("gh", "api", f"repos/{r.REPOSITORY}/actions/artifacts/456/zip"))

    def test_no_producer_and_newer_failed_producer_fail_closed(self):
        env = {"GITHUB_EVENT_NAME": "push", "GITHUB_REF": f"refs/tags/v{r.version()}",
               "GITHUB_SHA": self.commit, "GITHUB_REPOSITORY": r.REPOSITORY}
        with patch.dict(r.os.environ, env), patch.object(r, "command", return_value=self.commit.encode()), \
                patch.object(r, "pages", return_value=[]), self.assertRaisesRegex(ValueError, "No exact-commit"):
            r.prepare()
        with patch.dict(r.os.environ, env), patch.object(r, "command", return_value=self.commit.encode()), \
                patch.object(r, "pages", return_value=[{"id": 1}, {"id": 2}]), \
                patch.object(r, "api", return_value={**self.run, "conclusion": "failure"}) as api, \
                self.assertRaisesRegex(ValueError, "not green"):
            r.prepare()
        api.assert_called_once_with("actions/runs/2")

    def test_tag_version_mismatch(self):
        with patch.dict(r.os.environ, {"GITHUB_REF": "refs/tags/v0.0.0"}), \
                patch.object(r, "command", return_value=self.commit.encode()), self.assertRaises(ValueError):
            r.prepare()

    def test_remote_tag_must_resolve_to_exact_commit(self):
        with patch.object(r, "api", side_effect=[{"object": {"type": "tag", "sha": "b" * 40}},
                                                {"object": {"type": "commit", "sha": self.commit}}]):
            r.verify_tag(self.commit)
        with patch.object(r, "api", return_value={"object": {"type": "commit", "sha": "b" * 40}}), \
                self.assertRaisesRegex(ValueError, "tag commit mismatch"):
            r.verify_tag(self.commit)

    def test_public_final_bytes_and_metadata(self):
        release = {"draft": False, "prerelease": False, "assets": [
            {"name": n, "size": len(self.files[n]), "digest": f"sha256:{r.sha(self.files[n])}",
             "browser_download_url": f"https://example.invalid/{n}"} for n in r.ASSETS]}
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "provenance.json").write_text(json.dumps(self.manifest))
            for bad in [False, True]:
                responses = [io.BytesIO(b"tampered" if bad else self.files[n]) for n in r.ASSETS]
                with self.subTest(tampered=bad), patch.object(r, "PROMOTION", Path(tmp)), \
                        patch.object(r, "verify_tag"), patch.object(r, "api", return_value=release), \
                        patch.object(r.urllib.request, "urlopen", side_effect=responses):
                    if bad:
                        with self.assertRaisesRegex(ValueError, "Public download bytes"):
                            r.published()
                    else:
                        r.published()
            for key, value in {"digest": "sha256:" + "0" * 64, "size": 1}.items():
                wrong = copy.deepcopy(release)
                wrong["assets"][0][key] = value
                with self.subTest(key=key), patch.object(r, "PROMOTION", Path(tmp)), \
                        patch.object(r, "verify_tag"), patch.object(r, "api", return_value=wrong), \
                        self.assertRaisesRegex(ValueError, "metadata mismatch"):
                    r.published()


if __name__ == "__main__":
    unittest.main()
