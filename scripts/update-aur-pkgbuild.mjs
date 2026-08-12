import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const REPO = "fastrepl/anarlog";
const SHA256_PATTERN = /^[0-9a-f]{64}$/;
const DEB_ASSETS = {
  x86_64: "anarlog-linux-x86_64.deb",
  aarch64: "anarlog-linux-aarch64.deb",
};

function replaceOnce(contents, pattern, replacement, label) {
  const matches = contents.match(new RegExp(pattern.source, "gm"));
  if (matches?.length !== 1) {
    throw new Error(
      `Expected exactly one ${label} line, found ${matches?.length ?? 0}`,
    );
  }

  return contents.replace(pattern, replacement);
}

function assertSha256(value, label) {
  if (!SHA256_PATTERN.test(value)) {
    throw new Error(`Invalid sha256 for ${label}: ${value}`);
  }
}

export function readPkgver(pkgbuild) {
  const match = pkgbuild.match(/^pkgver=(.+)$/m);
  if (!match) throw new Error("PKGBUILD has no pkgver");
  return match[1];
}

function readPkgrel(contents, pattern, label) {
  const match = contents.match(pattern);
  if (!match) throw new Error(`${label} has no pkgrel`);

  const pkgrel = Number.parseInt(match[1], 10);
  if (!Number.isSafeInteger(pkgrel) || pkgrel < 1) {
    throw new Error(`${label} has invalid pkgrel: ${match[1]}`);
  }
  return pkgrel;
}

function nextPkgbuildPkgrel(pkgbuild, version, checksums) {
  if (readPkgver(pkgbuild) !== version) return 1;

  const checksumsChanged = Object.entries(checksums).some(([key, checksum]) => {
    const field = key === "license" ? "sha256sums" : `sha256sums_${key}`;
    return !pkgbuild.includes(`${field}=('${checksum}')`);
  });
  const current = readPkgrel(pkgbuild, /^pkgrel=(.+)$/m, "PKGBUILD");
  return checksumsChanged ? current + 1 : current;
}

// PKGBUILD interpolates ${pkgver} into every source URL, so only the version
// and the checksums change between releases.
export function bumpPkgbuild(pkgbuild, { version, checksums }) {
  const pkgrel = nextPkgbuildPkgrel(pkgbuild, version, checksums);
  let next = replaceOnce(
    pkgbuild,
    /^pkgver=.+$/m,
    `pkgver=${version}`,
    "pkgver",
  );
  next = replaceOnce(next, /^pkgrel=.+$/m, `pkgrel=${pkgrel}`, "pkgrel");
  next = replaceOnce(
    next,
    /^sha256sums=\(.+\)$/m,
    `sha256sums=('${checksums.license}')`,
    "sha256sums",
  );
  next = replaceOnce(
    next,
    /^sha256sums_x86_64=\(.+\)$/m,
    `sha256sums_x86_64=('${checksums.x86_64}')`,
    "sha256sums_x86_64",
  );

  return replaceOnce(
    next,
    /^sha256sums_aarch64=\(.+\)$/m,
    `sha256sums_aarch64=('${checksums.aarch64}')`,
    "sha256sums_aarch64",
  );
}

