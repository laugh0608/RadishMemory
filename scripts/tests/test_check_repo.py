from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "check-repo.py"
SPEC = importlib.util.spec_from_file_location("check_repo", SCRIPT_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("unable to load check-repo.py")
CHECK_REPO = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK_REPO)


class MarkdownLinkChecks(unittest.TestCase):
    def test_accepts_existing_relative_and_external_links(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target = root / "target.md"
            source = root / "README.md"
            target.write_text("# Target\n", encoding="utf-8")
            source.write_text(
                "[local](target.md) [external](https://example.com)\n",
                encoding="utf-8",
            )
            errors: list[str] = []

            CHECK_REPO.check_markdown_links(root, [source, target], errors)

            self.assertEqual([], errors)

    def test_rejects_missing_relative_link(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "README.md"
            source.write_text("[missing](docs/missing.md)\n", encoding="utf-8")
            errors: list[str] = []

            CHECK_REPO.check_markdown_links(root, [source], errors)

            self.assertEqual(
                ["broken relative link: README.md -> docs/missing.md"],
                errors,
            )

    def test_rejects_link_escaping_repository(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "README.md"
            source.write_text("[outside](../outside.md)\n", encoding="utf-8")
            errors: list[str] = []

            CHECK_REPO.check_markdown_links(root, [source], errors)

            self.assertEqual(
                ["relative link escapes repository: README.md -> ../outside.md"],
                errors,
            )


class TextAndDataSafetyChecks(unittest.TestCase):
    def test_rejects_crlf_and_trailing_whitespace(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "sample.py"
            source.write_bytes(b"value = 1 \r\n")
            errors: list[str] = []

            CHECK_REPO.check_text_files(root, [source], errors)

            self.assertIn("text file must use LF line endings: sample.py", errors)
            self.assertIn("trailing whitespace: sample.py:1", errors)

    def test_rejects_blank_line_at_eof(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "sample.md"
            source.write_text("content\n\n", encoding="utf-8")
            errors: list[str] = []

            CHECK_REPO.check_text_files(root, [source], errors)

            self.assertEqual(
                ["text file has a blank line at EOF: sample.md"],
                errors,
            )

    def test_rejects_personal_memory_root(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "vault" / "profile.json"
            source.parent.mkdir()
            source.write_text("{}\n", encoding="utf-8")
            errors: list[str] = []

            CHECK_REPO.check_paths_sizes_and_safety(root, [source], errors)

            self.assertEqual(
                ["personal-memory or local-data path must not be committed: vault/profile.json"],
                errors,
            )

    def test_allows_environment_example(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / ".env.example"
            source.write_text("PROVIDER_KEY=replace-me\n", encoding="utf-8")
            errors: list[str] = []

            CHECK_REPO.check_paths_sizes_and_safety(root, [source], errors)

            self.assertEqual([], errors)

    def test_rejects_key_file_even_outside_ignored_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "fixtures" / "device.key"
            source.parent.mkdir()
            source.write_text("synthetic\n", encoding="utf-8")
            errors: list[str] = []

            CHECK_REPO.check_paths_sizes_and_safety(root, [source], errors)

            self.assertEqual(
                ["sensitive key or local database file must not be committed: fixtures/device.key"],
                errors,
            )

    def test_rejects_database_sidecar_when_forced_into_git(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            source = root / "fixtures" / "memory.db-wal"
            source.parent.mkdir()
            source.write_bytes(b"synthetic")
            errors: list[str] = []

            CHECK_REPO.check_paths_sizes_and_safety(root, [source], errors)

            self.assertEqual(
                ["sensitive key or local database file must not be committed: fixtures/memory.db-wal"],
                errors,
            )


class GovernanceContractChecks(unittest.TestCase):
    def test_commit_subject_contract(self) -> None:
        self.assertTrue(CHECK_REPO.is_allowed_commit_subject("docs(memory): clarify states"))
        self.assertTrue(
            CHECK_REPO.is_allowed_commit_subject("Merge pull request #12 from proposal")
        )
        self.assertFalse(CHECK_REPO.is_allowed_commit_subject("update policy"))

    def test_required_file_contract_reports_missing_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            errors: list[str] = []

            CHECK_REPO.check_required_files(Path(temp_dir), errors)

            self.assertIn("missing required file: SECURITY.md", errors)

    def test_rust_workspace_contract_rejects_a_fourth_package(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target = root / "crates/extra/Cargo.toml"
            target.parent.mkdir(parents=True)
            target.write_text(
                '[package]\nname = "extra"\nversion = "0.1.0"\n',
                encoding="utf-8",
            )
            errors: list[str] = []

            CHECK_REPO.check_rust_workspace_contract(root, errors)

            self.assertTrue(
                any(
                    error.startswith(
                        "Rust workspace must contain only the root manifest and the three M0 package manifests"
                    )
                    for error in errors
                )
            )

    def test_rust_workspace_contract_rejects_a_floating_toolchain(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target = root / "rust-toolchain.toml"
            target.write_text(
                '[toolchain]\nchannel = "stable"\n',
                encoding="utf-8",
            )
            errors: list[str] = []

            CHECK_REPO.check_rust_workspace_contract(root, errors)

            self.assertIn(
                "rust-toolchain.toml must pin Rust 1.96.0 with the minimal profile, clippy, and rustfmt",
                errors,
            )

    def test_rust_workspace_contract_rejects_unreviewed_lock_package(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            for name, content in CHECK_REPO.EXPECTED_CARGO_MANIFESTS.items():
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(content, encoding="utf-8")
            lock_text = (CHECK_REPO.REPO_ROOT / "Cargo.lock").read_text(encoding="utf-8")
            (root / "Cargo.lock").write_text(
                lock_text
                + '\n[[package]]\nname = "unreviewed"\nversion = "1.0.0"\n'
                + f'source = "{CHECK_REPO.CRATES_IO_SOURCE}"\n'
                + 'checksum = "'
                + "0" * 64
                + '"\n',
                encoding="utf-8",
            )
            errors: list[str] = []

            CHECK_REPO.check_rust_workspace_contract(root, errors)

            self.assertIn(
                "Cargo.lock differs from the reviewed M0-I02 dependency set",
                errors,
            )

    def test_agent_mirrors_must_match(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            (root / "AGENTS.md").write_text("one\n", encoding="utf-8")
            (root / "CLAUDE.md").write_text("two\n", encoding="utf-8")
            errors: list[str] = []

            CHECK_REPO.check_agent_contract(root, errors)

            self.assertIn("AGENTS.md and CLAUDE.md must remain identical", errors)

    def test_ruleset_requires_extra_approval_for_unattributed_changes(self) -> None:
        source = CHECK_REPO.REPO_ROOT / ".github/rulesets/master-protection.json"
        ruleset = json.loads(source.read_text(encoding="utf-8"))
        pull_request = next(
            rule for rule in ruleset["rules"] if rule["type"] == "pull_request"
        )
        pull_request["parameters"][
            "require_extra_approval_for_unattributed_changes"
        ] = False

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target = root / ".github/rulesets/master-protection.json"
            target.parent.mkdir(parents=True)
            target.write_text(json.dumps(ruleset), encoding="utf-8")
            errors: list[str] = []

            CHECK_REPO.check_ruleset_contract(root, errors)

            self.assertIn(
                "unattributed Copilot changes must require extra approval",
                errors,
            )

    def test_workflow_contract_requires_windows_rust_quality(self) -> None:
        source = CHECK_REPO.REPO_ROOT / ".github/workflows/pr-check.yml"
        workflow = source.read_text(encoding="utf-8").replace(
            "          - platform: Windows\n            os: windows-latest\n",
            "",
        )

        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target = root / ".github/workflows/pr-check.yml"
            target.parent.mkdir(parents=True)
            target.write_text(workflow, encoding="utf-8")
            errors: list[str] = []

            CHECK_REPO.check_workflow_contract(root, errors)

            self.assertIn(
                "PR workflow is missing contract fragment: - platform: Windows\n            os: windows-latest",
                errors,
            )

    def test_m0_contract_reports_missing_fragment(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target = root / "docs/adr/0002-m0-local-memory-loop.md"
            target.parent.mkdir(parents=True)
            target.write_text("# M0 Local Memory Loop\n", encoding="utf-8")
            errors: list[str] = []

            CHECK_REPO.check_m0_contract(root, errors)

            self.assertIn(
                "docs/adr/0002-m0-local-memory-loop.md is missing M0 contract fragment: SourceArtifact",
                errors,
            )

    def test_m0_schema_contract_reports_missing_fragment(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target = root / "docs/schema/m0-canonical-schema.md"
            target.parent.mkdir(parents=True)
            target.write_text("# M0 Canonical Schema\n", encoding="utf-8")
            errors: list[str] = []

            CHECK_REPO.check_m0_schema_contract(root, errors)

            self.assertIn(
                "docs/schema/m0-canonical-schema.md is missing M0 schema contract fragment: radishmemory.m0/1",
                errors,
            )

    def test_m0_fixture_contract_reports_missing_fragment(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target = root / "docs/evaluation/m0-fixture-contract.md"
            target.parent.mkdir(parents=True)
            target.write_text("# M0 Fixture Contract\n", encoding="utf-8")
            errors: list[str] = []

            CHECK_REPO.check_m0_fixture_contract(root, errors)

            self.assertIn(
                "docs/evaluation/m0-fixture-contract.md is missing M0 fixture contract fragment: radishmemory.m0-fixture/1",
                errors,
            )

    def test_sync_trust_contract_reports_missing_fragment(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target = root / "docs/adr/0003-zero-knowledge-sync-first.md"
            target.parent.mkdir(parents=True)
            target.write_text("# Sync trust\n", encoding="utf-8")
            errors: list[str] = []

            CHECK_REPO.check_sync_trust_contract(root, errors)

            self.assertIn(
                "docs/adr/0003-zero-knowledge-sync-first.md is missing sync trust contract fragment: 模式 B：零知识同步服务",
                errors,
            )

    def test_radishmind_entry_contract_reports_missing_fragment(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target = root / "docs/adr/0004-radishmind-optional-gateway-entry.md"
            target.parent.mkdir(parents=True)
            target.write_text("# Gateway entry\n", encoding="utf-8")
            errors: list[str] = []

            CHECK_REPO.check_radishmind_entry_contract(root, errors)

            self.assertIn(
                "docs/adr/0004-radishmind-optional-gateway-entry.md is missing RadishMind entry contract fragment: 首次接入只使用模型网关能力",
                errors,
            )

    def test_implementation_stack_contract_reports_missing_fragment(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            target = root / "docs/adr/0005-m0-implementation-stack.md"
            target.parent.mkdir(parents=True)
            target.write_text("# M0 stack\n", encoding="utf-8")
            errors: list[str] = []

            CHECK_REPO.check_implementation_stack_contract(root, errors)

            self.assertIn(
                "docs/adr/0005-m0-implementation-stack.md is missing implementation stack contract fragment: Rust 2024 edition",
                errors,
            )


if __name__ == "__main__":
    unittest.main()
