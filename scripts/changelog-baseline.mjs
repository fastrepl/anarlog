import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { pathToFileURL } from "node:url";

const versionPattern = /^\d+\.\d+\.\d+$/;
const compare = (left, right) => {
  const a = left.split(".").map(Number);
  const b = right.split(".").map(Number);
  return a[0] - b[0] || a[1] - b[1] || a[2] - b[2];
};

export function changelogBaseline(
  stream,
  channel,
  version,
  cwd = process.cwd(),
  source = "",
) {
  if (
    !versionPattern.test(version) ||
    !["desktop", "mobile"].includes(stream) ||
    !["stable", "beta"].includes(channel) ||
    (stream === "desktop" && channel !== "stable")
  ) {
    throw new Error("Unsupported changelog stream, channel or version");
  }
  if (source && !/^[a-f0-9]{40}$/.test(source))
    throw new Error("Changelog source must be a full commit SHA");
  const git = (...args) =>
    execFileSync("git", args, {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }).trim();
  let current = "HEAD";
  let releases;
  if (stream === "desktop") {
    releases = git("tag", "-l", "desktop_v*")
      .split("\n")
      .flatMap((tag) => {
        const releasedVersion = tag.replace(/^desktop_v/, "");
        return versionPattern.test(releasedVersion)
          ? [{ version: releasedVersion, source: tag }]
          : [];
      });
  } else {
    const directory = `${cwd}/packages/changelog/content/mobile`;
    releases = readdirSync(directory)
      .filter((file) => /^\d+\.\d+\.\d+\.json$/.test(file))
      .map((file) => {
        const record = JSON.parse(readFileSync(`${directory}/${file}`, "utf8"));
        if (
          record.version !== file.replace(/\.json$/, "") ||
          !/^[a-f0-9]{40}$/.test(record.sourceSha) ||
          !["stable", "beta"].includes(record.channel) ||
          !Array.isArray(record.availability) ||
          record.availability.length === 0 ||
          record.availability.some(
            (item) =>
              !Number.isFinite(Date.parse(item.publishedAt)) ||
              Date.parse(item.publishedAt) > Date.now(),
          )
        ) {
          throw new Error(`Invalid mobile publication record: ${file}`);
        }
        return {
          version: record.version,
          source: record.sourceSha,
          channel: record.channel,
        };
      });
  }
  const requested = releases.find(
    (release) =>
      release.version === version &&
      (stream === "desktop" || release.channel === channel),
  );
  if (source) current = git("rev-parse", "--verify", `${source}^{commit}`);
  else if (requested) current = requested.source;
  const previous = releases
    .filter(
      (release) =>
        compare(release.version, version) < 0 &&
        (stream === "desktop" ||
          channel === "beta" ||
          release.channel === "stable"),
    )
    .sort((a, b) => compare(b.version, a.version))[0];
  const roots = previous
    ? []
    : git("rev-list", "--max-parents=0", current).split("\n");
  if (!previous && roots.length !== 1)
    throw new Error("Ambiguous initial release ancestry");
  const prev = previous?.source ?? roots[0];
  git("merge-base", "--is-ancestor", prev, current);
  return { prev, current, initial: !previous };
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  console.log(
    JSON.stringify(
      changelogBaseline(
        ...process.argv.slice(2, 5),
        process.cwd(),
        process.argv[5],
      ),
    ),
  );
}
