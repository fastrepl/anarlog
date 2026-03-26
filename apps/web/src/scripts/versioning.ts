import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

// https://docs.crabnebula.dev/cloud/cli/upload-assets/#public-platform---public-platform
export type VersionPlatform =
  | "dmg-aarch64"
  | "appimage-x86_64"
  | "deb-x86_64"
  | "appimage-aarch64"
  | "deb-aarch64";

const GITHUB_REPO_OWNER = "fastrepl";
const GITHUB_REPO_NAME = "char";

type GithubTagResponse = {
  name: string;
  commit: {
    sha: string;
    url: string;
  };
};

type GithubCommitResponse = {
  sha: string;
  commit: {
    author: {
      date: string;
    };
  };
};

type GithubTagInfo = {
  tag: string;
  version: string;
  sha: string;
  createdAt: string;
};

async function fetchGithubDesktopTags(options?: {
  signal?: AbortSignal;
  token?: string;
}): Promise<GithubTagInfo[]> {
  const headers: HeadersInit = {
    Accept: "application/vnd.github+json",
  };

  if (options?.token) {
    headers.Authorization = `Bearer ${options.token}`;
  }

  const response = await fetch(
    `https://api.github.com/repos/${GITHUB_REPO_OWNER}/${GITHUB_REPO_NAME}/tags?per_page=100`,
    {
      headers,
      signal: options?.signal,
    },
  );

  if (!response.ok) {
    throw new Error(
      `Failed to fetch GitHub tags: ${response.status} ${response.statusText}`,
    );
  }

  const data = (await response.json()) as GithubTagResponse[];

  const filteredTags = data.filter(
    (tag) =>
      tag.name.startsWith("desktop_v1") &&
      !tag.name.includes("1.0.0-nightly.0"),
  );

  const tagsWithDates = await Promise.all(
    filteredTags.map(async (tag) => {
      const commitResponse = await fetch(
        `https://api.github.com/repos/${GITHUB_REPO_OWNER}/${GITHUB_REPO_NAME}/commits/${tag.commit.sha}`,
        {
          headers,
          signal: options?.signal,
        },
      );

      if (!commitResponse.ok) {
        throw new Error(
          `Failed to fetch commit ${tag.commit.sha}: ${commitResponse.status} ${commitResponse.statusText}`,
        );
      }

      const commitData = (await commitResponse.json()) as GithubCommitResponse;

      return {
        tag: tag.name,
        version: tag.name.replace(/^desktop_v/, ""),
        sha: tag.commit.sha,
        createdAt: commitData.commit.author.date,
      };
    }),
  );

  return tagsWithDates;
}

function updateCreatedField(content: string, date: string): string {
  const frontmatterMatch = content.match(/^---\n([\s\S]*?)\n---\n([\s\S]*)$/);

  if (!frontmatterMatch) {
    return `---\ncreated: "${date}"\n---\n${content}`;
  }

  const [, frontmatter, body] = frontmatterMatch;
  const hasCreated = /^created:/m.test(frontmatter);

  const updatedFrontmatter = hasCreated
    ? frontmatter.replace(/^created:.*$/m, `created: "${date}"`)
    : `${frontmatter}\ncreated: "${date}"`;

  return `---\n${updatedFrontmatter}\n---\n${body.trimEnd()}\n`;
}

async function updateChangelogFiles(tags: GithubTagInfo[]): Promise<void> {
  const changelogDir = join(import.meta.dirname, "../../content/changelog");
  await mkdir(changelogDir, { recursive: true });

  await Promise.all(
    tags.map(async (tag) => {
      const filePath = join(changelogDir, `${tag.version}.mdx`);
      const datetime = tag.createdAt;

      const content = await readFile(filePath, "utf-8").catch(() => "");
      const updated = updateCreatedField(content, datetime);

      await writeFile(filePath, updated, "utf-8");
      console.log(`Updated ${tag.version}.mdx with created date: ${datetime}`);
    }),
  );
}

fetchGithubDesktopTags().then(updateChangelogFiles);
