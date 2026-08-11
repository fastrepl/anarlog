import * as path from "path";

import { getSupabaseServerClient } from "@/functions/supabase";

export const GITHUB_REPO = "fastrepl/anarlog";
export const GITHUB_BRANCH = "main";
export const CONTENT_PATH = "apps/web/content";

export function isDev(): boolean {
  return process.env.NODE_ENV === "development";
}

export function getLocalContentPath(): string {
  return path.resolve(process.cwd(), "content");
}

export const VALID_FOLDERS = [
  "articles",
  "changelog",
  "docs",
  "handbook",
  "legal",
  "templates",
];

export const REVIEWABLE_CONTENT_FOLDERS = [
  "articles",
  "docs",
  "handbook",
] as const;

export type ReviewableContentFolder =
  (typeof REVIEWABLE_CONTENT_FOLDERS)[number];

const GITHUB_USERNAME_TO_AUTHOR: Record<
  string,
  { name: string; email: string }
> = {
  yujonglee: { name: "Yujong Lee", email: "yujonglee@hyprnote.com" },
  ComputelessComputer: { name: "John Jeong", email: "john@hyprnote.com" },
};

export interface GitHubCredentials {
  token: string;
  author?: { name: string; email: string };
}

export interface CommitBody {
  message: string;
  content?: string;
  sha?: string;
  branch?: string;
  author?: { name: string; email: string };
  committer?: { name: string; email: string };
}

export function buildCommitBody(
  message: string,
  author?: { name: string; email: string },
  options?: { content?: string; sha?: string; branch?: string },
): CommitBody {
  const body: CommitBody = {
    message,
  };
  if (options?.content !== undefined) body.content = options.content;
  if (options?.sha) body.sha = options.sha;
  if (options?.branch) body.branch = options.branch;
  if (author) {
    body.author = author;
    body.committer = author;
  }
  return body;
}

export async function getGitHubCredentials(): Promise<
  GitHubCredentials | undefined
> {
  const supabase = getSupabaseServerClient();
  const { data: userData } = await supabase.auth.getUser();

  if (!userData.user?.id) {
    return undefined;
  }

  const { data: admin } = await supabase
    .from("admins")
    .select("github_token, github_username")
    .eq("id", userData.user.id)
    .single();

  if (!admin?.github_token) {
    return undefined;
  }

  const author = admin.github_username
    ? GITHUB_USERNAME_TO_AUTHOR[admin.github_username]
    : undefined;

  return { token: admin.github_token, author };
}

export function sanitizeFilename(filename: string): string {
  return filename
    .replace(/[^a-zA-Z0-9-_.]/g, "-")
    .replace(/-+/g, "-")
    .toLowerCase();
}

function sanitizePathSegment(segment: string): string {
  return segment
    .replace(/[^a-zA-Z0-9-_.]/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "")
    .toLowerCase();
}

export function sanitizeRelativeFilePath(relativePath: string): string {
  return relativePath
    .split("/")
    .filter(Boolean)
    .map((segment) => {
      if (segment === ".gitkeep") {
        return segment;
      }

      const hasExtension = segment.endsWith(".mdx");
      const baseName = hasExtension ? segment.slice(0, -4) : segment;
      const sanitized = sanitizePathSegment(baseName) || "untitled";
      return hasExtension ? `${sanitized}.mdx` : sanitized;
    })
    .join("/");
}

export function getCollectionFromPath(
  filePath: string,
): ReviewableContentFolder | undefined {
  const normalizedPath = filePath.replace(/^apps\/web\/content\//, "");
  const folder = normalizedPath.split("/")[0];

  if ((REVIEWABLE_CONTENT_FOLDERS as readonly string[]).includes(folder)) {
    return folder as ReviewableContentFolder;
  }

  return undefined;
}

function getBranchToken(filePath: string): string {
  const normalizedPath = filePath
    .replace(/^apps\/web\/content\//, "")
    .replace(/\.mdx$/, "");

  const [, ...rest] = normalizedPath.split("/");
  const token = rest.join("-").replace(/[^a-zA-Z0-9-]/g, "-");

  return token.replace(/-+/g, "-").replace(/^-|-$/g, "").toLowerCase();
}

function getBranchPrefix(collection: ReviewableContentFolder): string {
  switch (collection) {
    case "articles":
      return "blog";
    case "docs":
      return "content/docs";
    case "handbook":
      return "content/handbook";
  }
}

export function buildDraftBranchName(filePath: string): string {
  const collection = getCollectionFromPath(filePath);
  if (!collection) {
    return generateBranchName(filePath);
  }

  const token = getBranchToken(filePath);
  const prefix = getBranchPrefix(collection);
  if (collection === "articles") {
    return `${prefix}/${token}`;
  }

  return `${prefix}/${token}-${Date.now()}`;
}

export function buildPublishedEditBranchName(filePath: string): string {
  const collection = getCollectionFromPath(filePath);
  if (!collection) {
    return generateBranchName(filePath);
  }

  const token = getBranchToken(filePath);
  const prefix = getBranchPrefix(collection);

  if (collection === "articles") {
    return `${prefix}/${token}-${Date.now()}`;
  }

  return `${prefix}/${token}-${Date.now()}`;
}

export function getExistingBranchPrefix(filePath: string): string {
  const collection = getCollectionFromPath(filePath);
  if (!collection) {
    return `blog/${getBranchToken(filePath)}-`;
  }

  return `${getBranchPrefix(collection)}/${getBranchToken(filePath)}-`;
}

export function getFullPath(folder: string, filename: string): string {
  return `${CONTENT_PATH}/${folder}/${filename}`;
}

export function getDefaultFrontmatter(folder: string): string {
  const today = new Date().toISOString().split("T")[0];

  switch (folder) {
    case "articles":
      return `---
meta_title: ""
display_title: ""
meta_description: ""
author:
- "John Jeong"
featured: false
category: "Product"
date: "${today}"
---

`;
    case "changelog":
      return `---
date: "${today}"
---

`;
    case "docs":
      return `---
title: ""
section: ""
description: ""
---

`;
    case "handbook":
      return `---
title: ""
section: ""
summary: ""
---

`;
    case "legal":
      return `---
title: ""
summary: ""
date: "${today}"
---

`;
    case "templates":
      return `---
title: ""
description: ""
category: ""
targets: []
sections: []
---

`;
    default:
      return `---
title: ""
---

`;
  }
}

export function generateBranchName(slugOrPath: string): string {
  const sanitizedSlug = slugOrPath
    .replace(/^apps\/web\/content\//, "")
    .replace(/^articles\//, "")
    .replace(/\.mdx$/, "")
    .replace(/[^a-zA-Z0-9-]/g, "-")
    .toLowerCase();
  return `blog/${sanitizedSlug}`;
}
