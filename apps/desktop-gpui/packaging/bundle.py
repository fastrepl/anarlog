#!/usr/bin/env python3
"""Bundle a prebuilt GPUI binary like the shipping app, and verify the result.

`apps/desktop/src-tauri` stays the Tauri project: Tauri's bundler reads the same
`tauri.conf*.json` files that `desktop_cd.yaml` passes. Bundle ID, executable name,
Info.plist, entitlements, resources, sidecars, installers, signing and updater artifacts
therefore match shipping by construction. Only the binary and `tauri.gpui.json` differ.
The binary finds its files where the bundler puts them, and reports `APP_VERSION`:

  APP_VERSION=1.5.0 cargo build --locked --release
  bundle.py bundle --channel staging --target aarch64-apple-darwin \
      --binary target/release/desktop-gpui --version 1.5.0
  bundle.py verify --channel staging --target aarch64-apple-darwin \
      --version 1.5.0 <out>/aarch64-apple-darwin/release/bundle
"""

import argparse
import glob
import json
import os
import plistlib
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

PACKAGING = Path(__file__).resolve().parent
REPO = PACKAGING.parents[2]
TAURI_DIR = REPO / "apps/desktop/src-tauri"

# The sidecars desktop_cd.yaml stages per platform (with tauri.macos.conf.json). The bundler
# copies `<path>-<target>` to the executable's directory as `<file name>`.
EXTERNAL_BIN = {
    "macos": [
        "binaries/char-chrome-native-host",
        "binaries/check-permissions",
        "resources/cli/anarlog-cli",
    ],
    "linux": ["binaries/char-chrome-native-host", "resources/cli/anarlog-cli"],
    "windows": ["resources/cli/anarlog-cli"],
}

# The bundle types verify checks, and those it expects when `--bundles` doesn't say: what
# Tauri builds for `"targets": "all"` (Linux's rpm aside, which isn't checked).
VERIFIED_BUNDLES = {
    "macos": ["app", "dmg"],
    "linux": ["deb", "appimage"],
    "windows": ["nsis", "msi"],
}

# Shipping's Developer ID designated requirement (see `codesign -d -r-` on a release).
DESIGNATED_REQUIREMENT = (
    'identifier "{identifier}" and anchor apple generic and '
    "certificate 1[field.1.2.840.113635.100.6.2.6] /* exists */ and "
    "certificate leaf[field.1.2.840.113635.100.6.1.13] /* exists */ and "
    'certificate leaf[subject.OU] = "{team}"'
)

# tauri-cli appends these to every deb's Depends (interface/rust.rs), whatever the config says.
TAURI_DEB_DEPENDS = ["libwebkit2gtk-4.1-0", "libgtk-3-0"]

MACHO_CPU = {"aarch64": 0x0100000C, "x86_64": 0x01000007}


def platform_of(target):
    if target.endswith("-apple-darwin"):
        return "macos"
    if "-linux-" in target:
        return "linux"
    if "-windows-" in target:
        return "windows"
    sys.exit(f"Unsupported target {target}")


def requested_bundles(args):
    platform = platform_of(args.target)
    if not args.bundles:
        return set(VERIFIED_BUNDLES[platform])
    kinds = {kind.strip() for kind in args.bundles.split(",") if kind.strip()}
    unknown = kinds - set(VERIFIED_BUNDLES[platform])
    if not kinds or unknown:
        sys.exit(
            f"Can't verify --bundles {args.bundles} for {args.target}; choose from "
            + ",".join(VERIFIED_BUNDLES[platform])
        )
    # Tauri deletes the .app it built a DMG from unless app is requested, and verify checks it.
    if "dmg" in kinds and "app" not in kinds:
        sys.exit("--bundles dmg needs app as well: verify checks the app the DMG holds")
    return kinds


def artifact(config, args, kind, suffix, checks):
    """The bundler names each package `<productName>_<version>_<arch><suffix>`; matching the
    product keeps another channel's package in the same folder from passing for this one."""
    pattern = f"{glob.escape(config['productName'])}_{args.version}_*{suffix}"
    return one(args.bundle_dir / kind, pattern, checks)


def merge(base, patch):
    """RFC 7396 merge patch, which is how Tauri applies each `--config`."""
    if not isinstance(patch, dict):
        return patch
    result = dict(base) if isinstance(base, dict) else {}
    for key, value in patch.items():
        if value is None:
            result.pop(key, None)
        else:
            result[key] = merge(result.get(key), value)
    return result


