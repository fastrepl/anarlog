import json
import os
from pathlib import Path
import stat
import struct
import sys
import tempfile
import unittest
from types import SimpleNamespace
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bundle  # noqa: E402


FAKE_PRINT_PACKAGE_SCRIPT = """#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

if "--print-package" not in sys.argv:
    sys.exit(2)

# Mirror platform/package.rs: `APPDIR` and executable path determine resources & executables.
exe = Path(sys.argv[0]).resolve()
executables = exe.parent
product = os.environ.get("TEST_PRODUCT", "Anarlog Staging")
beside = executables.parent / "lib" / product
if beside.is_dir():
    resources = beside
else:
    appdir = Path(os.environ.get("APPDIR", "/"))
    resources = appdir / "usr" / "lib" / product

env_log = os.environ.get("TEST_ENV_LOG")
if env_log:
    Path(env_log).write_text(json.dumps({
        "APPDIR": os.environ.get("APPDIR"),
        "APPIMAGE": os.environ.get("APPIMAGE"),
        "ANARLOG_NATIVE_RESOURCES": os.environ.get("ANARLOG_NATIVE_RESOURCES"),
    }))

print(json.dumps({
    "channel": os.environ.get("TEST_CHANNEL", "staging"),
    "version": os.environ.get("TEST_VERSION", "1.5.0"),
    "bundle_type": os.environ.get("TEST_BUNDLE_TYPE"),
    "resources": str(resources),
    "executables": str(executables),
    "seen_appdir": os.environ.get("APPDIR"),
    "seen_appimage": os.environ.get("APPIMAGE"),
    "seen_native_resources": os.environ.get("ANARLOG_NATIVE_RESOURCES"),
}))
"""


def write_fake_appdir_contents(
    root: Path,
    *,
    product: str,
    exe: str,
    channel: str,
    version: str,
    bundle_type: str | None,
    marker: bytes = b"__TAURI_BUNDLE_TYPE_VAR_APP",
    include_sidecar: bool = True,
    include_resource: bool = True,
    apprun_mode: int = 0o755,
    apprun_is_dir: bool = False,
):
    root.mkdir(parents=True, exist_ok=True)
    apprun = root / "AppRun"
    if apprun_is_dir:
        apprun.mkdir(parents=True, exist_ok=True)
    else:
        apprun.write_text("#!/bin/sh\nexit 0\n")
        apprun.chmod(apprun_mode)
    usr_bin = root / "usr/bin"
    usr_bin.mkdir(parents=True, exist_ok=True)
    binary = usr_bin / exe
    script = (
        FAKE_PRINT_PACKAGE_SCRIPT.replace(
            'os.environ.get("TEST_PRODUCT", "Anarlog Staging")', repr(product)
        )
        .replace('os.environ.get("TEST_CHANNEL", "staging")', repr(channel))
        .replace('os.environ.get("TEST_VERSION", "1.5.0")', repr(version))
        .replace('os.environ.get("TEST_BUNDLE_TYPE")', repr(bundle_type))
        + f"\n# {marker.decode('ascii', errors='replace')}\n"
    )
    binary.write_text(script)
    binary.chmod(0o755)
    if include_sidecar:
        for sidecar in bundle.EXTERNAL_BIN["linux"]:
            path = usr_bin / Path(sidecar).name
            path.write_text("#!/bin/sh\nexit 0\n")
            path.chmod(0o755)
    if include_resource:
        res_dir = root / "usr/lib" / product / "icons"
        res_dir.mkdir(parents=True, exist_ok=True)
        (res_dir / "staging.icns").write_bytes(b"icns")


def write_self_extracting_appimage(
    path: Path,
    *,
    product: str,
    exe: str,
    channel: str,
    version: str,
    bundle_type: str | None = "AppImage",
    include_sidecar: bool = True,
    include_resource: bool = True,
    apprun_mode: int = 0o755,
    apprun_is_dir: bool = False,
):
    payload_dir = path.parent / "_payload_src"
    payload_dir.mkdir(parents=True, exist_ok=True)
    write_fake_appdir_contents(
        payload_dir,
        product=product,
        exe=exe,
        channel=channel,
        version=version,
        bundle_type=bundle_type,
        include_sidecar=include_sidecar,
        include_resource=include_resource,
        apprun_mode=apprun_mode,
        apprun_is_dir=apprun_is_dir,
    )
    script = f"""#!/usr/bin/env python3
import os, shutil, sys
from pathlib import Path
if "--appimage-extract" in sys.argv:
    dest = Path.cwd() / "squashfs-root"
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree({str(payload_dir)!r}, dest)
    # Record APPDIR/APPIMAGE check if invoked with --print-package directly
    sys.exit(0)
sys.exit(1)
"""
    path.write_text(script)
    path.chmod(0o644)


