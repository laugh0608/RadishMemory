#!/usr/bin/env python3
"""Generate the reviewed desktop third-party notice inventory from Cargo metadata."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import tomllib
from collections import deque
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
OUTPUT_PATH = REPO_ROOT / "THIRD_PARTY_NOTICES.md"
CRATES_IO_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
ROOT_PACKAGE = "radishmemory-desktop"
TARGETS = (
    ("macOS", "aarch64-apple-darwin"),
    ("Linux", "aarch64-unknown-linux-gnu"),
    ("Windows", "aarch64-pc-windows-msvc"),
)

# Cargo license expressions are upstream declarations. For OR expressions this
# table records the terms selected for RadishMemory binary/source distribution;
# it does not change the upstream license or remove alternative grants.
LICENSE_BASIS = {
    "(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0": "MIT AND OFL-1.1 AND Ubuntu-font-1.0",
    "(MIT OR Apache-2.0) AND Unicode-3.0": "MIT AND Unicode-3.0",
    "0BSD OR MIT OR Apache-2.0": "MIT",
    "Apache-2.0": "Apache-2.0",
    "Apache-2.0 AND MIT": "Apache-2.0 AND MIT",
    "Apache-2.0 OR GPL-2.0-only": "Apache-2.0",
    "Apache-2.0 OR MIT": "MIT",
    "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT": "MIT",
    "Apache-2.0/MIT": "MIT",
    "BSD-2-Clause OR Apache-2.0 OR MIT": "MIT",
    "BSD-3-Clause OR Apache-2.0": "Apache-2.0",
    "BSL-1.0": "BSL-1.0",
    "ISC": "ISC",
    "MIT": "MIT",
    "MIT / Apache-2.0": "MIT",
    "MIT OR Apache-2.0": "MIT",
    "MIT OR Apache-2.0 OR Zlib": "MIT",
    "MIT OR Zlib OR Apache-2.0": "MIT",
    "MIT/Apache-2.0": "MIT",
    "MPL-2.0": "MPL-2.0",
    "Unlicense OR MIT": "MIT",
    "Zlib": "Zlib",
    "Zlib OR Apache-2.0 OR MIT": "MIT",
}

LICENSE_FILES = {
    "Apache-2.0": "third_party/licenses/Apache-2.0.txt",
    "BSL-1.0": "third_party/licenses/BSL-1.0.txt",
    "ISC": "third_party/licenses/ISC.txt",
    "MIT": "third_party/licenses/MIT.txt",
    "MPL-2.0": "third_party/licenses/MPL-2.0.txt",
    "OFL-1.1": "third_party/licenses/OFL-1.1.txt",
    "Ubuntu-font-1.0": "third_party/licenses/Ubuntu-font-1.0.txt",
    "Unicode-3.0": "third_party/licenses/Unicode-3.0.txt",
    "Zlib": "third_party/licenses/Zlib.txt",
}

REVIEWED_FILE_SHA256 = {
    "third_party/licenses/Apache-2.0.txt": "5c9817c129b98e7bb966bca028c43c19107102ef8e03fe799bffb4354f4ef015",
    "third_party/licenses/BSL-1.0.txt": "c9bff75738922193e67fa726fa225535870d2aa1059f91452c411736284ad566",
    "third_party/licenses/ISC.txt": "f9f71a6ad91a98fa0b76db24cb023284115dd81a3fcdb549a98110f76c1ff39c",
    "third_party/licenses/MIT.txt": "508a77d2e7b51d98adeed32648ad124b7b30241a8e70b2e72c99f92d8e5874d1",
    "third_party/licenses/MPL-2.0.txt": "8b6d19b5a244372e4ea067e567500a49c574f3d5be76cd24851fef83033afdd7",
    "third_party/licenses/OFL-1.1.txt": "71801033d3c6353ba9400dc14791eecdd6a40dea827a557b2e2c22e36a997ff7",
    "third_party/licenses/README.md": "ce72ff330dd0526bdb82ee1e91c48953cdb27575721c6ab30f74d45c20849823",
    "third_party/licenses/SQLite-public-domain.txt": "5a10af5f046b9150058e4f0b4367dec9aa49e130a8cfe406c982ec6a55131ca3",
    "third_party/licenses/Ubuntu-font-1.0.txt": "0b734ab0b6f2742f42989c73282a8dcde87bbca0f0a624f79d4c03573fc9d10d",
    "third_party/licenses/Unicode-3.0.txt": "b0e06cbf38f2dae20705f48ed920c724259224e9bdf7591570466926ef21268e",
    "third_party/licenses/Zlib.txt": "7d86e948c734430d7e7be721daa3390c69d7c5d427025e884478ef0a68853bf5",
    "third_party/licenses/epaint-default-fonts-notices.txt": "6afe55bc5bd0b2302712f63c0d98e976963f44c3d177b303fd61db85241ddd66",
}


@dataclass(frozen=True)
class NoticePackage:
    name: str
    version: str
    license: str
    basis: str
    targets: tuple[str, ...]
    authors: tuple[str, ...]
    upstream: str
    checksum: str


def cargo_metadata(target: str) -> dict[str, object]:
    result = subprocess.run(
        [
            "cargo",
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--filter-platform",
            target,
        ],
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        detail = (result.stdout + result.stderr).strip()
        raise RuntimeError(f"cargo metadata failed for {target}: {detail}")
    return json.loads(result.stdout)


def reachable_package_ids(metadata: dict[str, object]) -> set[str]:
    packages = metadata["packages"]
    workspace_members = set(metadata["workspace_members"])
    roots = [
        package["id"]
        for package in packages
        if package["id"] in workspace_members and package["name"] == ROOT_PACKAGE
    ]
    if len(roots) != 1:
        raise RuntimeError(
            f"expected one {ROOT_PACKAGE} workspace root, found {len(roots)}"
        )

    resolve = metadata.get("resolve")
    if not isinstance(resolve, dict):
        raise RuntimeError("cargo metadata did not return a resolve graph")
    nodes = {node["id"]: node for node in resolve["nodes"]}
    pending = deque(roots)
    reachable: set[str] = set()
    while pending:
        package_id = pending.popleft()
        if package_id in reachable:
            continue
        reachable.add(package_id)
        node = nodes.get(package_id)
        if node is None:
            raise RuntimeError(f"resolve graph is missing node {package_id}")
        for dependency in node["deps"]:
            dep_kinds = dependency.get("dep_kinds", [])
            if any(dep_kind.get("kind") != "dev" for dep_kind in dep_kinds):
                pending.append(dependency["pkg"])
    return reachable


def lock_checksums() -> dict[tuple[str, str, str], str]:
    data = tomllib.loads((REPO_ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    checksums: dict[tuple[str, str, str], str] = {}
    for package in data["package"]:
        source = package.get("source")
        checksum = package.get("checksum")
        if source is not None and checksum is not None:
            checksums[(package["name"], package["version"], source)] = checksum
    return checksums


def collect_packages() -> list[NoticePackage]:
    target_membership: dict[str, set[str]] = {}
    package_records: dict[str, dict[str, object]] = {}
    for target_name, target_triple in TARGETS:
        metadata = cargo_metadata(target_triple)
        by_id = {package["id"]: package for package in metadata["packages"]}
        for package_id in reachable_package_ids(metadata):
            package = by_id[package_id]
            if package.get("source") is None:
                continue
            package_records[package_id] = package
            target_membership.setdefault(package_id, set()).add(target_name)

    checksums = lock_checksums()
    notices: list[NoticePackage] = []
    target_order = {name: index for index, (name, _) in enumerate(TARGETS)}
    for package_id, package in package_records.items():
        source = package.get("source")
        if source != CRATES_IO_SOURCE:
            raise RuntimeError(
                f"{package['name']} {package['version']} is not from reviewed crates.io source: {source}"
            )
        license_expression = package.get("license")
        if not isinstance(license_expression, str) or not license_expression:
            raise RuntimeError(
                f"{package['name']} {package['version']} has no declared license"
            )
        basis = LICENSE_BASIS.get(license_expression)
        if basis is None:
            raise RuntimeError(
                f"unreviewed license expression for {package['name']} {package['version']}: {license_expression}"
            )
        package_root = Path(package["manifest_path"]).parent
        upstream_notices = [
            candidate.name
            for candidate in package_root.iterdir()
            if candidate.is_file()
            and candidate.name.casefold() in {"notice", "notice.md", "notice.txt"}
        ]
        if upstream_notices:
            raise RuntimeError(
                f"unreviewed upstream NOTICE for {package['name']} {package['version']}: "
                + ", ".join(sorted(upstream_notices))
            )
        checksum = checksums.get((package["name"], package["version"], source))
        if checksum is None:
            raise RuntimeError(
                f"Cargo.lock checksum missing for {package['name']} {package['version']}"
            )
        upstream = package.get("repository") or package.get("homepage")
        if not isinstance(upstream, str) or not upstream:
            upstream = f"https://crates.io/crates/{package['name']}/{package['version']}"
        authors = tuple(author for author in package.get("authors", []) if author)
        targets = tuple(
            sorted(target_membership[package_id], key=target_order.__getitem__)
        )
        notices.append(
            NoticePackage(
                name=package["name"],
                version=package["version"],
                license=license_expression,
                basis=basis,
                targets=targets,
                authors=authors,
                upstream=upstream,
                checksum=checksum,
            )
        )
    return sorted(notices, key=lambda package: (package.name.casefold(), package.version))


def markdown_escape(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")


def inventory_digest(packages: list[NoticePackage]) -> str:
    rows = [
        "\t".join(
            (
                package.name,
                package.version,
                package.license,
                package.basis,
                ",".join(package.targets),
                package.checksum,
            )
        )
        for package in packages
    ]
    return hashlib.sha256("\n".join(rows).encode("utf-8")).hexdigest()


def render(packages: list[NoticePackage]) -> str:
    counts = {
        target_name: sum(target_name in package.targets for package in packages)
        for target_name, _ in TARGETS
    }
    lock_digest = hashlib.sha256((REPO_ROOT / "Cargo.lock").read_bytes()).hexdigest()
    lines = [
        "<!-- Generated by scripts/generate-third-party-notices.py; do not edit by hand. -->",
        "# RadishMemory third-party notices",
        "",
        "This inventory covers the locked normal and build dependency graph reachable from",
        "`radishmemory-desktop` for the reviewed ARM64 desktop targets. First-party workspace",
        "packages are excluded. It is a distribution supplement to the RadishMemory",
        "[source-available license](LICENSE), not a change to that license.",
        "",
        f"- Inventory entries: **{len(packages)}** unique crates",
        f"- Target entries: macOS **{counts['macOS']}**, Linux **{counts['Linux']}**, Windows **{counts['Windows']}**",
        f"- Cargo.lock SHA-256: `{lock_digest}`",
        f"- Reviewed inventory SHA-256: `{inventory_digest(packages)}`",
        "- Reproduce: `python3 scripts/generate-third-party-notices.py --check`",
        "",
        "The “distribution basis” column records the license branch selected when an",
        "upstream crate offers alternatives. Selection does not relicense upstream work or",
        "remove any alternative grant. License texts are stored under",
        "[`third_party/licenses/`](third_party/licenses/README.md). Package authors and the",
        "upstream project link provide the corresponding attribution and source location.",
        "",
        "## Locked Rust dependency inventory",
        "",
        "| Package | Declared license | Distribution basis | Targets | Attribution / upstream | Cargo checksum |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for package in packages:
        attribution = ", ".join(package.authors) or "Upstream project"
        upstream = f"[{markdown_escape(attribution)}]({package.upstream})"
        lines.append(
            "| "
            + " | ".join(
                (
                    f"`{markdown_escape(package.name)} {markdown_escape(package.version)}`",
                    f"`{markdown_escape(package.license)}`",
                    f"`{markdown_escape(package.basis)}`",
                    ", ".join(package.targets),
                    upstream,
                    f"`{package.checksum}`",
                )
            )
            + " |"
        )

    lines.extend(
        [
            "",
            "## Additional bundled material",
            "",
            "- `epaint_default_fonts 0.36.1` embeds Hack, Noto Emoji, Ubuntu Light and",
            "  emoji-icon-font. Their package-specific copyright, public-domain and reserved",
            "  font-name notices are preserved in",
            "  [`epaint-default-fonts-notices.txt`](third_party/licenses/epaint-default-fonts-notices.txt),",
            "  alongside the MIT, SIL Open Font License 1.1 and Ubuntu Font Licence 1.0 texts.",
            "- `libsqlite3-sys 0.38.2` compiles bundled SQLite `3.53.2`; the Rust wrapper's",
            "  declared license is in the inventory and SQLite's public-domain dedication is",
            "  preserved in [`SQLite-public-domain.txt`](third_party/licenses/SQLite-public-domain.txt).",
            "",
            "## Platform components not bundled by Cargo",
            "",
            "Operating-system frameworks, drivers, desktop services and dialog backends used",
            "at runtime are not copied into the Rust dependency inventory. Their target",
            "conditions and native / IPC surfaces are recorded in",
            "[Phase 1 third-party and platform dependency review](docs/implementation/phase1-third-party-notices.md).",
            "",
        ]
    )
    return "\n".join(lines)


def check_license_files() -> None:
    selected_identifiers = {
        identifier
        for basis in LICENSE_BASIS.values()
        for identifier in basis.split(" AND ")
    }
    if selected_identifiers != set(LICENSE_FILES):
        raise RuntimeError(
            "reviewed license text mapping does not match the selected distribution basis"
        )
    if not set(LICENSE_FILES.values()).issubset(REVIEWED_FILE_SHA256):
        raise RuntimeError("a selected license text is missing a reviewed SHA-256")
    missing = [path for path in REVIEWED_FILE_SHA256 if not (REPO_ROOT / path).is_file()]
    if missing:
        raise RuntimeError(f"missing reviewed license files: {', '.join(sorted(missing))}")
    drifted = [
        path
        for path, expected in REVIEWED_FILE_SHA256.items()
        if hashlib.sha256((REPO_ROOT / path).read_bytes()).hexdigest() != expected
    ]
    if drifted:
        raise RuntimeError(
            f"reviewed license files changed without hash review: {', '.join(sorted(drifted))}"
        )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail unless THIRD_PARTY_NOTICES.md exactly matches the locked target graphs",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        check_license_files()
        rendered = render(collect_packages())
    except (OSError, RuntimeError, KeyError, TypeError, json.JSONDecodeError) as exc:
        print(f"third-party notice generation failed: {exc}", file=sys.stderr)
        return 1

    if args.check:
        if not OUTPUT_PATH.is_file() or OUTPUT_PATH.read_text(encoding="utf-8") != rendered:
            print(
                "THIRD_PARTY_NOTICES.md is missing or differs from the locked desktop target graphs; "
                "run scripts/generate-third-party-notices.py",
                file=sys.stderr,
            )
            return 1
        print("Third-party notice inventory is current.")
        return 0

    OUTPUT_PATH.write_text(rendered, encoding="utf-8", newline="\n")
    print(f"Wrote {OUTPUT_PATH.relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
