import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { mkdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs, promisify } from "node:util";
import { gzipSync } from "node:zlib";

const execFileAsync = promisify(execFile);
const VERSION_PATTERN = /^\d+\.\d+\.\d+$/;
const PACKAGE_NAME = "anarlog";

const ARCHITECTURES = ["amd64", "arm64"];

function readControlField(control, field) {
  const match = control.match(new RegExp(`^${field}: (.+)$`, "m"));
  if (!match) throw new Error(`Debian control metadata has no ${field} field`);
  return match[1];
}

function validatePackage(control, version, architecture) {
  const expected = {
    Package: PACKAGE_NAME,
    Version: version,
    Architecture: architecture,
  };

  for (const [field, value] of Object.entries(expected)) {
    const actual = readControlField(control, field);
    if (actual !== value) {
      throw new Error(`Expected ${field} ${value}, got ${actual}`);
    }
  }
}

function hashBuffer(contents, algorithm) {
  return createHash(algorithm).update(contents).digest("hex");
}

function checksumRows(files, algorithm) {
  return [...files.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([filePath, contents]) => {
      const buffer = Buffer.isBuffer(contents)
        ? contents
        : Buffer.from(contents, "utf8");
      return ` ${hashBuffer(buffer, algorithm)} ${buffer.byteLength} ${filePath}`;
    })
    .join("\n");
}

export function buildPackageIndex({
  architecture,
  control,
  hashes,
  size,
  version,
}) {
  validatePackage(control, version, architecture);
  const release = `desktop_v${version}`;

  return `${control.trimEnd()}
Filename: pool/${release}/${architecture}/anarlog.deb
Size: ${size}
MD5sum: ${hashes.md5}
SHA1: ${hashes.sha1}
SHA256: ${hashes.sha256}
`;
}

export function buildRepository({ date, packages, version }) {
  if (!VERSION_PATTERN.test(version)) {
    throw new Error(`Invalid stable version: ${version}`);
  }

  const publishedAt = new Date(date);
  if (Number.isNaN(publishedAt.valueOf())) {
    throw new Error(`Invalid repository date: ${date}`);
  }

  const files = new Map();
  for (const architecture of ARCHITECTURES) {
    const packageIndex = buildPackageIndex({
      ...packages[architecture],
      architecture,
      version,
    });
    const directory = `main/binary-${architecture}`;
    files.set(`${directory}/Packages`, packageIndex);
    files.set(
      `${directory}/Packages.gz`,
      gzipSync(packageIndex, { level: 9, mtime: 0 }),
    );
  }

  const release = `Origin: Anarlog
Label: Anarlog
Suite: stable
Codename: stable
Version: ${version}
Date: ${publishedAt.toUTCString()}
Architectures: amd64 arm64
Components: main
Description: Anarlog stable desktop releases
MD5Sum:
${checksumRows(files, "md5")}
SHA1:
${checksumRows(files, "sha1")}
SHA256:
${checksumRows(files, "sha256")}
`;

  return { files, release };
}

async function hashFile(filePath) {
  const hashes = {
    md5: createHash("md5"),
    sha1: createHash("sha1"),
    sha256: createHash("sha256"),
  };

  for await (const chunk of createReadStream(filePath)) {
    for (const hash of Object.values(hashes)) hash.update(chunk);
  }

  return Object.fromEntries(
    Object.entries(hashes).map(([algorithm, hash]) => [
      algorithm,
      hash.digest("hex"),
    ]),
  );
}

async function inspectPackage(filePath) {
  const [{ stdout: control }, file, hashes] = await Promise.all([
    execFileAsync("dpkg-deb", ["--field", filePath], {
      maxBuffer: 1024 * 1024,
    }),
    stat(filePath),
    hashFile(filePath),
  ]);

  return { control, hashes, size: file.size };
}

export async function writeRepository(outputDirectory, repository) {
  const distribution = path.join(outputDirectory, "dists", "stable");
  await rm(distribution, { force: true, recursive: true });

  for (const [relativePath, contents] of repository.files) {
    const outputPath = path.join(distribution, relativePath);
    await mkdir(path.dirname(outputPath), { recursive: true });
    await writeFile(outputPath, contents);
  }

  await writeFile(path.join(distribution, "Release"), repository.release);
}

function defaultOutputDirectory() {
  const here = path.dirname(fileURLToPath(import.meta.url));
  return path.join(here, "..", "apps", "web", "public", "apt");
}

async function main() {
  const { values } = parseArgs({
    options: {
      amd64: { type: "string" },
      arm64: { type: "string" },
      date: { type: "string" },
      out: { type: "string" },
      version: { type: "string" },
    },
  });

  if (!values.version || !VERSION_PATTERN.test(values.version)) {
    throw new Error("--version must be a stable version such as 1.4.9");
  }
  if (!values.amd64 || !values.arm64) {
    throw new Error("--amd64 and --arm64 Debian packages are required");
  }

  const packages = {
    amd64: await inspectPackage(values.amd64),
    arm64: await inspectPackage(values.arm64),
  };
  const repository = buildRepository({
    date: values.date ?? new Date().toISOString(),
    packages,
    version: values.version,
  });
  const outputDirectory = values.out ?? defaultOutputDirectory();
  await writeRepository(outputDirectory, repository);

  const releasePath = path.join(outputDirectory, "dists", "stable", "Release");
  const written = await readFile(releasePath, "utf8");
  if (written !== repository.release) {
    throw new Error(`Failed to verify generated ${releasePath}`);
  }

  console.log(`APT repository metadata now tracks ${values.version}`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  await main();
}
