import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  bumpPkgbuild,
  bumpSrcinfo,
  readPkgver,
  updatePackage,
} from "./update-aur-pkgbuild.mjs";

const LICENSE_SHA = "a".repeat(64);
const X86_SHA = "b".repeat(64);
const ARM_SHA = "c".repeat(64);
const CHECKSUMS = {
  license: LICENSE_SHA,
  x86_64: X86_SHA,
  aarch64: ARM_SHA,
};

const PKGBUILD = `pkgname=anarlog-bin
pkgver=1.4.8
pkgrel=2
arch=('x86_64' 'aarch64')
_release="desktop_v\${pkgver}"
source=("LICENSE-\${pkgver}::https://raw.githubusercontent.com/fastrepl/anarlog/\${_release}/LICENSE")
source_x86_64=("anarlog-\${pkgver}-x86_64.deb::https://github.com/fastrepl/anarlog/releases/download/\${_release}/anarlog-linux-x86_64.deb")
source_aarch64=("anarlog-\${pkgver}-aarch64.deb::https://github.com/fastrepl/anarlog/releases/download/\${_release}/anarlog-linux-aarch64.deb")
sha256sums=('0000000000000000000000000000000000000000000000000000000000000001')
sha256sums_x86_64=('0000000000000000000000000000000000000000000000000000000000000002')
sha256sums_aarch64=('0000000000000000000000000000000000000000000000000000000000000003')
`;

const SRCINFO = `pkgbase = anarlog-bin
\tpkgver = 1.4.8
\tpkgrel = 2
\tarch = x86_64
\tsource = LICENSE-1.4.8::https://raw.githubusercontent.com/fastrepl/anarlog/desktop_v1.4.8/LICENSE
\tsha256sums = 0000000000000000000000000000000000000000000000000000000000000001
\tsource_x86_64 = anarlog-1.4.8-x86_64.deb::https://github.com/fastrepl/anarlog/releases/download/desktop_v1.4.8/anarlog-linux-x86_64.deb
\tsha256sums_x86_64 = 0000000000000000000000000000000000000000000000000000000000000002
\tsource_aarch64 = anarlog-1.4.8-aarch64.deb::https://github.com/fastrepl/anarlog/releases/download/desktop_v1.4.8/anarlog-linux-aarch64.deb
\tsha256sums_aarch64 = 0000000000000000000000000000000000000000000000000000000000000003

pkgname = anarlog-bin
`;

test("bumps the PKGBUILD version, checksums, and resets pkgrel", () => {
  const next = bumpPkgbuild(PKGBUILD, {
    version: "1.5.0",
    checksums: CHECKSUMS,
  });

  assert.match(next, /^pkgver=1\.5\.0$/m);
  assert.match(next, /^pkgrel=1$/m);
  assert.match(next, new RegExp(`^sha256sums=\\('${LICENSE_SHA}'\\)$`, "m"));
  assert.match(next, new RegExp(`^sha256sums_x86_64=\\('${X86_SHA}'\\)$`, "m"));
  assert.match(
    next,
    new RegExp(`^sha256sums_aarch64=\\('${ARM_SHA}'\\)$`, "m"),
  );
});

test("leaves interpolated PKGBUILD source URLs untouched", () => {
  const next = bumpPkgbuild(PKGBUILD, {
    version: "1.5.0",
    checksums: CHECKSUMS,
  });

  assert.match(next, /_release="desktop_v\$\{pkgver\}"/);
  assert.ok(!next.includes("1.4.8"));
});

test("rewrites expanded .SRCINFO source URLs and checksums", () => {
  const next = bumpSrcinfo(SRCINFO, {
    version: "1.5.0",
    previousVersion: "1.4.8",
    checksums: CHECKSUMS,
  });

  assert.match(next, /^\tpkgver = 1\.5\.0$/m);
  assert.match(next, /^\tpkgrel = 1$/m);
  assert.ok(next.includes("desktop_v1.5.0/anarlog-linux-x86_64.deb"));
  assert.ok(next.includes("anarlog-1.5.0-aarch64.deb"));
  assert.ok(next.includes("LICENSE-1.5.0::"));
  assert.ok(!next.includes("1.4.8"));
});

test("fails when a stale version survives the .SRCINFO rewrite", () => {
  const stale = SRCINFO.replace(
    "pkgbase = anarlog-bin",
    "pkgbase = anarlog-bin\n\tpkgdesc = build 1.4.8",
  );

  assert.throws(
    () =>
      bumpSrcinfo(stale, {
        version: "1.5.0",
        previousVersion: "1.4.8",
        checksums: CHECKSUMS,
      }),
    /still references 1\.4\.8/,
  );
});

test("fails when the PKGBUILD is missing an expected field", () => {
  const malformed = PKGBUILD.replace(/^sha256sums_aarch64=.+$/m, "");

  assert.throws(
    () => bumpPkgbuild(malformed, { version: "1.5.0", checksums: CHECKSUMS }),
    /Expected exactly one sha256sums_aarch64 line/,
  );
});

test("reads the current pkgver", () => {
  assert.equal(readPkgver(PKGBUILD), "1.4.8");
  assert.throws(() => readPkgver("pkgname=anarlog-bin\n"), /no pkgver/);
});

test("writes both files and reports the previous version", async (t) => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "anarlog-aur-"));
  t.after(() => rm(directory, { force: true, recursive: true }));

  await writeFile(path.join(directory, "PKGBUILD"), PKGBUILD);
  await writeFile(path.join(directory, ".SRCINFO"), SRCINFO);

  const result = await updatePackage(directory, "1.5.0", CHECKSUMS);
  assert.deepEqual(result, { previousVersion: "1.4.8", changed: true });

  const pkgbuild = await readFile(path.join(directory, "PKGBUILD"), "utf8");
  const srcinfo = await readFile(path.join(directory, ".SRCINFO"), "utf8");
  assert.match(pkgbuild, /^pkgver=1\.5\.0$/m);
  assert.match(srcinfo, /^\tpkgver = 1\.5\.0$/m);
});

test("reports no change when the version already matches", async (t) => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "anarlog-aur-"));
  t.after(() => rm(directory, { force: true, recursive: true }));

  await writeFile(path.join(directory, "PKGBUILD"), PKGBUILD);
  await writeFile(path.join(directory, ".SRCINFO"), SRCINFO);

  const result = await updatePackage(directory, "1.4.8", CHECKSUMS);
  assert.equal(result.changed, false);

  const pkgbuild = await readFile(path.join(directory, "PKGBUILD"), "utf8");
  const srcinfo = await readFile(path.join(directory, ".SRCINFO"), "utf8");
  assert.match(pkgbuild, /^pkgrel=3$/m);
  assert.match(srcinfo, /^\tpkgrel = 3$/m);
});

test("keeps pkgrel when a same-version refresh changes nothing", () => {
  const pkgbuild = bumpPkgbuild(PKGBUILD, {
    version: "1.4.8",
    checksums: CHECKSUMS,
  });
  const srcinfo = bumpSrcinfo(SRCINFO, {
    version: "1.4.8",
    previousVersion: "1.4.8",
    checksums: CHECKSUMS,
  });

  assert.match(
    bumpPkgbuild(pkgbuild, { version: "1.4.8", checksums: CHECKSUMS }),
    /^pkgrel=3$/m,
  );
  assert.match(
    bumpSrcinfo(srcinfo, {
      version: "1.4.8",
      previousVersion: "1.4.8",
      checksums: CHECKSUMS,
    }),
    /^\tpkgrel = 3$/m,
  );
});
