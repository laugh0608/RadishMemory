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
    "docs/adr/0002-m0-local-memory-loop.md",
    "docs/adr/0003-zero-knowledge-sync-first.md",
    "docs/adr/0004-radishmind-optional-gateway-entry.md",
    "docs/adr/0005-m0-implementation-stack.md",
    "docs/architecture.md",
    "docs/evaluation/m0-fixture-contract.md",
    "docs/evaluation/m0-local-memory-loop.md",
    "docs/governance/agent-collaboration.md",
    "docs/governance/repository-governance.md",
    "docs/memory-model.md",
    "docs/mvp-roadmap.md",
    "docs/privacy-threat-model.md",
    "docs/product-scope.md",
    "docs/radishmind-boundary.md",
    "docs/references.md",
    "docs/schema/m0-canonical-schema.md",
    "docs/status/current.md",
    "fixtures/m0/local-memory-loop.v1.json",
    "scripts/check-m0-fixtures.py",
    "scripts/check-repo.ps1",
    "scripts/check-repo.py",
    "scripts/check-repo.sh",
    "scripts/tests/test_check_repo.py",
    "scripts/tests/test_check_m0_fixtures.py",
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
            "真实 M0 runner 和产品能力仍未实现",
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
            "M0 implementation entry",
            "ADR 0005",
            "首个工具链固定为 Rust `1.96.0`",
            "待完成：三 package workspace",
        ),
        "docs/architecture.md": (
            "[ADR 0005]",
            "Rust 2024 模块化单体",
            "数据库 rowid、SQL schema、FTS 分数和 SQLite JSON 不进入长期 canonical 格式",
        ),
        "docs/mvp-roadmap.md": (
            "已通过 [ADR 0005]",
            "Rust 模块化单体、SQLite / FTS5、依赖和验证基线",
        ),
        "docs/adr/0002-m0-local-memory-loop.md": (
            "[ADR 0005]",
            "下一步直接建立最小 workspace 并实现真实 runner",
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
    check_m0_contract(REPO_ROOT, errors)
    check_m0_schema_contract(REPO_ROOT, errors)
    check_m0_fixture_contract(REPO_ROOT, errors)
    check_sync_trust_contract(REPO_ROOT, errors)
    check_radishmind_entry_contract(REPO_ROOT, errors)
    check_implementation_stack_contract(REPO_ROOT, errors)
    check_issue_and_pr_contracts(REPO_ROOT, errors)
    check_ruleset_contract(REPO_ROOT, errors)
    check_workflow_contract(REPO_ROOT, errors)
    check_diff(REPO_ROOT, args.base_ref, errors)
    check_commit_messages(REPO_ROOT, args.base_ref, errors)
    run_m0_fixture_check(REPO_ROOT, errors)
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
