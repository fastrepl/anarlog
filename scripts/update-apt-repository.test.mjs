import assert from "node:assert/strict";
import {
  access,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { gunzipSync } from "node:zlib";

import {
  buildPackageIndex,
  buildRepository,
  writeRepository,
} from "./update-apt-repository.mjs";

function packageFixture(architecture, marker) {
  return {
    control: `Package: anarlog
Version: 1.4.9
Architecture: ${architecture}
Maintainer: Fastrepl <support@anarlog.so>
Description: Anarlog Desktop App
`,
    hashes: {
      md5: marker.repeat(32),
      sha1: marker.repeat(40),
      sha256: marker.repeat(64),
    },
    size: architecture === "amd64" ? 318551982 : 309903016,
  };
}

const PACKAGES = {
  amd64: packageFixture("amd64", "a"),
  arm64: packageFixture("arm64", "b"),
};

test("builds package indexes that redirect through immutable release paths", () => {
  const index = buildPackageIndex({
    ...PACKAGES.amd64,
    architecture: "amd64",
    version: "1.4.9",
  });

  assert.match(index, /^Package: anarlog$/m);
  assert.match(index, /^Version: 1\.4\.9$/m);
  assert.match(index, /^Architecture: amd64$/m);
  assert.match(
    index,
    /^Filename: pool\/desktop_v1\.4\.9\/amd64\/anarlog\.deb$/m,
  );
  assert.match(index, /^Size: 318551982$/m);
  assert.match(index, new RegExp(`^SHA256: ${"a".repeat(64)}$`, "m"));
});

test("rejects a package with the wrong identity", () => {
  assert.throws(
    () =>
      buildPackageIndex({
        ...PACKAGES.amd64,
        architecture: "arm64",
        version: "1.4.9",
      }),
    /Expected Architecture arm64, got amd64/,
  );
});

test("builds release checksums for both architectures", () => {
  const repository = buildRepository({
    date: "2026-08-13T14:52:57Z",
    packages: PACKAGES,
    version: "1.4.9",
  });

  assert.match(repository.release, /^Suite: stable$/m);
  assert.match(repository.release, /^Version: 1\.4\.9$/m);
  assert.match(repository.release, /^Date: Thu, 13 Aug 2026 14:52:57 GMT$/m);
  assert.match(repository.release, /^Architectures: amd64 arm64$/m);
  assert.match(repository.release, /main\/binary-amd64\/Packages\.gz/);
  assert.match(repository.release, /main\/binary-arm64\/Packages$/m);
});

test("writes readable package indexes and removes stale signatures", async (t) => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "anarlog-apt-"));
  t.after(() => rm(directory, { force: true, recursive: true }));

  const repository = buildRepository({
    date: "2026-08-13T14:52:57Z",
    packages: PACKAGES,
    version: "1.4.9",
  });
  await mkdir(path.join(directory, "dists", "stable"), { recursive: true });
  await writeFile(
    path.join(directory, "dists", "stable", "InRelease"),
    "stale",
  );
  await writeRepository(directory, repository);

  const distribution = path.join(directory, "dists", "stable");
  const plain = await readFile(
    path.join(distribution, "main", "binary-amd64", "Packages"),
    "utf8",
  );
  const compressed = gunzipSync(
    await readFile(
      path.join(distribution, "main", "binary-amd64", "Packages.gz"),
    ),
  ).toString("utf8");

  assert.equal(compressed, plain);
  assert.equal(
    await readFile(path.join(distribution, "Release"), "utf8"),
    repository.release,
  );
  await assert.rejects(access(path.join(distribution, "InRelease")));
});