// .SRCINFO stores the expanded values, so its source URLs carry the literal
// version and have to be rewritten alongside the checksums.
export function bumpSrcinfo(srcinfo, { version, previousVersion, checksums }) {
  const checksumsChanged = Object.entries(checksums).some(([key, checksum]) => {
    const field = key === "license" ? "sha256sums" : `sha256sums_${key}`;
    return !srcinfo.includes(`\t${field} = ${checksum}`);
  });
  const currentPkgrel = readPkgrel(srcinfo, /^\tpkgrel = (.+)$/m, ".SRCINFO");
  const pkgrel =
    previousVersion === version
      ? checksumsChanged
        ? currentPkgrel + 1
        : currentPkgrel
      : 1;
  const rewritten = srcinfo
    .split("\n")
    .map((line) => {
      if (/^\tpkgver = /.test(line)) return `\tpkgver = ${version}`;
      if (/^\tpkgrel = /.test(line)) return `\tpkgrel = ${pkgrel}`;
      if (/^\tsha256sums = /.test(line)) {
        return `\tsha256sums = ${checksums.license}`;
      }
      if (/^\tsha256sums_x86_64 = /.test(line)) {
        return `\tsha256sums_x86_64 = ${checksums.x86_64}`;
      }
      if (/^\tsha256sums_aarch64 = /.test(line)) {
        return `\tsha256sums_aarch64 = ${checksums.aarch64}`;
      }
      if (/^\tsource(_x86_64|_aarch64)? = /.test(line)) {
        return line.replaceAll(previousVersion, version);
      }
      return line;
    })
    .join("\n");

  if (previousVersion !== version && rewritten.includes(previousVersion)) {
    throw new Error(
      `.SRCINFO still references ${previousVersion} after the bump`,
    );
  }

  return rewritten;
}

async function fetchText(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`GET ${url} failed with ${response.status}`);
  }
  return response.text();
}

async function fetchReleaseChecksum(version, asset) {
  const text = await fetchText(
    `https://github.com/${REPO}/releases/download/desktop_v${version}/${asset}.sha256`,
  );
  return text.trim().split(/\s+/)[0];
}

async function fetchLicenseChecksum(version) {
  const license = await fetchText(
    `https://raw.githubusercontent.com/${REPO}/desktop_v${version}/LICENSE`,
  );
  return createHash("sha256").update(license).digest("hex");
}

export async function resolveChecksums(version, overrides = {}) {
  const checksums = {
    license: overrides.license ?? (await fetchLicenseChecksum(version)),
    x86_64:
      overrides.x86_64 ??
      (await fetchReleaseChecksum(version, DEB_ASSETS.x86_64)),
    aarch64:
      overrides.aarch64 ??
      (await fetchReleaseChecksum(version, DEB_ASSETS.aarch64)),
  };

  for (const [label, value] of Object.entries(checksums)) {
    assertSha256(value, label);
  }

  return checksums;
}

function defaultPackageDirectory() {
  const here = path.dirname(fileURLToPath(import.meta.url));
  return path.join(here, "..", "packaging", "aur", "anarlog-bin");
}

export async function updatePackage(directory, version, checksums) {
  const pkgbuildPath = path.join(directory, "PKGBUILD");
  const srcinfoPath = path.join(directory, ".SRCINFO");

  const pkgbuild = await readFile(pkgbuildPath, "utf8");
  const srcinfo = await readFile(srcinfoPath, "utf8");
  const previousVersion = readPkgver(pkgbuild);

  await writeFile(pkgbuildPath, bumpPkgbuild(pkgbuild, { version, checksums }));
  await writeFile(
    srcinfoPath,
    bumpSrcinfo(srcinfo, { version, previousVersion, checksums }),
  );

  return { previousVersion, changed: previousVersion !== version };
}

async function main() {
  const { values } = parseArgs({
    options: {
      version: { type: "string" },
      dir: { type: "string" },
      "sha-license": { type: "string" },
      "sha-x86_64": { type: "string" },
      "sha-aarch64": { type: "string" },
    },
  });

  if (!values.version) {
    throw new Error("--version is required, for example --version 1.4.9");
  }

  const version = values.version.replace(/^desktop_v/, "");
  const checksums = await resolveChecksums(version, {
    license: values["sha-license"],
    x86_64: values["sha-x86_64"],
    aarch64: values["sha-aarch64"],
  });

  const directory = values.dir ?? defaultPackageDirectory();
  const { previousVersion, changed } = await updatePackage(
    directory,
    version,
    checksums,
  );

  console.log(
    changed
      ? `anarlog-bin ${previousVersion} -> ${version}`
      : `anarlog-bin already at ${version}, refreshed checksums`,
  );
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  await main();
}