def make_config(product="Anarlog Staging", exe="anarlog-staging"):
    return {
        "productName": product,
        "mainBinaryName": exe,
        "identifier": "com.hyprnote.staging",
        "bundle": {
            "resources": {"src/staging.icns": "icons/staging.icns"},
            "linux": {"deb": {"depends": []}},
        },
        "plugins": {"deep-link": {"desktop": {"schemes": ["anarlog-staging"]}}},
    }


def make_args(bundle_dir: Path, *, target="x86_64-unknown-linux-gnu"):
    return SimpleNamespace(
        channel="staging",
        target=target,
        version="1.5.0",
        no_sign=True,
        team_id=None,
        bundles="appimage",
        bundle_dir=bundle_dir,
    )


class VerifyLinuxAppImageTests(unittest.TestCase):
    def test_host_arch_extract_verifies_appimage_and_preserves_artifact_mode(self):
        with tempfile.TemporaryDirectory() as tmp:
            bundle_dir = Path(tmp) / "bundle"
            appimage_dir = bundle_dir / "appimage"
            appimage_dir.mkdir(parents=True)
            appimage = appimage_dir / "Anarlog Staging_1.5.0_amd64.AppImage"
            write_self_extracting_appimage(
                appimage,
                product="Anarlog Staging",
                exe="anarlog-staging",
                channel="staging",
                version="1.5.0",
                bundle_type="AppImage",
            )
            before_mode = stat.S_IMODE(appimage.stat().st_mode)
            self.assertEqual(before_mode, 0o644)

            checks = bundle.Checks()
            seen_env = {}
            real_check_binary = bundle.check_binary

            def spy_check_binary(binary, expected, args, chk, extra_env=None):
                seen_env["extra_env"] = extra_env
                # Also execute the binary directly to inspect what env vars reached it.
                real_check_binary(binary, expected, args, chk, extra_env=extra_env)

            env_log = Path(tmp) / "env.json"
            with (
                mock.patch.dict(
                    os.environ,
                    {
                        "ANARLOG_NATIVE_RESOURCES": "/stale/resources",
                        "APPDIR": "/stale/appdir",
                        "APPIMAGE": "/stale/appimage",
                        "TEST_ENV_LOG": str(env_log),
                    },
                ),
                mock.patch.object(bundle, "can_run_linux_target", return_value=True),
                mock.patch.object(bundle, "check_binary", side_effect=spy_check_binary),
            ):
                bundle.verify_linux(
                    make_config(),
                    make_args(bundle_dir),
                    checks,
                )

            self.assertEqual(checks.failures, [])
            self.assertEqual(stat.S_IMODE(appimage.stat().st_mode), 0o644)
            self.assertIsNotNone(seen_env["extra_env"])
            self.assertTrue(seen_env["extra_env"]["APPDIR"].endswith("squashfs-root"))
            self.assertEqual(
                Path(seen_env["extra_env"]["APPIMAGE"]), appimage.resolve()
            )
            child_env = json.loads(env_log.read_text())
            self.assertEqual(child_env["APPDIR"], seen_env["extra_env"]["APPDIR"])
            self.assertEqual(child_env["APPIMAGE"], seen_env["extra_env"]["APPIMAGE"])
            self.assertIsNone(child_env["ANARLOG_NATIVE_RESOURCES"])

    def test_host_arch_reports_unpatched_marker_missing_sidecar_and_missing_resource(
        self,
    ):
        with tempfile.TemporaryDirectory() as tmp:
            bundle_dir = Path(tmp) / "bundle"
            appimage_dir = bundle_dir / "appimage"
            appimage_dir.mkdir(parents=True)
            appimage = appimage_dir / "Anarlog Staging_1.5.0_amd64.AppImage"
            write_self_extracting_appimage(
                appimage,
                product="Anarlog Staging",
                exe="anarlog-staging",
                channel="staging",
                version="1.5.0",
                bundle_type=None,
                include_sidecar=False,
                include_resource=False,
            )

            checks = bundle.Checks()
            with mock.patch.object(bundle, "can_run_linux_target", return_value=True):
                bundle.verify_linux(
                    make_config(),
                    make_args(bundle_dir),
                    checks,
                )

            joined = "\n".join(checks.failures)
            self.assertIn("package type", joined)
            self.assertIn("anarlog-cli", joined)
            self.assertIn("Missing /usr/lib/Anarlog Staging", joined)

    def test_cross_arch_unsquashfs_fallback_checks_offset_and_bundle_marker(self):
        for marker, expect_ok in [
            (b"__TAURI_BUNDLE_TYPE_VAR_APP", True),
            (b"__TAURI_BUNDLE_TYPE_VAR_UNK", False),
        ]:
            with self.subTest(marker=marker):
                with tempfile.TemporaryDirectory() as tmp:
                    tmp_path = Path(tmp)
                    bundle_dir = tmp_path / "bundle"
                    appimage_dir = bundle_dir / "appimage"
                    appimage_dir.mkdir(parents=True)
                    appimage = appimage_dir / "Anarlog Staging_1.5.0_aarch64.AppImage"

                    # Construct minimal ELF64 little-endian header: e_shoff=64, e_shentsize=64, e_shnum=1 -> offset=128
                    elf = bytearray(128)
                    elf[0:4] = b"\x7fELF"
                    elf[4] = 2  # ELFCLASS64
                    elf[5] = 1  # ELFDATA2LSB
                    struct.pack_into("<Q", elf, 40, 64)
                    struct.pack_into("<HH", elf, 58, 64, 1)
                    appimage.write_bytes(
                        bytes(elf) + b"hsqs_synthetic_squashfs_payload"
                    )

                    payload_dir = tmp_path / "payload"
                    write_fake_appdir_contents(
                        payload_dir,
                        product="Anarlog Staging",
                        exe="anarlog-staging",
                        channel="staging",
                        version="1.5.0",
                        bundle_type="AppImage",
                        marker=marker,
                    )

                    bin_dir = tmp_path / "fakebin"
                    bin_dir.mkdir()
                    offset_log = tmp_path / "seen_offset.txt"
                    fake_unsquashfs = bin_dir / "unsquashfs"
                    fake_unsquashfs.write_text(
                        f"""#!/usr/bin/env python3
import shutil, sys
from pathlib import Path
args = sys.argv[1:]
dest = Path(args[args.index("-d") + 1])
offset = args[args.index("-o") + 1]
Path({str(offset_log)!r}).write_text(offset)
if dest.exists():
    shutil.rmtree(dest)
shutil.copytree({str(payload_dir)!r}, dest)
"""
                    )
                    fake_unsquashfs.chmod(0o755)

                    checks = bundle.Checks()
                    with (
                        mock.patch.dict(
                            os.environ,
                            {"PATH": f"{bin_dir}:{os.environ.get('PATH', '')}"},
                        ),
                        mock.patch.object(
                            bundle, "can_run_linux_target", return_value=False
                        ),
                    ):
                        bundle.verify_linux(
                            make_config(),
                            make_args(bundle_dir, target="aarch64-unknown-linux-gnu"),
                            checks,
                        )

                    self.assertEqual(offset_log.read_text(), "128")
                    if expect_ok:
                        self.assertEqual(checks.failures, [])
                    else:
                        self.assertTrue(
                            any(
                                "__TAURI_BUNDLE_TYPE_VAR_APP" in failure
                                for failure in checks.failures
                            ),
                            checks.failures,
                        )

    def test_rejects_non_executable_or_directory_apprun(self):
        for label, kwargs, expected_fragment in [
            ("non_executable", {"apprun_mode": 0o644}, "not executable"),
            (
                "directory",
                {"apprun_is_dir": True},
                "Missing AppRun launcher file",
            ),
        ]:
            with self.subTest(case=label):
                with tempfile.TemporaryDirectory() as tmp:
                    bundle_dir = Path(tmp) / "bundle"
                    appimage_dir = bundle_dir / "appimage"
                    appimage_dir.mkdir(parents=True)
                    appimage = appimage_dir / "Anarlog Staging_1.5.0_amd64.AppImage"
                    write_self_extracting_appimage(
                        appimage,
                        product="Anarlog Staging",
                        exe="anarlog-staging",
                        channel="staging",
                        version="1.5.0",
                        bundle_type="AppImage",
                        **kwargs,
                    )

                    checks = bundle.Checks()
                    with mock.patch.object(
                        bundle, "can_run_linux_target", return_value=True
                    ):
                        bundle.verify_linux(
                            make_config(),
                            make_args(bundle_dir),
                            checks,
                        )

                    self.assertTrue(
                        any(
                            expected_fragment in failure for failure in checks.failures
                        ),
                        checks.failures,
                    )


if __name__ == "__main__":
    unittest.main()
