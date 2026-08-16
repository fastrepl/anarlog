#!/usr/bin/env python3

from __future__ import annotations

import json
import os
import sys
import tomllib
from pathlib import Path


REQUIRED_NOTICE = "are not licensed under the MIT License"
DEPENDENCY_SECTIONS = (
    "dependencies",
    "dev-dependencies",
    "build-dependencies",
)
NODE_DEPENDENCY_SECTIONS = (
    "dependencies",
    "devDependencies",
    "optionalDependencies",
    "peerDependencies",
)
IGNORED_DIRECTORIES = {".git", "node_modules", "target"}


def is_within(path: Path, parent: Path) -> bool:
    try:
        path.resolve().relative_to(parent.resolve())
        return True
    except ValueError:
        return False


def manifests(root: Path, filename: str) -> list[Path]:
    paths = []
    for current, directories, filenames in os.walk(root):
        directories[:] = [
            directory
            for directory in directories
            if directory not in IGNORED_DIRECTORIES
        ]
        if filename in filenames:
            paths.append(Path(current) / filename)
    return sorted(paths)


def rust_dependency_tables(manifest: dict[str, object]) -> list[dict[str, object]]:
    tables = [
        table
        for section in DEPENDENCY_SECTIONS
        if isinstance((table := manifest.get(section)), dict)
    ]
    targets = manifest.get("target")
    if isinstance(targets, dict):
        for target in targets.values():
            if not isinstance(target, dict):
                continue
            tables.extend(
                table
                for section in DEPENDENCY_SECTIONS
                if isinstance((table := target.get(section)), dict)
            )
    workspace = manifest.get("workspace")
    if isinstance(workspace, dict) and isinstance(
        (dependencies := workspace.get("dependencies")), dict
    ):
        tables.append(dependencies)
    return tables


def enterprise_rust_packages(enterprise: Path) -> set[str]:
    packages = set()
    for path in manifests(enterprise, "Cargo.toml"):
        package = tomllib.loads(path.read_text()).get("package")
        if isinstance(package, dict) and isinstance(package.get("name"), str):
            packages.add(package["name"])
    return packages


def enterprise_node_packages(enterprise: Path) -> set[str]:
    packages = set()
    for path in manifests(enterprise, "package.json"):
        package = json.loads(path.read_text())
        if isinstance(package.get("name"), str):
            packages.add(package["name"])
    return packages


def check_boundary(root: Path) -> list[str]:
    root = root.resolve()
    enterprise = root / "enterprise"
    errors = []

    root_notice = root / "LICENSE.enterprise"
    directory_notice = enterprise / "LICENSE"
    for notice in (root_notice, directory_notice):
        if not notice.is_file():
            errors.append(
                f"missing commercial license notice: {notice.relative_to(root)}"
            )
        elif REQUIRED_NOTICE not in notice.read_text():
            errors.append(
                f"invalid commercial license notice: {notice.relative_to(root)}"
            )
    if root_notice.is_file() and directory_notice.is_file():
        if root_notice.read_bytes() != directory_notice.read_bytes():
            errors.append("LICENSE.enterprise and enterprise/LICENSE must be identical")

    enterprise_rust = enterprise_rust_packages(enterprise)
    for path in manifests(root, "Cargo.toml"):
        if is_within(path, enterprise):
            continue
        manifest = tomllib.loads(path.read_text())
        for table in rust_dependency_tables(manifest):
            for dependency_name, specification in table.items():
                package_name = dependency_name
                if isinstance(specification, dict):
                    if isinstance(specification.get("package"), str):
                        package_name = specification["package"]
                    dependency_path = specification.get("path")
                    if isinstance(dependency_path, str) and is_within(
                        path.parent / dependency_path, enterprise
                    ):
                        errors.append(
                            f"MIT Rust manifest {path.relative_to(root)} depends on enterprise path {dependency_path}"
                        )
                if package_name in enterprise_rust:
                    errors.append(
                        f"MIT Rust manifest {path.relative_to(root)} depends on enterprise package {package_name}"
                    )

    enterprise_node = enterprise_node_packages(enterprise)
    for path in manifests(root, "package.json"):
        if is_within(path, enterprise):
            continue
        package = json.loads(path.read_text())
        for section in NODE_DEPENDENCY_SECTIONS:
            dependencies = package.get(section)
            if not isinstance(dependencies, dict):
                continue
            for dependency_name, specification in dependencies.items():
                if dependency_name in enterprise_node:
                    errors.append(
                        f"MIT JavaScript package {path.relative_to(root)} depends on enterprise package {dependency_name}"
                    )
                if not isinstance(specification, str):
                    continue
                if any(
                    specification == f"npm:{package_name}"
                    or specification.startswith(f"npm:{package_name}@")
                    for package_name in enterprise_node
                ):
                    errors.append(
                        f"MIT JavaScript package {path.relative_to(root)} aliases an enterprise package"
                    )
                protocol, separator, dependency_path = specification.partition(":")
                if not separator:
                    dependency_path = specification
                is_local_reference = not separator or protocol in {
                    "file",
                    "link",
                    "portal",
                    "workspace",
                }
                if is_local_reference and is_within(
                    path.parent / dependency_path, enterprise
                ):
                    errors.append(
                        f"MIT JavaScript package {path.relative_to(root)} depends on enterprise path {dependency_path}"
                    )

    return errors


def main() -> None:
    root = Path(__file__).resolve().parents[2]
    errors = check_boundary(root)
    if errors:
        print("License boundary check failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        raise SystemExit(1)
    print("License boundary check passed")


if __name__ == "__main__":
    main()
