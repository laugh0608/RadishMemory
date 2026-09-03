#!/usr/bin/env python3
"""Dependency-free RadishMemory repository governance and hygiene checks."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from urllib.parse import unquote


REPO_ROOT = Path(__file__).resolve().parents[1]
MAX_PATH_LENGTH = 180
MAX_FILE_BYTES = 10 * 1024 * 1024
MAX_AGENT_LINES = 180
MAX_ACTIVE_DOC_LINES = 500

REQUIRED_FILES = (
    ".editorconfig",
    ".gitattributes",
    ".gitignore",
    ".github/ISSUE_TEMPLATE/bug-report.yml",
    ".github/ISSUE_TEMPLATE/change-proposal.yml",
    ".github/ISSUE_TEMPLATE/config.yml",
    ".github/PULL_REQUEST_TEMPLATE.md",
    ".github/rulesets/README.md",
    ".github/rulesets/master-protection.json",
    ".github/workflows/pr-check.yml",
    "AGENTS.md",
    "CLAUDE.md",
    "CODE_OF_CONDUCT.md",
    "CONTRIBUTING.md",
    "LICENSE",
    "README.md",
    "SECURITY.md",
    "THIRD_PARTY_NOTICES.md",
    "Cargo.lock",
    "Cargo.toml",
    "apps/radishmemory-desktop/Cargo.toml",
    "apps/radishmemory-desktop/src/controller.rs",
    "apps/radishmemory-desktop/src/error.rs",
    "apps/radishmemory-desktop/src/lib.rs",
    "apps/radishmemory-desktop/src/main.rs",
    "apps/radishmemory-desktop/src/paths.rs",
    "apps/radishmemory-desktop/src/picker.rs",
    "apps/radishmemory-desktop/src/profile.rs",
    "apps/radishmemory-desktop/src/runtime.rs",
    "apps/radishmemory-desktop/src/ui.rs",
    "apps/radishmemory-m0/Cargo.toml",
    "apps/radishmemory-m0/src/main.rs",
    "crates/radishmemory-application/Cargo.toml",
    "crates/radishmemory-application/src/error.rs",
    "crates/radishmemory-application/src/lib.rs",
    "crates/radishmemory-application/tests/local_library.rs",
    "crates/radishmemory-core/Cargo.toml",
    "crates/radishmemory-core/src/canonical_json.rs",
    "crates/radishmemory-core/src/context.rs",
    "crates/radishmemory-core/src/deletion.rs",
    "crates/radishmemory-core/src/digest.rs",
    "crates/radishmemory-core/src/error.rs",
    "crates/radishmemory-core/src/invariants.rs",
    "crates/radishmemory-core/src/library.rs",
    "crates/radishmemory-core/src/lib.rs",
    "crates/radishmemory-core/src/memory.rs",
    "crates/radishmemory-core/src/model.rs",
    "crates/radishmemory-core/src/ports.rs",
    "crates/radishmemory-core/src/source.rs",
    "crates/radishmemory-core/src/temporal.rs",
    "crates/radishmemory-core/tests/m0_canonical_objects.rs",
    "crates/radishmemory-core/tests/m0_invariants.rs",
    "crates/radishmemory-core/tests/m0_primitives.rs",
    "crates/radishmemory-file-entry/Cargo.toml",
    "crates/radishmemory-file-entry/src/error.rs",
    "crates/radishmemory-file-entry/src/lib.rs",
    "crates/radishmemory-file-entry/tests/file_snapshot.rs",
    "crates/radishmemory-source-vault/Cargo.toml",
    "crates/radishmemory-source-vault/src/aad.rs",
    "crates/radishmemory-source-vault/src/crypto.rs",
    "crates/radishmemory-source-vault/src/error.rs",
    "crates/radishmemory-source-vault/src/lib.rs",
    "crates/radishmemory-source-vault/src/random.rs",
    "crates/radishmemory-sqlite/Cargo.toml",
    "crates/radishmemory-sqlite/migrations/0001_sqlite_entry.sql",
    "crates/radishmemory-sqlite/migrations/0002_source_storage.sql",
    "crates/radishmemory-sqlite/migrations/0003_memory_storage.sql",
    "crates/radishmemory-sqlite/migrations/0004_local_recall.sql",
    "crates/radishmemory-sqlite/migrations/0005_local_deletion.sql",
    "crates/radishmemory-sqlite/migrations/0006_source_capture.sql",
    "crates/radishmemory-sqlite/src/capability.rs",
    "crates/radishmemory-sqlite/src/error.rs",
    "crates/radishmemory-sqlite/src/lib.rs",
    "crates/radishmemory-sqlite/src/migration.rs",
    "crates/radishmemory-sqlite/src/memory_store.rs",
    "crates/radishmemory-sqlite/src/source_store.rs",
    "crates/radishmemory-sqlite/src/source_capture.rs",
    "crates/radishmemory-sqlite/src/source_catalog.rs",
    "crates/radishmemory-sqlite/tests/memory_store.rs",
    "crates/radishmemory-sqlite/tests/source_vault.rs",
    "crates/radishmemory-sqlite/tests/source_capture.rs",
    "crates/radishmemory-sqlite/tests/sqlite_entry.rs",
    "crates/radishmemory-sqlite/tests/support/mod.rs",
    "docs/README.md",
    "docs/adr/0001-branch-and-pr-governance.md",
    "docs/adr/0002-m0-local-memory-loop.md",
    "docs/adr/0003-zero-knowledge-sync-first.md",
    "docs/adr/0004-radishmind-optional-gateway-entry.md",
    "docs/adr/0005-m0-implementation-stack.md",
    "docs/adr/0006-phase1-text-markdown-file-entry.md",
    "docs/adr/0007-phase1-local-library-host.md",
    "docs/adr/0008-phase1-encrypted-source-vault.md",
    "docs/implementation/phase1-encrypted-source-vault-dependency-review.md",
    "docs/implementation/phase1-source-vault-portable-crypto.md",
    "docs/architecture.md",
    "docs/evaluation/m0-fixture-contract.md",
    "docs/evaluation/m0-local-memory-loop.md",
    "docs/governance/agent-collaboration.md",
    "docs/governance/repository-governance.md",
    "docs/implementation/m0-rust-dependency-baseline.md",
    "docs/implementation/phase1-desktop-dependency-review.md",
    "docs/implementation/phase1-linux-host-acceptance.md",
    "docs/implementation/phase1-macos-host-acceptance.md",
    "docs/implementation/phase1-third-party-notices.md",
    "docs/implementation/phase1-windows-host-acceptance.md",
    "docs/memory-model.md",
    "docs/mvp-roadmap.md",
    "docs/privacy-threat-model.md",
    "docs/product-scope.md",
    "docs/radishmind-boundary.md",
    "docs/references.md",
    "docs/schema/m0-canonical-schema.md",
    "docs/status/current.md",
    "fixtures/m0/local-memory-loop.v1.json",
    "rust-toolchain.toml",
    "scripts/check-m0-fixtures.py",
    "scripts/check-repo.ps1",
    "scripts/check-repo.py",
    "scripts/check-repo.sh",
    "scripts/generate-third-party-notices.py",
    "scripts/tests/test_check_repo.py",
    "scripts/tests/test_check_m0_fixtures.py",
    "third_party/licenses/Apache-2.0.txt",
    "third_party/licenses/BSL-1.0.txt",
    "third_party/licenses/ISC.txt",
    "third_party/licenses/MIT.txt",
    "third_party/licenses/MPL-2.0.txt",
    "third_party/licenses/OFL-1.1.txt",
    "third_party/licenses/README.md",
    "third_party/licenses/SQLite-public-domain.txt",
    "third_party/licenses/Ubuntu-font-1.0.txt",
    "third_party/licenses/Unicode-3.0.txt",
    "third_party/licenses/Zlib.txt",
    "third_party/licenses/epaint-default-fonts-notices.txt",
)

TEXT_SUFFIXES = {
    ".c",
    ".cc",
    ".cfg",
    ".conf",
    ".cpp",
    ".cs",
    ".css",
    ".dart",
    ".go",
    ".h",
    ".hpp",
    ".html",
    ".ini",
    ".java",
    ".js",
    ".json",
    ".jsonc",
    ".jsx",
    ".kt",
    ".kts",
    ".md",
    ".mjs",
    ".proto",
    ".ps1",
    ".py",
    ".rs",
    ".scss",
    ".sh",
    ".sql",
    ".swift",
    ".toml",
    ".ts",
    ".tsx",
    ".txt",
    ".xml",
    ".yaml",
    ".yml",
}

TEXT_NAMES = {
    ".dockerignore",
    ".editorconfig",
    ".gitattributes",
    ".gitignore",
    "Dockerfile",
    "Cargo.lock",
    "LICENSE",
    "Makefile",
}

EXPECTED_CARGO_MANIFESTS = {
    "Cargo.toml": """[workspace]
