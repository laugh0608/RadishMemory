#!/usr/bin/env python3
"""Dependency-free RadishMemory repository governance and hygiene checks."""

from __future__ import annotations

import argparse
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
    "docs/README.md",
    "docs/adr/0001-branch-and-pr-governance.md",
    "docs/architecture.md",
    "docs/governance/agent-collaboration.md",
    "docs/governance/repository-governance.md",
    "docs/memory-model.md",
    "docs/mvp-roadmap.md",
    "docs/privacy-threat-model.md",
    "docs/product-scope.md",
    "docs/radishmind-boundary.md",
    "docs/references.md",
    "docs/status/current.md",
    "scripts/check-repo.ps1",
    "scripts/check-repo.py",
    "scripts/check-repo.sh",
    "scripts/tests/test_check_repo.py",
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
    "LICENSE",
    "Makefile",
}

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
        "name: Candidate Quality",
        "./scripts/check-repo.sh --base-ref",
    )
    for fragment in required_fragments:
        if fragment not in text:
            errors.append(f"PR workflow is missing contract fragment: {fragment.strip()}")
    if text.count("    timeout-minutes: 10") != 2:
        errors.append("PR workflow jobs must use the 10-minute timeout baseline")
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
    check_paths_sizes_and_safety(REPO_ROOT, paths, errors)
    check_text_files(REPO_ROOT, paths, errors)
    check_json_files(REPO_ROOT, paths, errors)
    check_markdown_links(REPO_ROOT, paths, errors)
    check_document_budgets(REPO_ROOT, errors)
    check_agent_contract(REPO_ROOT, errors)
    check_issue_and_pr_contracts(REPO_ROOT, errors)
    check_ruleset_contract(REPO_ROOT, errors)
    check_workflow_contract(REPO_ROOT, errors)
    check_diff(REPO_ROOT, args.base_ref, errors)
    check_commit_messages(REPO_ROOT, args.base_ref, errors)
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
