import braces from "braces";
import decode from "decode-uri-component";
import extract from "extract-zip";
import forge from "node-forge";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { generateKeyPairSync } from "node:crypto";
import { mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import sprintf from "sprintf-js";

test("braces rejects deeply nested patterns before recursive walkers exhaust the stack", () => {
  const input = "{".repeat(4_000) + "a,b" + "}".repeat(4_000);
  for (const operation of [braces.compile, braces.expand]) {
    assert.throws(() => operation(input), {
      name: "SyntaxError",
      message: /nesting depth/,
    });
  }
  assert.deepEqual(braces.expand("{one,two}"), ["one", "two"]);
});

test("malformed URI decoding stays bounded and preserves valid UTF-8", () => {
  assert.equal(decode("%E2%82%AC%FF"), "€%FF");
  assert.equal(decode("a+b"), "a b");
  const result = spawnSync(
    process.execPath,
    [
      "-e",
      "const d=require('decode-uri-component'); const s='%FF'.repeat(5000); if(d(s)!==s) process.exit(1);",
    ],
    { timeout: 3_000, encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
});

test("sprintf clamps untrusted floating-point precision to supported limits", () => {
  for (const specifier of ["f", "e", "g"]) {
    assert.doesNotThrow(() => sprintf.sprintf(`%.50000${specifier}`, 1.25));
  }
  assert.equal(sprintf.sprintf("%.2f", 1.25), "1.25");
});

test("RSA verification rejects additional nested DigestAlgorithm elements", () => {
  const pair = generateKeyPairSync("rsa", {
    modulusLength: 1024,
    publicExponent: 3,
    privateKeyEncoding: { type: "pkcs1", format: "pem" },
    publicKeyEncoding: { type: "pkcs1", format: "pem" },
  });
  const privateKey = forge.pki.privateKeyFromPem(pair.privateKey);
  const publicKey = forge.pki.publicKeyFromPem(pair.publicKey);
  const md = forge.md.sha256.create().update("message");
  const digest = md.digest().getBytes();
  assert.equal(publicKey.verify(digest, privateKey.sign(md)), true);
  const asn1 = forge.asn1;
  const node = (type, constructed, value) =>
    asn1.create(asn1.Class.UNIVERSAL, type, constructed, value);
  const malformed = node(asn1.Type.SEQUENCE, true, [
    node(asn1.Type.SEQUENCE, true, [
      node(asn1.Type.OID, false, asn1.oidToDer(forge.oids.sha256).getBytes()),
      node(asn1.Type.NULL, false, ""),
      node(asn1.Type.OCTETSTRING, false, "garbage"),
    ]),
    node(asn1.Type.OCTETSTRING, false, digest),
  ]);
  const signature = privateKey.sign(asn1.toDer(malformed).getBytes(), "NONE");
  assert.throws(() => publicKey.verify(digest, signature), /DigestInfo/);
});

function createZip(file, entries) {
  const result = spawnSync(
    "python3",
    [
      "-c",
      `import json, sys, zipfile
with zipfile.ZipFile(sys.argv[1], 'w') as archive:
 for name, content, link in json.loads(sys.argv[2]):
  entry = zipfile.ZipInfo(name)
  entry.create_system = 3
  entry.external_attr = (0o120777 if link else 0o100644) << 16
  archive.writestr(entry, content)
`,
      file,
      JSON.stringify(entries),
    ],
    { encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
}

test("zip extraction confines symlink targets and never follows existing output links", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "zip-security-"));
  const output = path.join(directory, "output");
  const outside = path.join(directory, "outside");
  const archive = path.join(directory, "archive.zip");
  try {
    createZip(archive, [
      ["file", "content", false],
      ["link", "file", true],
    ]);
    await extract(archive, { dir: output });
    assert.equal(await readFile(path.join(output, "link"), "utf8"), "content");
    createZip(archive, [["escape", "../outside", true]]);
    await assert.rejects(extract(archive, { dir: output }), /Out of bound/);
    await writeFile(outside, "untouched");
    await symlink(outside, path.join(output, "existing"));
    createZip(archive, [["existing", "overwrite", false]]);
    await assert.rejects(extract(archive, { dir: output }));
    assert.equal(await readFile(outside, "utf8"), "untouched");
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
