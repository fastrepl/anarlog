#!/usr/bin/env python3

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from check_license_boundary import REQUIRED_NOTICE, check_boundary


NOTICE = f"Enterprise files {REQUIRED_NOTICE}.\n"


class LicenseBoundaryTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        (self.root / "enterprise" / "crates" / "server").mkdir(parents=True)
        (self.root / "crates" / "client").mkdir(parents=True)
        (self.root / "enterprise" / "packages" / "admin").mkdir(parents=True)
        (self.root / "packages" / "app").mkdir(parents=True)
        (self.root / "LICENSE.enterprise").write_text(NOTICE)
        (self.root / "enterprise" / "LICENSE").write_text(NOTICE)
        (self.root / "enterprise" / "crates" / "server" / "Cargo.toml").write_text(
            '[package]\nname = "enterprise-server"\nversion = "0.1.0"\n'
        )
        (self.root / "crates" / "client" / "Cargo.toml").write_text(
            '[package]\nname = "community-client"\nversion = "0.1.0"\n'
        )
        (self.root / "enterprise" / "packages" / "admin" / "package.json").write_text(
            '{"name":"@anlg/enterprise-admin"}\n'
        )
        (self.root / "packages" / "app" / "package.json").write_text(
            '{"name":"@anlg/app"}\n'
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def test_accepts_one_way_enterprise_boundary(self) -> None:
        self.assertEqual(check_boundary(self.root), [])

    def test_rejects_rust_path_from_community_to_enterprise(self) -> None:
        (self.root / "crates" / "client" / "Cargo.toml").write_text(
            '[package]\nname = "community-client"\nversion = "0.1.0"\n'
            '[dependencies]\nenterprise-server = { path = "../../enterprise/crates/server" }\n'
        )

        errors = check_boundary(self.root)

        self.assertTrue(any("depends on enterprise path" in error for error in errors))

    def test_rejects_workspace_rust_dependency_by_enterprise_package_name(self) -> None:
        (self.root / "crates" / "client" / "Cargo.toml").write_text(
            '[package]\nname = "community-client"\nversion = "0.1.0"\n'
            "[dependencies]\nenterprise-server = { workspace = true }\n"
        )

        errors = check_boundary(self.root)

        self.assertTrue(
            any("enterprise package enterprise-server" in error for error in errors)
        )

    def test_rejects_virtual_workspace_rust_dependency_on_enterprise_path(self) -> None:
        (self.root / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["crates/client"]\n'
            '[workspace.dependencies]\nserver = { path = "enterprise/crates/server", package = "enterprise-server" }\n'
        )

        errors = check_boundary(self.root)

        self.assertTrue(any("depends on enterprise path" in error for error in errors))

    def test_rejects_javascript_dependency_on_enterprise_package(self) -> None:
        (self.root / "packages" / "app" / "package.json").write_text(
            '{"name":"@anlg/app","dependencies":{"@anlg/enterprise-admin":"workspace:*"}}\n'
        )

        errors = check_boundary(self.root)

        self.assertTrue(
            any(
                "enterprise package @anlg/enterprise-admin" in error for error in errors
            )
        )

    def test_rejects_javascript_alias_for_enterprise_package(self) -> None:
        for specification in (
            "npm:@anlg/enterprise-admin",
            "npm:@anlg/enterprise-admin@1.0.0",
        ):
            with self.subTest(specification=specification):
                (self.root / "packages" / "app" / "package.json").write_text(
                    '{"name":"@anlg/app","dependencies":{"admin":"'
                    + specification
                    + '"}}\n'
                )

                errors = check_boundary(self.root)

                self.assertTrue(
                    any("aliases an enterprise package" in error for error in errors)
                )

    def test_rejects_javascript_workspace_path_into_enterprise(self) -> None:
        (self.root / "packages" / "app" / "package.json").write_text(
            '{"name":"@anlg/app","dependencies":{"admin":"workspace:../../enterprise/packages/admin"}}\n'
        )

        errors = check_boundary(self.root)

        self.assertTrue(any("depends on enterprise path" in error for error in errors))

    def test_rejects_javascript_root_paths_without_dot_prefix(self) -> None:
        for specification in (
            "file:enterprise/packages/admin",
            "enterprise/packages/admin",
        ):
            with self.subTest(specification=specification):
                (self.root / "package.json").write_text(
                    '{"name":"root","dependencies":{"admin":"' + specification + '"}}\n'
                )

                errors = check_boundary(self.root)

                self.assertTrue(
                    any("depends on enterprise path" in error for error in errors)
                )

    def test_rejects_mismatched_commercial_notices(self) -> None:
        (self.root / "enterprise" / "LICENSE").write_text(NOTICE + "changed\n")

        errors = check_boundary(self.root)

        self.assertIn(
            "LICENSE.enterprise and enterprise/LICENSE must be identical", errors
        )


if __name__ == "__main__":
    unittest.main()