def config_files(channel, target):
    """The `--config` files desktop_cd.yaml passes for this channel and target, plus GPUI's."""
    files = [TAURI_DIR / f"tauri.conf.{channel}.json"]
    if platform_of(target) == "macos" and channel != "staging":
        files.append(TAURI_DIR / f"tauri.conf.{channel}-macos.json")
    if target == "x86_64-apple-darwin":
        files.append(TAURI_DIR / "tauri.conf.macos-intel.json")
    files.append(PACKAGING / "tauri.gpui.json")
    return files


def load_config(channel, target):
    config = json.loads((TAURI_DIR / "tauri.conf.json").read_text())
    platform_file = TAURI_DIR / f"tauri.{platform_of(target)}.conf.json"
    files = [platform_file] if platform_file.exists() else []
    for path in files + config_files(channel, target):
        config = merge(config, json.loads(path.read_text()))
    return config


def bundle(args):
    platform = platform_of(args.target)
    requested_bundles(args)
    config = load_config(args.channel, args.target)
    exe = ".exe" if platform == "windows" else ""
    out = args.out.resolve()
    release = out / args.target / "release"
    release.mkdir(parents=True, exist_ok=True)

    # A copy, not a link: on Linux and Windows the bundler patches this file in place with
    # the package type (`__TAURI_BUNDLE_TYPE_VAR_*`).
    staged = release / f"{config['mainBinaryName']}{exe}"
    staged.unlink(missing_ok=True)
    shutil.copy2(args.binary, staged)

    missing = [
        sidecar
        for sidecar in EXTERNAL_BIN[platform]
        if not (TAURI_DIR / f"{sidecar}-{args.target}{exe}").is_file()
    ]
    if missing:
        sys.exit(
            f"Missing sidecars under {TAURI_DIR}: "
            + ", ".join(f"{sidecar}-{args.target}{exe}" for sidecar in missing)
            + ". Stage them as desktop_cd.yaml does (see apps/desktop-gpui/README.md)."
        )

    overrides = {
        "version": args.version,
        "bundle": {"externalBin": EXTERNAL_BIN[platform]},
    }
    generated = out / args.target / "tauri.gpui.generated.json"
    generated.write_text(json.dumps(overrides, indent=2) + "\n")

    command = shlex.split(os.environ.get("TAURI_CLI", "pnpm exec tauri"))
    # Windows runs `pnpm.cmd` only by its full path.
    command[0] = shutil.which(command[0]) or command[0]
    command += ["bundle", "--target", args.target, "--verbose"]
    for path in [*config_files(args.channel, args.target), generated]:
        command += ["--config", str(path)]
    if args.bundles:
        command += ["--bundles", args.bundles]
    if args.no_sign:
        command.append("--no-sign")
    # The bundler finds the binary and writes bundles under the cargo target directory.
    env = {**os.environ, "CARGO_TARGET_DIR": str(out)}
    # Without CI the DMG step drives Finder through AppleScript. Shipping's CD opts in with CI=false.
    env.setdefault("CI", "true")
    subprocess.run(command, cwd=REPO / "apps/desktop", env=env, check=True)

    args.bundle_dir = release / "bundle"
    verify(args)


class Checks:
    def __init__(self):
        self.failures = []

    def expect(self, condition, message):
        if not condition:
            self.failures.append(message)

    def equal(self, actual, expected, what):
        self.expect(
            actual == expected, f"{what}: expected {expected!r}, got {actual!r}"
        )


def one(directory, pattern, checks):
    found = sorted(directory.glob(pattern))
    checks.expect(
        len(found) == 1, f"Expected one {pattern} in {directory}, found {found}"
    )
    return found[0] if found else None


def run(*command):
    return subprocess.run(command, capture_output=True, text=True)


def check_binary(binary, expected, args, checks):
    """The app's own view, run from the bundle (`--print-package`, src/platform/package.rs): the
    channel and version it was built with, the package type the bundler patched in, and the
    folders it reads its resources and sidecars from, which must be where they are."""
    data = binary.read_bytes() if binary.is_file() else b""
    # A binary without the flag would ignore it and start the app on this channel's real data.
    if b"--print-package" not in data:
        checks.expect(
            False, f"{binary.name} can't report its package (--print-package)"
        )
        return
    env = {
        key: value
        for key, value in os.environ.items()
        if key not in ("ANARLOG_NATIVE_RESOURCES", "APPDIR", "APPIMAGE")
    }
    try:
        result = subprocess.run(
            [str(binary), "--print-package"],
            capture_output=True,
            text=True,
            env=env,
            timeout=60,
        )
        report = json.loads(result.stdout) if result.returncode == 0 else None
    except (OSError, subprocess.TimeoutExpired, json.JSONDecodeError) as error:
        result, report = None, None
        checks.expect(False, f"{binary.name} --print-package failed: {error}")
    if report is None:
        if result is not None:
            checks.expect(
                False,
                f"{binary.name} --print-package exited {result.returncode}: "
                + result.stderr.strip(),
            )
        return
    checks.equal(report["channel"], args.channel, f"{binary.name} compiled channel")
    checks.equal(report["version"], args.version, f"{binary.name} version")
    checks.equal(
        report["bundle_type"], expected["bundle_type"], f"{binary.name} package type"
    )
    for key in ["resources", "executables"]:
        checks.equal(
            Path(report[key]).resolve() if report[key] else None,
            expected[key].resolve(),
            f"{binary.name} {key} folder",
        )


