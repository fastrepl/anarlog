import type { TaskArgsMapTransformed } from ".";

type EnhanceArgs = TaskArgsMapTransformed["enhance"];

const TAG_NAME_RE = /^[\p{L}_][\p{L}\p{N}_-]*$/u;
const HASHTAG_RE = /(^|[^\p{L}\p{N}_/#])#([\p{L}_][\p{L}\p{N}_-]*)/gu;

export function extractEnhanceTagNames(
  summaryMarkdown: string,
  transformedArgs: EnhanceArgs,
): string[] {
  const sources = [
    summaryMarkdown,
    transformedArgs.preMeetingMemo,
    transformedArgs.postMeetingMemo,
    transformedArgs.template?.title,
    transformedArgs.template?.description,
    ...(transformedArgs.template?.sections ?? []).flatMap((section) => [
      section.title,
      section.description,
    ]),
  ];

  return extractHashtagNames(sources);
}

export function appendTagLineToMarkdown(
  markdown: string,
  tagNames: string[],
): string {
  const normalizedTagNames = normalizeTagNames(tagNames);
  if (normalizedTagNames.length === 0) {
    return markdown;
  }

  const body = stripTrailingTagLines(markdown).trimEnd();
  const tagLine = normalizedTagNames.map((tagName) => `#${tagName}`).join(" ");

  return body ? `${body}\n\n${tagLine}` : tagLine;
}

function extractHashtagNames(sources: Array<string | null | undefined>) {
  const tagNames: string[] = [];

  for (const source of sources) {
    if (!source) {
      continue;
    }

    for (const match of source.matchAll(HASHTAG_RE)) {
      const tagName = match[2];
      if (tagName) {
        tagNames.push(tagName);
      }
    }
  }

  return normalizeTagNames(tagNames);
}

function normalizeTagNames(tagNames: string[]) {
  const result = new Map<string, string>();

  for (const rawTagName of tagNames) {
    const tagName = rawTagName.replace(/^#/, "").trim().toLowerCase();
    if (!TAG_NAME_RE.test(tagName)) {
      continue;
    }

    result.set(tagName, tagName);
  }

  return [...result.values()];
}

function stripTrailingTagLines(markdown: string) {
  const lines = markdown.split(/\r?\n/);
  let end = lines.length;

  while (end > 0 && lines[end - 1]?.trim() === "") {
    end -= 1;
  }

  while (end > 0 && isTagOnlyLine(lines[end - 1] ?? "")) {
    end -= 1;
    while (end > 0 && lines[end - 1]?.trim() === "") {
      end -= 1;
    }
  }

  return lines.slice(0, end).join("\n");
}

function isTagOnlyLine(line: string) {
  const tokens = line.trim().split(/\s+/);
  return (
    tokens.length > 0 &&
    tokens.every(
      (token) => token.startsWith("#") && TAG_NAME_RE.test(token.slice(1)),
    )
  );
}