members = [
  \"apps/radishmemory-desktop\",
  \"apps/radishmemory-m0\",
  \"crates/radishmemory-application\",
  \"crates/radishmemory-core\",
  \"crates/radishmemory-file-entry\",
  \"crates/radishmemory-source-vault\",
  \"crates/radishmemory-sqlite\",
]
resolver = \"3\"

[workspace.package]
version = \"0.1.0\"
edition = \"2024\"
rust-version = \"1.96.0\"
license-file = \"LICENSE\"
publish = false

[workspace.dependencies]
aead-stream = { version = \"=0.6.0\", default-features = false, features = [\"alloc\"] }
chacha20poly1305 = { version = \"=0.11.0\", default-features = false, features = [\"alloc\", \"zeroize\"] }
directories = \"=6.0.0\"
eframe = { version = \"=0.36.1\", default-features = false, features = [\"accesskit\", \"default_fonts\", \"wayland\", \"wgpu\", \"x11\"] }
getrandom = { version = \"=0.4.3\", default-features = false }
radishmemory-application = { path = \"crates/radishmemory-application\", version = \"=0.1.0\" }
radishmemory-core = { path = \"crates/radishmemory-core\", version = \"=0.1.0\" }
radishmemory-file-entry = { path = \"crates/radishmemory-file-entry\", version = \"=0.1.0\" }
radishmemory-source-vault = { path = \"crates/radishmemory-source-vault\", version = \"=0.1.0\" }
radishmemory-sqlite = { path = \"crates/radishmemory-sqlite\", version = \"=0.1.0\" }
rusqlite = { version = \"0.40.2\", default-features = false, features = [\"bundled\"] }
rfd = { version = \"=0.17.2\", default-features = false, features = [\"xdg-portal\", \"wayland\"] }
serde_json = { version = \"1.0.151\", default-features = false, features = [\"arbitrary_precision\", \"std\"] }
sha2 = { version = \"0.11.0\", default-features = false }
time = { version = \"0.3.55\", default-features = false, features = [\"formatting\", \"parsing\", \"std\"] }
unicode-normalization = { version = \"0.1.25\", default-features = false, features = [\"std\"] }
zeroize = { version = \"=1.9.0\", default-features = false, features = [\"alloc\"] }

[workspace.lints.rust]
unsafe_code = \"forbid\"
unused_crate_dependencies = \"deny\"
""",
    "apps/radishmemory-desktop/Cargo.toml": """[package]
name = \"radishmemory-desktop\"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license-file.workspace = true
publish.workspace = true

[dependencies]
directories.workspace = true
eframe.workspace = true
getrandom.workspace = true
radishmemory-application.workspace = true
rfd.workspace = true
time.workspace = true

[lints]
workspace = true
""",
    "apps/radishmemory-m0/Cargo.toml": """[package]
name = \"radishmemory-m0\"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license-file.workspace = true
publish.workspace = true

[lints]
workspace = true

[dependencies]
radishmemory-core.workspace = true
radishmemory-sqlite = { workspace = true, features = ["fixture-runner"] }
serde_json.workspace = true
""",
    "crates/radishmemory-application/Cargo.toml": """[package]
name = \"radishmemory-application\"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license-file.workspace = true
publish.workspace = true

[lints]
workspace = true

[dependencies]
radishmemory-core.workspace = true
radishmemory-file-entry.workspace = true
radishmemory-sqlite.workspace = true
""",
    "crates/radishmemory-core/Cargo.toml": """[package]
name = \"radishmemory-core\"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license-file.workspace = true
publish.workspace = true

[lints]
workspace = true

[dependencies]
serde_json.workspace = true
sha2.workspace = true
time.workspace = true
unicode-normalization.workspace = true
""",
    "crates/radishmemory-file-entry/Cargo.toml": """[package]
name = \"radishmemory-file-entry\"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license-file.workspace = true
publish.workspace = true

[lints]
workspace = true

[features]
acceptance-test-support = []

[dependencies]
radishmemory-core.workspace = true
""",
    "crates/radishmemory-source-vault/Cargo.toml": """[package]
name = "radishmemory-source-vault"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license-file.workspace = true
publish.workspace = true

[lints]
workspace = true

[dependencies]
aead-stream.workspace = true
chacha20poly1305.workspace = true
getrandom.workspace = true
sha2.workspace = true
zeroize.workspace = true
""",
    "crates/radishmemory-sqlite/Cargo.toml": """[package]
name = \"radishmemory-sqlite\"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license-file.workspace = true
publish.workspace = true

[lints]
workspace = true

[features]
fixture-runner = []

[dependencies]
radishmemory-core.workspace = true
rusqlite.workspace = true

[dev-dependencies]
radishmemory-file-entry = { workspace = true, features = ["acceptance-test-support"] }
""",
}

EXPECTED_RUST_TOOLCHAIN = """[toolchain]
channel = \"1.96.0\"
components = [\"clippy\", \"rustfmt\"]
profile = \"minimal\"
"""

EXPECTED_REVIEWED_LOCK_PACKAGE_COUNT = 430
EXPECTED_REVIEWED_LOCK_DIGEST = "c8d2e33f72694eedf0a2c44ac21d059826fc8ea039215225f3d70ea68903f80e"
FIRST_PARTY_RUST_PACKAGES = {
    "radishmemory-application",
    "radishmemory-core",
    "radishmemory-desktop",
    "radishmemory-file-entry",
    "radishmemory-m0",
    "radishmemory-source-vault",
    "radishmemory-sqlite",
}
CRATES_IO_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"

FORBIDDEN_DIRECTORY_NAMES = {
    "__pycache__",
    "node_modules",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
}
FORBIDDEN_ROOT_DATA_DIRECTORIES = {
    "backups",
    "exports",
    "local-data",
    "memory-store",
    "sources",
    "sync-state",
    "user-data",
    "vault",
}
FORBIDDEN_SENSITIVE_SUFFIXES = {
    ".db",
    ".db-shm",
    ".db-wal",
    ".jks",
    ".key",
    ".keystore",
    ".kdbx",
    ".p12",
    ".p8",
    ".pem",
    ".pfx",
    ".sqlite",
    ".sqlite3",
}
FORBIDDEN_SENSITIVE_NAMES = {
    "credentials.json",
    "id_ed25519",
    "id_rsa",
    "service-account.json",
    "service_account.json",
    "secrets.json",
}

CONVENTIONAL_COMMIT = re.compile(
    r"^(feat|fix|docs|refactor|test|chore|ci|build|perf|revert)"
    r"(\([a-z0-9._/-]+\))?!?: .+"
)
ALLOWED_MERGE_COMMIT = re.compile(
    r"^Merge (pull request|branch|remote-tracking branch)"
)
MARKDOWN_LINK = re.compile(
    r"!?\[[^\]]*\]\(([^)\s]+)(?:\s+['\"][^'\"]*['\"])?\)"
)


def git(repo_root: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=repo_root,
        check=False,
        capture_output=True,
        text=True,
    )


def repository_files(repo_root: Path) -> list[Path]:
    result = git(
        repo_root,
        "ls-files",
        "--cached",
        "--others",
        "--exclude-standard",
        "-z",
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "git ls-files failed")
    return sorted(
        (repo_root / item for item in result.stdout.split("\0") if item),
        key=lambda path: path.as_posix(),
    )


def relative(repo_root: Path, path: Path) -> str:
    return path.relative_to(repo_root).as_posix()


def is_text_file(path: Path) -> bool:
    return path.name in TEXT_NAMES or path.suffix.lower() in TEXT_SUFFIXES


def line_count(text: str) -> int:
    if not text:
        return 0
    return len(text.splitlines())


def check_required_files(repo_root: Path, errors: list[str]) -> None:
    for item in REQUIRED_FILES:
        if not (repo_root / item).is_file():
            errors.append(f"missing required file: {item}")


def check_rust_workspace_contract(
    repo_root: Path,
    errors: list[str],
    paths: list[Path] | None = None,
) -> None:
    candidate_paths = paths if paths is not None else list(repo_root.rglob("Cargo.toml"))
    manifests = sorted(
        relative(repo_root, path)
        for path in candidate_paths
        if path.name == "Cargo.toml"
        and not {".git", "target"}.intersection(path.relative_to(repo_root).parts)
    )
    expected_manifests = sorted(EXPECTED_CARGO_MANIFESTS)
    if manifests != expected_manifests:
        errors.append(
            "Rust workspace must contain only the reviewed root, M0, Phase 1 library, application, and desktop manifests: "
            f"found {manifests}"
        )

    for name, expected in EXPECTED_CARGO_MANIFESTS.items():
        path = repo_root / name
        if path.is_file() and path.read_text(encoding="utf-8") != expected:
            errors.append(
                f"Rust workspace manifest differs from the reviewed implementation contract: {name}"
            )

    toolchain = repo_root / "rust-toolchain.toml"
    if toolchain.is_file() and toolchain.read_text(encoding="utf-8") != EXPECTED_RUST_TOOLCHAIN:
        errors.append(
            "rust-toolchain.toml must pin Rust 1.96.0 with the minimal profile, clippy, and rustfmt"
        )

    lockfile = repo_root / "Cargo.lock"
    if not lockfile.is_file():
        return
    lock_text = lockfile.read_text(encoding="utf-8")
    package_blocks = re.findall(
        r"\[\[package\]\]\n(.*?)(?=\n\[\[package\]\]|\Z)",
        lock_text,
        flags=re.DOTALL,
    )
    resolved_packages: list[tuple[str, str, str, str]] = []
    for block in package_blocks:
        name_match = re.search(r'^name = "([^"]+)"$', block, flags=re.MULTILINE)
        version_match = re.search(r'^version = "([^"]+)"$', block, flags=re.MULTILINE)
        if name_match is None or version_match is None:
            errors.append("Cargo.lock contains a package without a name or version")
            continue
        name = name_match.group(1)
        source_match = re.search(r'^source = "([^"]+)"$', block, flags=re.MULTILINE)
        checksum_match = re.search(r'^checksum = "([^"]+)"$', block, flags=re.MULTILINE)
        source = source_match.group(1) if source_match is not None else ""
        checksum = checksum_match.group(1) if checksum_match is not None else ""
        resolved_packages.append((name, version_match.group(1), source, checksum))
        if name in FIRST_PARTY_RUST_PACKAGES:
            if source_match is not None or checksum_match is not None:
                errors.append(f"first-party lock package must remain a workspace path: {name}")
        elif source_match is None or source_match.group(1) != CRATES_IO_SOURCE:
            errors.append(f"third-party lock package must come from crates.io: {name}")
        elif checksum_match is None:
            errors.append(f"third-party lock package is missing a checksum: {name}")

    lock_digest_payload = "\n".join(
        "\t".join(package) for package in sorted(resolved_packages)
    ).encode("utf-8")
    lock_digest = hashlib.sha256(lock_digest_payload).hexdigest()
    if (
        len(resolved_packages) != EXPECTED_REVIEWED_LOCK_PACKAGE_COUNT
        or lock_digest != EXPECTED_REVIEWED_LOCK_DIGEST
    ):
        errors.append("Cargo.lock differs from the reviewed dependency set")

    entrypoint_fragments = (
        "fmt --all --check",
        "clippy --workspace --all-targets --all-features --locked -- -D warnings",
        "test --workspace --all-targets --all-features --locked",
    )
    for name in ("scripts/check-repo.sh", "scripts/check-repo.ps1"):
        path = repo_root / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for fragment in entrypoint_fragments:
            if fragment not in text:
                errors.append(f"{name} is missing Rust check fragment: {fragment}")


def check_paths_sizes_and_safety(
    repo_root: Path,
    paths: list[Path],
    errors: list[str],
) -> None:
    for path in paths:
        name = relative(repo_root, path)
        parts = path.relative_to(repo_root).parts
        lower_name = path.name.lower()
        lower_suffix = path.suffix.lower()

        if len(name) > MAX_PATH_LENGTH:
            errors.append(f"path exceeds {MAX_PATH_LENGTH} characters: {name}")
        if path.is_file() and path.stat().st_size > MAX_FILE_BYTES:
            errors.append(f"file exceeds 10 MiB; define an artifact or LFS policy: {name}")
        if lower_name in {".ds_store", "thumbs.db", "desktop.ini"}:
            errors.append(f"operating-system metadata must not be committed: {name}")
        if set(parts).intersection(FORBIDDEN_DIRECTORY_NAMES):
            errors.append(f"generated dependency or cache path must not be committed: {name}")
        if parts and parts[0] in FORBIDDEN_ROOT_DATA_DIRECTORIES:
            errors.append(f"personal-memory or local-data path must not be committed: {name}")
        if lower_name == ".env" or (
            lower_name.startswith(".env.") and not lower_name.endswith(".example")
        ):
            errors.append(f"environment file must not be committed: {name}")
        if lower_name in FORBIDDEN_SENSITIVE_NAMES:
            errors.append(f"credential or secret file must not be committed: {name}")
        if lower_suffix in FORBIDDEN_SENSITIVE_SUFFIXES:
            errors.append(f"sensitive key or local database file must not be committed: {name}")


def check_text_files(repo_root: Path, paths: list[Path], errors: list[str]) -> None:
    for path in paths:
        if not path.is_file() or not is_text_file(path):
            continue

        name = relative(repo_root, path)
        data = path.read_bytes()
        if data.startswith(b"\xef\xbb\xbf"):
            errors.append(f"UTF-8 BOM is not allowed: {name}")
            continue

        try:
            text = data.decode("utf-8")
        except UnicodeDecodeError as exc:
            errors.append(f"text file is not valid UTF-8: {name}: {exc}")
            continue

        if "\x00" in text:
            errors.append(f"NUL byte found in declared text file: {name}")
        if "\r" in text:
            errors.append(f"text file must use LF line endings: {name}")
        if text and not text.endswith("\n"):
            errors.append(f"text file is missing final newline: {name}")
        if text.endswith("\n\n"):
            errors.append(f"text file has a blank line at EOF: {name}")

        for line_number, line in enumerate(text.splitlines(), start=1):
            if line.endswith((" ", "\t")):
                errors.append(f"trailing whitespace: {name}:{line_number}")


def check_json_files(repo_root: Path, paths: list[Path], errors: list[str]) -> None:
    for path in paths:
        if not path.is_file() or path.suffix.lower() != ".json":
            continue
        try:
            json.loads(path.read_text(encoding="utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            errors.append(f"invalid JSON: {relative(repo_root, path)}: {exc}")


def check_markdown_links(
    repo_root: Path,
    paths: list[Path],
    errors: list[str],
) -> None:
    resolved_root = repo_root.resolve()
    for path in paths:
        if not path.is_file() or path.suffix.lower() != ".md":
            continue
        text = path.read_text(encoding="utf-8")
        for match in MARKDOWN_LINK.finditer(text):
            target = unquote(match.group(1))
            if target.startswith(("#", "/", "http://", "https://", "mailto:")):
                continue
            target = target.split("#", 1)[0].split("?", 1)[0]
            if not target:
                continue
            resolved = (path.parent / target).resolve()
            try:
                resolved.relative_to(resolved_root)
            except ValueError:
                errors.append(
                    f"relative link escapes repository: {relative(repo_root, path)} -> {target}"
                )
                continue
            if not resolved.exists():
                errors.append(
                    f"broken relative link: {relative(repo_root, path)} -> {target}"
                )


def check_document_budgets(repo_root: Path, errors: list[str]) -> None:
    for name in ("AGENTS.md", "CLAUDE.md"):
        path = repo_root / name
        if path.is_file() and line_count(path.read_text(encoding="utf-8")) > MAX_AGENT_LINES:
            errors.append(f"{name} exceeds {MAX_AGENT_LINES} line startup-entry limit")

    docs_root = repo_root / "docs"
    if not docs_root.is_dir():
        return
    for path in docs_root.rglob("*.md"):
        lines = line_count(path.read_text(encoding="utf-8"))
        if lines > MAX_ACTIVE_DOC_LINES:
            errors.append(
                f"{relative(repo_root, path)} exceeds {MAX_ACTIVE_DOC_LINES} line active-document limit"
            )


def check_agent_contract(repo_root: Path, errors: list[str]) -> None:
    agents = repo_root / "AGENTS.md"
    claude = repo_root / "CLAUDE.md"
    if agents.is_file() and claude.is_file() and agents.read_bytes() != claude.read_bytes():
        errors.append("AGENTS.md and CLAUDE.md must remain identical")

    required_fragments = (
        "docs/status/current.md",
        "docs/governance/agent-collaboration.md",
        "docs/governance/repository-governance.md",
        "MemoryProposal",
        "./scripts/check-repo.sh",
        "pwsh ./scripts/check-repo.ps1",
    )
    for name in ("AGENTS.md", "CLAUDE.md"):
        path = repo_root / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for fragment in required_fragments:
            if fragment not in text:
                errors.append(f"{name} is missing startup contract fragment: {fragment}")


def check_m0_contract(repo_root: Path, errors: list[str]) -> None:
    contracts = {
        "docs/adr/0002-m0-local-memory-loop.md": (
            "M0 Local Memory Loop",
            "SourceArtifact",
            "MemoryDecision",
            "DeletionEvidence",
            "不调用云端或本地生成模型",
            "测试观察到网络请求即失败",
        ),
        "docs/evaluation/m0-local-memory-loop.md": (
            "M0-E01",
            "M0-E12",
            "标注 citation 可解析率",
            "策略违规或网络外发",
            "错误声明完全删除",
        ),
        "docs/status/current.md": (
            "ADR 0002",
            "M0 字段级 canonical schema",
            "M0 Fixture 与指标契约",
        ),
        "docs/product-scope.md": (
            "M0 Local Memory Loop",
            "ADR 0002",
        ),
        "docs/architecture.md": (
            "M0 本地架构边界",
            "不调用 Model Adapter 或 RadishMind",
        ),
        "docs/memory-model.md": (
            "SourceArtifact",
            "MemoryDecision",
            "DeleteRequest",
            "ADR 0002",
        ),
        "docs/privacy-threat-model.md": (
            "M0 信任边界",
            "任何请求视为策略违规",
        ),
        "docs/radishmind-boundary.md": (
            "M0 不使用 RadishMind",
            "不得改变已冻结的记忆真相与确认边界",
        ),
        "docs/mvp-roadmap.md": (
            "M0 本地记忆闭环",
            "完整 MVP 首个可演示场景",
        ),
    }
    for name, fragments in contracts.items():
        path = repo_root / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for fragment in fragments:
            if fragment not in text:
                errors.append(f"{name} is missing M0 contract fragment: {fragment}")


def check_m0_schema_contract(repo_root: Path, errors: list[str]) -> None:
    contracts = {
        "docs/schema/m0-canonical-schema.md": (
            "radishmemory.m0/1",
            "## SourceArtifact",
            "## SourceFragment",
            "## MemoryProposal",
            "## MemoryDecision",
            "## MemoryRecord",
            "## MemoryStateEvent",
            "## ContextPack",
            "## DeleteRequest",
            "## DeletionEvidence",
            "UTF-8",
            "sha256",
            "local_only",
            "effective_at",
            "planned_components",
            "component_results",
            "processed_count = target_count",
            "TruncationFacts",
            "FrozenTargetClosure",
            "retention_basis",
            "previous_evidence_id",
            "未知版本必须返回显式 unsupported schema 错误",
        ),
        "docs/status/current.md": (
            "M0 Canonical Schema",
            "九种顶层对象",
            "不绑定数据库、生产 ID 编码或语言类型",
        ),
        "docs/memory-model.md": (
            "schema/m0-canonical-schema.md",
            "last_state_event_id",
            "不使用 `updated_at` 原地改写历史",
        ),
        "docs/architecture.md": (
            "M0 Canonical Schema",
            "MemoryStateEvent",
            "以下运行接口仍需在对应阶段冻结",
        ),
        "docs/evaluation/m0-local-memory-loop.md": (
            "M0 Canonical Schema",
            "不得另造平行字段",
            "字段级 canonical schema、fixture 格式",
        ),
        "docs/adr/0002-m0-local-memory-loop.md": (
            "M0 Canonical Schema",
            "合成 fixture 格式与指标口径",
        ),
        "docs/mvp-roadmap.md": (
            "已冻结的 [M0 Canonical Schema]",
        ),
    }
    for name, fragments in contracts.items():
        path = repo_root / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for fragment in fragments:
            if fragment not in text:
                errors.append(
                    f"{name} is missing M0 schema contract fragment: {fragment}"
                )


def check_m0_fixture_contract(repo_root: Path, errors: list[str]) -> None:
    contracts = {
        "docs/evaluation/m0-fixture-contract.md": (
            "radishmemory.m0-fixture/1",
            "radishmemory-canonical-json-v1",
            "radishmemory-fixture-id-v1",
            "./scripts/check-m0-fixtures.py",
            "retrieval_recall_at_5",
            "model_free_loop_completion_rate",
            "目标闭包未冻结",
            "runner 不得用默认成功",
        ),
        "docs/status/current.md": (
            "12 个场景的 86 个有序操作",
            "12 个指标 gate",
            "真实 M0 runner 已经建立",
        ),
        "docs/evaluation/m0-local-memory-loop.md": (
            "M0 Fixture 与指标契约",
            "Retrieval Recall@5",
            "仓库校验器可以验证契约自洽",
        ),
        "docs/schema/m0-canonical-schema.md": (
            "M0 Fixture 与指标契约",
            "production API 仍由实现阶段决策",
            "不得自行发明第二种编码",
        ),
        "docs/adr/0002-m0-local-memory-loop.md": (
            "M0 Fixture 与指标契约",
            "首个同步信任模式",
        ),
        "docs/mvp-roadmap.md": (
            "已冻结的 [M0 Fixture 与指标契约]",
        ),
    }
    for name, fragments in contracts.items():
        path = repo_root / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for fragment in fragments:
            if fragment not in text:
                errors.append(
                    f"{name} is missing M0 fixture contract fragment: {fragment}"
                )


def check_sync_trust_contract(repo_root: Path, errors: list[str]) -> None:
    contracts = {
        "docs/adr/0003-zero-knowledge-sync-first.md": (
            "状态：Accepted",
            "模式 B：零知识同步服务",
            "服务端账号重置不能单独恢复内容解密能力",
            "不得从零知识模式静默降级",
            "可选可信计算节点",
            "已接受的目标信任模式",
        ),
        "docs/status/current.md": (
            "ADR 0003",
            "可信计算节点后置为显式可选能力",
            "不代表零知识同步已经实现",
        ),
        "docs/privacy-threat-model.md": (
            "首个多设备同步已经通过 [ADR 0003]",
            "服务端账号重置不能单独恢复内容",
        ),
        "docs/architecture.md": (
            "首个多设备同步已经通过 [ADR 0003]",
            "服务端数据库不是记忆 canonical truth",
        ),
        "docs/mvp-roadmap.md": (
            "已通过 [ADR 0003]",
            "可信计算节点后置",
        ),
    }
    for name, fragments in contracts.items():
        path = repo_root / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for fragment in fragments:
            if fragment not in text:
                errors.append(
                    f"{name} is missing sync trust contract fragment: {fragment}"
                )


def check_radishmind_entry_contract(repo_root: Path, errors: list[str]) -> None:
    contracts = {
        "docs/adr/0004-radishmind-optional-gateway-entry.md": (
            "状态：Accepted",
            "完整 MVP 的阶段 3",
            "首次接入只使用模型网关能力",
            "不同路径之间不得隐式回退",
            "不直接复制 RadishMind 的 Copilot、Application、Workflow 或 Session schema",
            "只能声明“RadishMind 可选 Gateway 接入阶段已规划或正在验证”",
        ),
        "docs/status/current.md": (
            "ADR 0004",
            "显式可关闭的 Model Gateway",
            "首次不接 Workflow、Tooling、RAG 数据 owner、Session owner 或业务写回",
        ),
        "docs/radishmind-boundary.md": (
            "首次运行接入已由 [ADR 0004]",
            "## 最小逻辑契约",
            "retry / fallback 默认关闭",
        ),
        "docs/architecture.md": (
            "根据 [ADR 0004]",
            "M0、单机资料库和记忆生命周期不以 RadishMind 可用为前提",
        ),
        "docs/privacy-threat-model.md": (
            "RadishMind 等 Gateway",
            "Gateway 是独立接收方",
        ),
        "docs/mvp-roadmap.md": (
            "已通过 [ADR 0004]",
            "首次 RadishMind 接入不包含 Workflow、Tooling、RAG 数据 owner 或业务写回",
        ),
    }
    for name, fragments in contracts.items():
        path = repo_root / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for fragment in fragments:
            if fragment not in text:
                errors.append(
                    f"{name} is missing RadishMind entry contract fragment: {fragment}"
                )


def check_implementation_stack_contract(repo_root: Path, errors: list[str]) -> None:
    contracts = {
        "docs/adr/0005-m0-implementation-stack.md": (
            "状态：Accepted",
            "Rust 2024 edition",
            "Rust `1.96.0`",
            "crates/radishmemory-core/",
            "crates/radishmemory-sqlite/",
            "apps/radishmemory-m0/",
            "bundled SQLite / FTS5",
            "首批依赖白名单",
            "M0 不实现静态加密",
            "不引入 `tokio`",
        ),
        "docs/status/current.md": (
            "Phase 1 Source Vault portable crypto complete; immutable object adapter next",
            "ADR 0005",
            "首个工具链固定为 Rust `1.96.0`",
            "`M0-I01` 已建立且仅建立上述三个可编译 package",
            "`M0-I02` 的第一个独立评审单元已实现稳定 core 错误",
            "`M0-I02` 的第二个独立评审单元已实现九种 canonical 顶层对象",
            "`M0-I02` 的第三个独立评审单元已实现跨对象不变量",
            "`M0-I03 SQLite entry` 已实现版本化 migration",
            "`M0-I03 SQLite storage` 的首个纵向切片已实现",
            "`M0-I03 SQLite storage` 的第二个纵向切片已实现",
            "`M0-I04 fixture runner` 已实现冻结 suite 摘要与向量复验",
            "已完成：精确 Rust 工具链、三 package workspace",
        ),
        "README.md": (
            "Phase 1 Source Vault portable crypto complete; immutable object adapter next",
            "SQLite v6 connection / migration",
            "真实 M0 runner",
            "不授权本任务使用真实个人资料",
        ),
        "docs/implementation/m0-rust-dependency-baseline.md": (
            "lockfile format 为 `4`",
            "七个第一方 workspace package",
            "423 个第三方 package",
            "40 个第三方 package",
            "没有 Git dependency",
            "`serde_json 1.0.151`",
            "`rusqlite 0.40.2`",
            "`libsqlite3-sys 0.38.2`",
            "SQLite `3.53.2`",
            "`SQLITE_ENABLE_FTS5`",
            "`serde_derive` 与 `time-macros` 是 headless 基础子图实际解析的 proc macro",
            "Linux、macOS、Windows 与 `Candidate Quality` 已通过",
        ),
        "docs/architecture.md": (
            "[ADR 0005]",
            "Rust 2024 模块化单体",
            "仅 opt-in `fixture-runner` feature 为每个合成场景建立独立内存连接",
            "数据库 rowid、SQL schema、FTS 分数和 SQLite JSON 不进入长期 canonical 格式",
        ),
        "docs/mvp-roadmap.md": (
            "已通过 [ADR 0005]",
            "Rust 模块化单体、SQLite / FTS5、依赖和验证基线",
            "阶段 1 不把上述范围一次性展开为大批次",
        ),
        "docs/adr/0002-m0-local-memory-loop.md": (
            "[ADR 0005]",
            "实施顺序从最小 workspace 开始",
        ),
    }
    for name, fragments in contracts.items():
        path = repo_root / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for fragment in fragments:
            if fragment not in text:
                errors.append(
                    f"{name} is missing implementation stack contract fragment: {fragment}"
                )


def check_phase1_file_entry_contract(repo_root: Path, errors: list[str]) -> None:
    contracts = {
        "docs/adr/0006-phase1-text-markdown-file-entry.md": (
            "状态：Accepted",
            "radishmemory.phase1-file-entry/1",
            "P1-I01 file snapshot contract",
            "P1-I02 atomic source capture",
            "P1-I03 exact export",
            "P1-I04 lineage deletion",
            "crates/radishmemory-file-entry/",
            "用户显式选择的单个普通文件",
            "8_388_608",
            "symlink_not_allowed",
            "source_changed_during_capture",
            "普通 search、citation 与 ContextPack 只接受 active 的唯一 lineage tip",
            "不操作外部原件或用户导出",
            "`P1-F01`",
            "`P1-F18`",
            "不修改 M0 fixture schema",
        ),
        "README.md": (
            "[ADR 0006]",
            "`P1-F01` 至 `P1-F18`",
            "workflow run 33302423840",
            "workflow run 33751048480",
        ),
        "docs/README.md": (
            "ADR 0006：阶段 1 文本 / Markdown 文件入口",
        ),
        "docs/status/current.md": (
            "ADR 0006",
            "P1-I01 file snapshot contract",
            "P1-I02 atomic source capture",
            "P1-I03 exact export",
            "P1-I04 lineage deletion",
            "radishmemory-file-entry",
            "SourceCaptureStore",
            "`P1-F01` 至 `P1-F18`",
            "acceptance-test-support",
            "不代表完整 importer / exporter 已实现",
        ),
        "docs/architecture.md": (
            "阶段 1 文本 / Markdown 文件入口边界",
            "radishmemory-file-entry",
            "不增加文件专用 canonical object",
            "active lineage tip",
            "SQLite v6 adapter",
            "P1-I03 exact export",
            "P1-I04 lineage deletion",
            "`P1-F02` / `P1-F05`",
            "`P1-F11` 至 `P1-F14`",
            "`P1-F15` 至 `P1-F18`",
        ),
        "docs/implementation/m0-rust-dependency-baseline.md": (
            "七个第一方 workspace package",
            "radishmemory-file-entry 0.1.0",
            "40 个第三方 package",
            "当时没有扩大 40 个第三方 package 的 headless 基础子图",
            "P1-I02 atomic source capture",
            "P1-I03",
            "P1-I04",
            "hardlink provenance 独立删除",
            "确定性 TOCTOU",
            "acceptance-test-support",
        ),
        "docs/privacy-threat-model.md": (
            "阶段 1 文件入口信任边界",
            "外部原件、hardlink alias、手工副本和用户导出",
            "未实现静态加密",
        ),
        "docs/mvp-roadmap.md": (
            "已通过 [ADR 0006]",
            "18 个合成验收场景",
        ),
    }
    for name, fragments in contracts.items():
        path = repo_root / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for fragment in fragments:
            if fragment not in text:
                errors.append(
                    f"{name} is missing Phase 1 file entry contract fragment: {fragment}"
                )

    diagnostic_sink = re.compile(
        r"\b(?:log|tracing)::|(?:println|eprintln|dbg)!\s*\("
    )
    for source_root in (
        repo_root / "apps/radishmemory-desktop/src",
        repo_root / "crates/radishmemory-application/src",
        repo_root / "crates/radishmemory-file-entry/src",
        repo_root / "crates/radishmemory-source-vault/src",
        repo_root / "crates/radishmemory-sqlite/src",
    ):
        for path in sorted(source_root.rglob("*.rs")):
            text = path.read_text(encoding="utf-8")
            if diagnostic_sink.search(text):
                errors.append(
                    f"Phase 1 source introduces an unreviewed diagnostic sink: "
                    f"{path.relative_to(repo_root)}"
                )


def check_phase1_local_host_contract(repo_root: Path, errors: list[str]) -> None:
    contracts = {
        "docs/adr/0007-phase1-local-library-host.md": (
            "状态：Accepted",
            "radishmemory.phase1-local-library-host/1",
            "P1-H02 application service",
            "P1-H03 source catalog",
            "P1-H04 desktop UI",
            "P1-H05 host acceptance",
            "不持久化 platform bookmark",
            "不启动本地 HTTP 服务",
            "`P1-HF01`",
            "`P1-HF12`",
            "不新增 canonical 顶层对象",
        ),
        "README.md": (
            "[ADR 0007]",
            "Phase 1 host acceptance complete",
            "application service",
            "THIRD_PARTY_NOTICES.md",
        ),
        "docs/README.md": (
            "ADR 0007：阶段 1 本地资料库宿主与显式文件授权",
            "Phase 1 macOS 桌面宿主交互验收",
            "Phase 1 Windows 桌面宿主交互验收",
            "Phase 1 Linux 桌面宿主交互验收",
            "Phase 1 第三方 notices 与条件平台依赖复核",
        ),
        "docs/status/current.md": (
            "ADR 0007",
            "P1-H02 application service",
            "P1-H03 source catalog",
            "P1-H04 desktop UI",
            "P1-H05 host acceptance",
            "`P1-HF01` 至 `P1-HF12`",
            "333 个目标可达 crate",
        ),
        "docs/architecture.md": (
            "阶段 1 本地资料库宿主边界",
            "一次性系统文件选择 capability",
            "不启动本地 HTTP 服务",
        ),
        "docs/privacy-threat-model.md": (
            "阶段 1 本地宿主授权边界",
            "不持久化完整路径",
            "真实系统选择器",
        ),
        "docs/mvp-roadmap.md": (
            "[ADR 0007]",
            "十二项宿主验收",
        ),
        "docs/implementation/m0-rust-dependency-baseline.md": (
            "radishmemory-application 0.1.0",
            "radishmemory-desktop 0.1.0",
            "P1-H02 application service",
            "P1-H03 source catalog",
            "P1-H04 desktop UI",
            "423 个第三方 package",
        ),
        "docs/implementation/phase1-desktop-dependency-review.md": (
            "状态：`Accepted",
            "eframe = { version = \"=0.36.1\"",
            "rfd = { version = \"=0.17.2\"",
            "418 个 package",
            "180 个唯一 package ID",
            "workflow run `33751048480`",
        ),
        "docs/implementation/phase1-macos-host-acceptance.md": (
            "P1-H05 complete",
            "AppKit open / save panel",
            "`P1-HF01`",
            "`P1-HF12`",
            "Linux、macOS、Windows Rust Quality 与聚合 `Candidate Quality`",
            "第三方 notices 与条件平台依赖复核",
        ),
        "docs/implementation/phase1-windows-host-acceptance.md": (
            "P1-H05 complete",
            "Windows 原生 open / save dialog",
            "egui_glow requires opengl 2.0+",
            "BUILTIN\\Administrators",
            "BUILTIN\\Users",
        ),
        "docs/implementation/phase1-linux-host-acceptance.md": (
            "P1-H05 complete",
            "XDG Portal / GTK",
            "没有 `zenity` 进程",
            "host-profile-v1.txt",
            "library.sqlite3",
            "第三方 notices 与条件平台依赖复核",
        ),
        "docs/implementation/phase1-third-party-notices.md": (
            "P1-H05 distribution inventory gate complete",
            "344 个唯一 crates.io package",
            "67e767a36884963bd2ddc5b2db932226a1cdba076ad974630eec357d52dd2e9a",
            "MIT AND OFL-1.1 AND Ubuntu-font-1.0",
            "MIT AND Unicode-3.0",
            "XDG Desktop Portal",
            "SQLite `3.53.2`",
            "P1-H05 gate 完成",
        ),
        "crates/radishmemory-application/src/lib.rs": (
            "radishmemory.phase1-local-library-host/1",
            "pub struct LocalLibrary",
            "pub fn import_new_source",
            "pub fn update_source",
            "pub fn search_sources",
            "pub fn delete_source_lineage",
        ),
        "crates/radishmemory-core/src/ports.rs": (
            "pub trait SourceCatalog",
            "fn resolve_source_lineage",
            "fn list_source_lineages",
            "fn list_source_versions",
            "fn resolve_source_lineage_deletion_targets",
        ),
        "crates/radishmemory-sqlite/src/source_catalog.rs": (
            "impl SourceCatalog for SqliteDatabase",
            "verify_origin_bindings",
        ),
        "apps/radishmemory-desktop/src/profile.rs": (
            "radishmemory.phase1-host-profile/1",
            "ProfileMissingForExistingDatabase",
            "fs::hard_link",
        ),
        "apps/radishmemory-desktop/src/controller.rs": (
            "LocalLibraryConfig::phase1_local",
            "LibraryController::bootstrap",
            "delete_selected_lineage",
        ),
        "apps/radishmemory-desktop/src/picker.rs": (
            "rfd::FileDialog",
            "FileReadRequest::new",
            "FileExportRequest::new",
        ),
        "apps/radishmemory-desktop/src/ui.rs": (
            "impl eframe::App for RadishMemoryApp",
            "Latest deletion evidence",
            "The original selected file and prior exports are not deleted.",
        ),
    }
    for name, fragments in contracts.items():
        path = repo_root / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for fragment in fragments:
            if fragment not in text:
                errors.append(
                    f"{name} is missing Phase 1 local host contract fragment: {fragment}"
                )


def check_phase1_encrypted_source_vault_contract(
    repo_root: Path, errors: list[str]
) -> None:
    contracts = {
        "docs/adr/0008-phase1-encrypted-source-vault.md": (
            "状态：Accepted",
            "radishmemory.phase1-encrypted-source-vault/1",
            "P1-S01 storage contract",
            "P1-S02 dependency and cipher review",
            "一个不可变 `SourceArtifact` version 对应一个不可变密文对象",
            "不进行跨 lineage 或跨 provenance 物理去重",
            "设备本地 key-encryption key（KEK）",
            "经过评审的 AEAD cipher suite",
            "SQLite `IMMEDIATE` transaction",
            "SQLite v6 inline body 迁移",
            "不新增 canonical 顶层对象",
            "`P1-SF01`",
            "`P1-SF18`",
            "当前代码仍使用 SQLite v6 inline plaintext body",
        ),
        "README.md": (
            "[ADR 0008]",
            "Phase 1 Source Vault portable crypto complete; immutable object adapter next",
            "一 source version 一密文对象",
            "SQLite v6 inline plaintext body",
            "不能声明加密 Source Vault 已可用或整个资料库已静态加密",
        ),
        "docs/README.md": (
            "ADR 0008：阶段 1 加密内容寻址 Source Vault",
        ),
        "docs/status/current.md": (
            "ADR 0008",
            "P1-S01 storage contract",
            "P1-S02 dependency and cipher review",
            "`P1-SF01` 至 `P1-SF18`",
            "SQLite metadata、FTS、派生数据",
            "不跨 provenance 物理去重",
            "当前 production code 仍是 SQLite v6 inline plaintext body",
        ),
        "docs/architecture.md": (
            "阶段 1 加密内容寻址 Source Vault 边界",
            "一个不可变 SourceArtifact version 首批对应一个不可变密文对象",
            "不同 `source_id` 即使摘要相同也不跨 lineage / provenance 物理去重",
            "密文 publish → SQLite commit → read-back",
            "P1-S02",
        ),
        "docs/privacy-threat-model.md": (
            "阶段 1 加密 Source Vault 信任边界",
            "SQLite metadata、FTS、标题、摘要",
            "每个 SourceArtifact version 使用独立随机 DEK",
            "本地 key 丢失可能使对应对象永久不可恢复",
            "整个本地资料库已经静态加密",
        ),
        "docs/mvp-roadmap.md": (
            "[ADR 0008]",
            "一 source version 一密文对象",
            "P1-S02",
            "PDF / 图片解析只能在 encrypted Source Vault",
        ),
        "docs/adr/0005-m0-implementation-stack.md": (
            "[ADR 0008]",
            "P1-S03a portable manifest / lockfile landing 已完成",
        ),
        "docs/adr/0006-phase1-text-markdown-file-entry.md": (
            "[ADR 0008]",
            "在其 dependency、adapter、migration 与 host acceptance 完成前不进入 PDF / 图片解析",
        ),
        "docs/adr/0007-phase1-local-library-host.md": (
            "[ADR 0008]",
            "不代表 object adapter、platform provider 或加密 Source Vault 已实现",
        ),
    }
    for name, fragments in contracts.items():
        path = repo_root / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for fragment in fragments:
            if fragment not in text:
                errors.append(
                    f"{name} is missing Phase 1 encrypted Source Vault contract "
                    f"fragment: {fragment}"
                )


def check_phase1_encrypted_source_vault_dependency_review(
    repo_root: Path, errors: list[str]
) -> None:
    contracts = {
        "docs/implementation/phase1-encrypted-source-vault-dependency-review.md": (
            "状态：`Accepted — profile 已冻结；P1-S03a portable graph 已落地，platform providers 待后续单元`",
            "radishmemory.xchacha20poly1305-stream-be32/1",
            "radishmemory.xchacha20poly1305-dek-wrap/1",
            'aead-stream = { version = "=0.6.0"',
            'chacha20poly1305 = { version = "=0.11.0"',
            'zeroize = { version = "=1.9.0"',
            'keyring-core = { version = "=1.0.0"',
            'apple-native-keyring-store = { version = "=1.0.2"',
            'windows-native-keyring-store = { version = "=1.1.0"',
            'zbus-secret-service-keyring-store = { version = "=1.0.1"',
            "`Local`",
            "`crypto-rust`",
            "`rmkek1:`",
            "`create_if_absent_for_bootstrap`",
            "SQLite `IMMEDIATE` transaction",
            "P1-S03a portable crypto dependency landing",
            "P1-S03b immutable object filesystem adapter",
        ),
        "README.md": (
            "Phase 1 Source Vault portable crypto complete; immutable object adapter next",
            "XChaCha20-Poly1305 + STREAM-BE32",
            "P1-S03a 已完成 portable manifest / `Cargo.lock`",
        ),
        "docs/README.md": (
            "Phase 1 加密 Source Vault 依赖与密码套件评审",
        ),
        "docs/status/current.md": (
            "P1-S02 dependency and cipher review",
            "P1-S03a portable crypto dependency landing",
            "radishmemory.xchacha20poly1305-stream-be32/1",
            "radishmemory.xchacha20poly1305-dek-wrap/1",
            "portable manifest / `Cargo.lock` / notices 和 cipher 实现落地",
        ),
        "docs/architecture.md": (
            "XChaCha20-Poly1305 + STREAM-BE32",
            "macOS Keychain、Windows Credential Manager 或 Linux Secret Service",
            "三个 platform provider 尚未进入依赖图",
        ),
        "docs/privacy-threat-model.md": (
            "XChaCha20-Poly1305 + STREAM-BE32",
            "不回退 file-stored key、sample store 或其它 provider",
        ),
        "docs/mvp-roadmap.md": (
            "P1-S03a portable crypto dependency landing",
            "manifest / lockfile / notices 变化",
        ),
        "docs/adr/0008-phase1-encrypted-source-vault.md": (
            "P1-S02 依赖与密码套件评审",
            "radishmemory.xchacha20poly1305-stream-be32/1",
            "radishmemory.xchacha20poly1305-dek-wrap/1",
            "P1-S03a portable crypto dependency landing",
        ),
        "docs/adr/0005-m0-implementation-stack.md": (
            "P1-S02",
            "manifest / lockfile landing",
        ),
        "docs/adr/0007-phase1-local-library-host.md": (
            "P1-S02",
            "不代表 object adapter、platform provider 或加密 Source Vault 已实现",
        ),
    }
    for name, fragments in contracts.items():
        path = repo_root / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for fragment in fragments:
            if fragment not in text:
                errors.append(
                    f"{name} is missing Phase 1 encrypted Source Vault "
                    f"dependency review fragment: {fragment}"
                )


def check_phase1_source_vault_portable_crypto(
    repo_root: Path, errors: list[str]
) -> None:
    contracts = {
        "docs/implementation/phase1-source-vault-portable-crypto.md": (
            "P1-S03a portable crypto dependency landing complete",
            "radishmemory-source-vault",
            "radishmemory.xchacha20poly1305-stream-be32/1",
            "radishmemory.xchacha20poly1305-dek-wrap/1",
            "当前 12 个 package unit test",
            "430 个 package",
            "423 个 crates.io package",
            "并集 344",
            "67e767a36884963bd2ddc5b2db932226a1cdba076ad974630eec357d52dd2e9a",
            "5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5",
            "RUSTSEC-2026-0003",
            "RUSTSEC-2019-0029",
            "P1-S03b immutable object filesystem adapter",
        ),
        "README.md": (
            "Phase 1 Source Vault portable crypto complete; immutable object adapter next",
            "P1-S03a 落地记录",
            "扩大到 344 项",
            "三个 platform provider、object filesystem、SQLite migration",
        ),
        "docs/status/current.md": (
            "P1-S03a portable crypto dependency landing",
            "P1-S03b immutable object filesystem adapter",
            "7 个第一方和 423 个 crates.io 第三方 package",
            "macOS / Linux / Windows 分别为 215 / 285 / 209 项",
        ),
        "docs/architecture.md": (
            "P1-S03a",
            "独立 portable crypto package",
            "三个 platform provider 尚未进入依赖图",
            "P1-S03b` 至 `P1-S05",
        ),
        "docs/privacy-threat-model.md": (
            "P1-S03a",
            "portable cipher / wrap / AAD 与合成测试",
            "filesystem、platform key provider、SQLite migration",
        ),
        "docs/mvp-roadmap.md": (
            "P1-S03a portable crypto dependency landing",
            "P1-S03b immutable object filesystem adapter",
            "durable no-overwrite publish",
        ),
        "docs/adr/0008-phase1-encrypted-source-vault.md": (
            "P1-S03a portable crypto 落地",
            "P1-S03b immutable object filesystem adapter",
            "portable dependency / cipher / wrap / AAD / 合成测试已落地",
        ),
        "docs/implementation/m0-rust-dependency-baseline.md": (
            "七个第一方 workspace package",
            "423 个第三方 package",
            "Source Vault portable crypto 直接依赖",
            "P1-S03a 的 11 个新增 package",
            "两个分发根的三目标可达依赖 notices",
        ),
        "docs/implementation/phase1-third-party-notices.md": (
            "P1-S03a expansion reviewed",
            "344 个唯一 crates.io package",
            "两个分发根",
            "aead-stream",
        ),
        "scripts/generate-third-party-notices.py": (
            'ROOT_PACKAGES = ("radishmemory-desktop", "radishmemory-source-vault")',
        ),
        "crates/radishmemory-source-vault/src/lib.rs": (
            "#![forbid(unsafe_code)]",
            'OBJECT_CIPHER_PROFILE: &str = "radishmemory.xchacha20poly1305-stream-be32/1"',
            'DEK_WRAP_PROFILE: &str = "radishmemory.xchacha20poly1305-dek-wrap/1"',
            "MAX_OBJECT_PLAINTEXT_BYTES: usize = 8 * 1024 * 1024",
        ),
        "crates/radishmemory-source-vault/src/aad.rs": (
            'AAD_CODEC_PREFIX: &[u8] = b"RMAAD\\x01"',
            "aad_codec_matches_frozen_byte_level_vectors",
            "every_caller_supplied_metadata_field_changes_both_aad_domains",
        ),
        "crates/radishmemory-source-vault/src/crypto.rs": (
            "EncryptorBE32::<XChaCha20Poly1305>",
            "Zeroizing<[u8; KEY_BYTES]>",
            "seal_object_with_random",
            "cfrg_xchacha20poly1305_appendix_a1_vector_matches",
            "project_owned_stream_vectors_cover_phase1_size_boundaries",
            "tampering_truncation_reordering_and_metadata_changes_fail_closed",
        ),
        "crates/radishmemory-source-vault/src/random.rs": (
            "pub(crate) trait RandomSource",
            "getrandom::fill(destination)",
        ),
    }
    for name, fragments in contracts.items():
        path = repo_root / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for fragment in fragments:
            if fragment not in text:
                errors.append(
                    f"{name} is missing P1-S03a portable crypto fragment: {fragment}"
                )

    forbidden_platform_dependencies = (
        "keyring-core",
        "apple-native-keyring-store",
        "windows-native-keyring-store",
        "zbus-secret-service-keyring-store",
    )
    for name in ("Cargo.toml", "Cargo.lock"):
        path = repo_root / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for dependency in forbidden_platform_dependencies:
            if dependency in text:
                errors.append(
                    f"{name} includes platform key-store dependency before its authorized unit: "
                    f"{dependency}"
                )


def run_m0_fixture_check(repo_root: Path, errors: list[str]) -> None:
    result = subprocess.run(
        [sys.executable, "scripts/check-m0-fixtures.py"],
        cwd=repo_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        detail = (result.stdout + result.stderr).strip()
        errors.append(f"M0 fixture validation failed: {detail}")


def run_third_party_notice_check(repo_root: Path, errors: list[str]) -> None:
    result = subprocess.run(
        [sys.executable, "scripts/generate-third-party-notices.py", "--check"],
        cwd=repo_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        detail = (result.stdout + result.stderr).strip()
        errors.append(f"third-party notice validation failed: {detail}")


def check_issue_and_pr_contracts(repo_root: Path, errors: list[str]) -> None:
    contracts = {
        ".github/ISSUE_TEMPLATE/config.yml": (
            "blank_issues_enabled: false",
            "RadishMemory/security/policy",
        ),
        ".github/ISSUE_TEMPLATE/bug-report.yml": (
            "Private Vulnerability Reporting",
            "合成或安全脱敏",
            "数据与隐私影响",
        ),
        ".github/ISSUE_TEMPLATE/change-proposal.yml": (
            "受影响真相源",
            "信任、隐私与删除边界",
            "验证计划与失败判据",
            "SECURITY.md",
        ),
        ".github/PULL_REQUEST_TEMPLATE.md": (
            "目标分支：`dev` / `master`",
            "明确非目标",
            "MemoryProposal",
            "ContextPack",
            "未验证、风险与回滚",
            "`master` 合并后回流",
        ),
    }
    for name, fragments in contracts.items():
        path = repo_root / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for fragment in fragments:
            if fragment not in text:
                errors.append(f"{name} is missing contract fragment: {fragment}")


def find_rule(rules: list[object], rule_type: str) -> dict[str, object] | None:
    for rule in rules:
        if isinstance(rule, dict) and rule.get("type") == rule_type:
            return rule
    return None


def check_ruleset_contract(repo_root: Path, errors: list[str]) -> None:
    path = repo_root / ".github/rulesets/master-protection.json"
    if not path.is_file():
        return
    try:
        ruleset = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return

    if ruleset.get("target") != "branch" or ruleset.get("enforcement") != "active":
        errors.append("master ruleset must be an active branch ruleset")
    include = ruleset.get("conditions", {}).get("ref_name", {}).get("include", [])
    if include != ["refs/heads/master"]:
        errors.append("master ruleset must target only refs/heads/master")

    bypass = ruleset.get("bypass_actors", [])
    if not isinstance(bypass, list) or any(
        not isinstance(actor, dict) or actor.get("bypass_mode") != "pull_request"
        for actor in bypass
    ):
        errors.append("master ruleset bypass must be limited to pull_request mode")

    rules = ruleset.get("rules")
    if not isinstance(rules, list):
        errors.append("master ruleset must define a rules array")
        return
    for required_type in ("deletion", "non_fast_forward", "pull_request", "required_status_checks"):
        if find_rule(rules, required_type) is None:
            errors.append(f"master ruleset is missing rule: {required_type}")

    pull_request = find_rule(rules, "pull_request")
    if pull_request is not None:
        parameters = pull_request.get("parameters", {})
        if parameters.get("allowed_merge_methods") != ["merge", "rebase"]:
            errors.append("master ruleset must allow merge and rebase, in that order")
        if parameters.get("required_review_thread_resolution") is not True:
            errors.append("master ruleset must require review thread resolution")
        if parameters.get("required_approving_review_count") != 0:
            errors.append("single-maintainer baseline must require zero approvals")
        if parameters.get("require_code_owner_review") is not False:
            errors.append("single-maintainer baseline must not require CODEOWNERS")
        if parameters.get("require_extra_approval_for_unattributed_changes") is not True:
            errors.append("unattributed Copilot changes must require extra approval")

    checks = find_rule(rules, "required_status_checks")
    if checks is not None:
        parameters = checks.get("parameters", {})
        contexts = [
            item.get("context")
            for item in parameters.get("required_status_checks", [])
            if isinstance(item, dict)
        ]
        if contexts != ["Candidate Quality"]:
            errors.append("master ruleset must require only Candidate Quality")
        integration_ids = [
            item.get("integration_id")
            for item in parameters.get("required_status_checks", [])
            if isinstance(item, dict)
        ]
        if integration_ids != [15368]:
            errors.append("Candidate Quality must originate from the GitHub Actions app")
        if parameters.get("strict_required_status_checks_policy") is not True:
            errors.append("master ruleset must require the branch to be up to date")


def check_workflow_contract(repo_root: Path, errors: list[str]) -> None:
    path = repo_root / ".github/workflows/pr-check.yml"
    if not path.is_file():
        return
    text = path.read_text(encoding="utf-8")
    required_fragments = (
        "pull_request:",
        "      - dev",
        "      - master",
        "permissions:\n  contents: read",
        "uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1",
        "persist-credentials: false",
        "uses: actions/setup-python@5fda3b95a4ea91299a34e894583c3862153e4b97 # v7.0.0",
        "name: Repo Hygiene",
        "python scripts/check-repo.py --base-ref",
        "name: Rust Quality (${{ matrix.platform }})",
        "          - platform: Linux\n            os: ubuntu-latest",
        "          - platform: macOS\n            os: macos-latest",
        "          - platform: Windows\n            os: windows-latest",
        "rustup toolchain install 1.96.0 --profile minimal --component clippy,rustfmt",
        "cargo fmt --all --check",
        "cargo clippy --workspace --all-targets --all-features --locked -- -D warnings",
        "cargo test --workspace --all-targets --all-features --locked",
        "name: Candidate Quality",
        "RUST_QUALITY_RESULT: ${{ needs.rust-quality.result }}",
    )
    for fragment in required_fragments:
        if fragment not in text:
            errors.append(f"PR workflow is missing contract fragment: {fragment.strip()}")
    if text.count("    timeout-minutes: 10") != 3:
        errors.append("PR workflow jobs must use the 10-minute timeout baseline")
    checkout = (
        "uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1"
    )
    if text.count(checkout) != 2:
        errors.append("PR workflow must pin checkout for Repo Hygiene and Rust Quality")
    for forbidden in ("pull_request_target:", "workflow_run:", "contents: write"):
        if forbidden in text:
            errors.append(f"PR workflow must not use privileged capability: {forbidden}")


def check_diff(repo_root: Path, base_ref: str | None, errors: list[str]) -> None:
    commands: list[tuple[str, ...]] = []
    if base_ref:
        if git(repo_root, "rev-parse", "--verify", base_ref).returncode != 0:
            errors.append(f"base ref does not resolve: {base_ref}")
            return
        commands.append(("diff", "--check", f"{base_ref}...HEAD"))
    else:
        commands.extend((("diff", "--check"), ("diff", "--cached", "--check")))

    for command in commands:
        result = git(repo_root, *command)
        if result.returncode != 0:
            detail = (result.stdout + result.stderr).strip()
            errors.append(f"git {' '.join(command)} failed: {detail}")


def is_allowed_commit_subject(subject: str) -> bool:
    return bool(
        CONVENTIONAL_COMMIT.fullmatch(subject)
        or ALLOWED_MERGE_COMMIT.match(subject)
    )


def check_commit_messages(
    repo_root: Path,
    base_ref: str | None,
    errors: list[str],
) -> None:
    if not base_ref:
        return
    result = git(repo_root, "log", "--format=%H%x09%s", f"{base_ref}..HEAD")
    if result.returncode != 0:
        errors.append(result.stderr.strip() or "unable to read PR commit range")
        return
    for line in result.stdout.splitlines():
        commit, _, subject = line.partition("\t")
        if not is_allowed_commit_subject(subject):
            errors.append(f"non-conventional commit subject: {commit[:12]} {subject}")


def run_checker_tests(repo_root: Path, errors: list[str]) -> None:
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "unittest",
            "discover",
            "-s",
            "scripts/tests",
            "-p",
            "test_*.py",
        ],
        cwd=repo_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        detail = (result.stdout + result.stderr).strip()
        errors.append(f"repository checker tests failed: {detail}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--base-ref",
        help="optional base commit/ref for PR diff and commit-message checks",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    errors: list[str] = []
    try:
        paths = repository_files(REPO_ROOT)
    except RuntimeError as exc:
        print(f"RadishMemory repository baseline failed: {exc}", file=sys.stderr)
        return 1

    check_required_files(REPO_ROOT, errors)
    check_rust_workspace_contract(REPO_ROOT, errors, paths)
    check_paths_sizes_and_safety(REPO_ROOT, paths, errors)
    check_text_files(REPO_ROOT, paths, errors)
    check_json_files(REPO_ROOT, paths, errors)
    check_markdown_links(REPO_ROOT, paths, errors)
    check_document_budgets(REPO_ROOT, errors)
    check_agent_contract(REPO_ROOT, errors)
    check_m0_contract(REPO_ROOT, errors)
    check_m0_schema_contract(REPO_ROOT, errors)
    check_m0_fixture_contract(REPO_ROOT, errors)
    check_sync_trust_contract(REPO_ROOT, errors)
    check_radishmind_entry_contract(REPO_ROOT, errors)
    check_implementation_stack_contract(REPO_ROOT, errors)
    check_phase1_file_entry_contract(REPO_ROOT, errors)
    check_phase1_local_host_contract(REPO_ROOT, errors)
    check_phase1_encrypted_source_vault_contract(REPO_ROOT, errors)
    check_phase1_encrypted_source_vault_dependency_review(REPO_ROOT, errors)
    check_phase1_source_vault_portable_crypto(REPO_ROOT, errors)
    check_issue_and_pr_contracts(REPO_ROOT, errors)
    check_ruleset_contract(REPO_ROOT, errors)
    check_workflow_contract(REPO_ROOT, errors)
    check_diff(REPO_ROOT, args.base_ref, errors)
    check_commit_messages(REPO_ROOT, args.base_ref, errors)
    run_m0_fixture_check(REPO_ROOT, errors)
    run_third_party_notice_check(REPO_ROOT, errors)
    run_checker_tests(REPO_ROOT, errors)

    if errors:
        print("RadishMemory repository baseline failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(f"RadishMemory repository baseline passed ({len(paths)} files checked).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