def verify_macos(config, args, checks):
    product, exe = config["productName"], config["mainBinaryName"]
    app = args.bundle_dir / "macos" / f"{product}.app"
    contents = app / "Contents"
    info = plistlib.loads((contents / "Info.plist").read_bytes())
    expected = plistlib.loads((TAURI_DIR / "Info.plist").read_bytes())
    expected |= {
        "CFBundleIdentifier": config["identifier"],
        "CFBundleExecutable": exe,
        "CFBundleName": product,
        "CFBundleDisplayName": product,
        "CFBundleShortVersionString": args.version,
        "CFBundleVersion": args.version,
        "LSMinimumSystemVersion": config["bundle"]["macOS"]["minimumSystemVersion"],
    }
    for key, value in expected.items():
        checks.equal(info.get(key), value, f"Info.plist {key}")
    schemes = [
        scheme
        for kind in info.get("CFBundleURLTypes", [])
        for scheme in kind["CFBundleURLSchemes"]
    ]
    checks.equal(
        schemes, config["plugins"]["deep-link"]["desktop"]["schemes"], "URL schemes"
    )
    # Shipping launches as a regular app and toggles the Dock icon at runtime.
    checks.expect(
        "LSUIElement" not in info, "Info.plist sets LSUIElement; shipping does not"
    )

    binary = contents / "MacOS" / exe
    header = binary.read_bytes()[:8] if binary.is_file() else b""
    checks.equal(
        int.from_bytes(header[4:8], "little") if len(header) == 8 else None,
        MACHO_CPU[args.target.split("-")[0]],
        f"{binary.name} Mach-O CPU type",
    )
    for sidecar in EXTERNAL_BIN["macos"]:
        path = contents / "MacOS" / Path(sidecar).name
        checks.expect(
            path.is_file() and not path.is_symlink(), f"Missing sidecar {path}"
        )
    for framework in config["bundle"]["macOS"].get("frameworks", []):
        path = contents / "Frameworks" / Path(framework).name
        checks.expect(path.is_file(), f"Missing framework {path}")
    for destination in config["bundle"]["resources"].values():
        path = contents / "Resources" / destination
        checks.expect(path.exists(), f"Missing resource {path}")
    for destination, source in config["bundle"]["macOS"].get("files", {}).items():
        path = contents / destination
        checks.expect(source is None or path.exists(), f"Missing bundle file {path}")

    check_binary(
        binary,
        {
            "bundle_type": "App",
            "resources": contents / "Resources",
            "executables": contents / "MacOS",
        },
        args,
        checks,
    )

    updater = args.bundle_dir / "macos" / f"{product}.app.tar.gz"
    checks.expect(updater.is_file(), f"Missing updater artifact {updater}")
    if not args.no_sign:
        checks.expect(
            updater.with_name(updater.name + ".sig").is_file(),
            "Missing updater signature",
        )
    if "dmg" in requested_bundles(args):
        artifact(config, args, "dmg", ".dmg", checks)

    if not args.no_sign and shutil.which("codesign"):
        verify_macos_signature(config, args, app, checks)


def verify_macos_signature(config, args, app, checks):
    result = run("codesign", "--verify", "--deep", "--strict", "--verbose=2", str(app))
    checks.expect(
        result.returncode == 0, f"codesign --verify failed: {result.stderr.strip()}"
    )
    details = run("codesign", "-dvvv", str(app)).stderr
    fields = dict(line.split("=", 1) for line in details.splitlines() if "=" in line)
    checks.equal(
        fields.get("Identifier"), config["identifier"], "Code signing identifier"
    )
    checks.expect(
        "runtime" in fields.get("CodeDirectory v", ""), "Hardened runtime is off"
    )
    entitlements = run(
        "codesign", "-d", "--entitlements", "-", "--xml", str(app)
    ).stdout
    expected = plistlib.loads(
        (TAURI_DIR / config["bundle"]["macOS"]["entitlements"]).read_bytes()
    )
    checks.equal(
        plistlib.loads(entitlements.encode()) if entitlements.strip() else None,
        expected,
        "Entitlements",
    )
    if args.team_id:
        requirement = run("codesign", "-d", "-r-", str(app)).stdout.strip()
        checks.equal(
            requirement.removeprefix("designated => "),
            DESIGNATED_REQUIREMENT.format(
                identifier=config["identifier"], team=args.team_id
            ),
            "Designated requirement",
        )
        checks.equal(fields.get("TeamIdentifier"), args.team_id, "Team identifier")


def check_signature(artifact, args, checks):
    if artifact and not args.no_sign:
        checks.expect(
            artifact.with_name(artifact.name + ".sig").is_file(),
            f"Missing {artifact}.sig",
        )


def verify_linux(config, args, checks):
    product, exe = config["productName"], config["mainBinaryName"]
    bundles = requested_bundles(args)
    # Tauri keeps earlier versions' packages in the same folder.
    if "appimage" in bundles:
        appimage = artifact(config, args, "appimage", ".AppImage", checks)
        check_signature(appimage, args, checks)
    if "deb" not in bundles:
        return
    deb = artifact(config, args, "deb", ".deb", checks)
    check_signature(deb, args, checks)
    if not deb:
        return

    def field(name):
        return run("dpkg-deb", "--field", str(deb), name).stdout.strip()

    deb_config = config["bundle"]["linux"]["deb"]
    checks.equal(field("Version"), args.version, "deb Version")
    checks.equal(
        [item.strip() for item in field("Depends").split(",") if item.strip()],
        deb_config["depends"] + TAURI_DEB_DEPENDS,
        "deb Depends",
    )
    for relation in ["provides", "conflicts", "replaces"]:
        checks.equal(
            [
                item.strip()
                for item in field(relation.capitalize()).split(",")
                if item.strip()
            ],
            deb_config.get(relation, []),
            f"deb {relation.capitalize()}",
        )
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        subprocess.run(["dpkg-deb", "-x", str(deb), str(root)], check=True)
        checks.expect((root / "usr/bin" / exe).is_file(), f"Missing /usr/bin/{exe}")
        check_binary(
            root / "usr/bin" / exe,
            {
                "bundle_type": "Deb",
                "resources": root / "usr/lib" / product,
                "executables": root / "usr/bin",
            },
            args,
            checks,
        )
        for sidecar in EXTERNAL_BIN["linux"]:
            name = Path(sidecar).name
            checks.expect(
                (root / "usr/bin" / name).is_file(), f"Missing /usr/bin/{name}"
            )
        for destination in config["bundle"]["resources"].values():
            path = root / "usr/lib" / product / destination
            checks.expect(path.exists(), f"Missing resource /{path.relative_to(root)}")
        desktop = root / "usr/share/applications" / f"{product}.desktop"
        entry = desktop.read_text() if desktop.is_file() else ""
        checks.expect(f"Exec={exe}" in entry, f"{desktop.name} does not launch {exe}")
        for scheme in config["plugins"]["deep-link"]["desktop"]["schemes"]:
            checks.expect(
                f"x-scheme-handler/{scheme}" in entry,
                f"{desktop.name} lacks {scheme}://",
            )


def verify_windows(config, args, checks):
    bundles = requested_bundles(args)
    if "nsis" in bundles:
        installer = artifact(config, args, "nsis", "-setup.exe", checks)
        check_signature(installer, args, checks)
    if "msi" in bundles:
        installer = artifact(config, args, "msi", ".msi", checks)
        check_signature(installer, args, checks)


def verify(args):
    config = load_config(args.channel, args.target)
    checks = Checks()
    {"macos": verify_macos, "linux": verify_linux, "windows": verify_windows}[
        platform_of(args.target)
    ](config, args, checks)
    if checks.failures:
        sys.exit("Bundle verification failed:\n- " + "\n- ".join(checks.failures))
    print(f"Verified {config['identifier']} {args.version} bundle for {args.target}")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ["bundle", "verify"]:
        command = commands.add_parser(name)
        command.add_argument("--channel", required=True, choices=["staging", "stable"])
        command.add_argument("--target", required=True)
        command.add_argument("--version", required=True)
        command.add_argument(
            "--no-sign",
            action="store_true",
            help="Built without code or updater signatures (tauri bundle --no-sign)",
        )
        command.add_argument(
            "--team-id", help="Require this Developer ID designated requirement"
        )
        command.add_argument(
            "--bundles",
            help="Tauri bundle types, e.g. app,dmg; verify checks only these "
            "(default: every type the platform builds)",
        )
    commands.choices["bundle"].add_argument("--binary", required=True, type=Path)
    commands.choices["bundle"].add_argument(
        "--out", type=Path, default=REPO / "apps/desktop-gpui/target/packaging.noindex"
    )
    commands.choices["verify"].add_argument("bundle_dir", type=Path)
    args = parser.parse_args()
    {"bundle": bundle, "verify": verify}[args.command](args)


if __name__ == "__main__":
    main()
