import { defineCollection, defineConfig } from "@content-collections/core";
import { compileMDX } from "@content-collections/mdx";
import * as fs from "fs";
import GithubSlugger from "github-slugger/index.js";
import mdxMermaid from "mdx-mermaid";
import * as path from "path";
import rehypeAutolinkHeadings from "rehype-autolink-headings";
import rehypeSlug from "rehype-slug";
import remarkGfm from "remark-gfm";
import { createHighlighter, type Highlighter } from "shiki";
import { z } from "zod";

import { VersionPlatform } from "@/scripts/versioning";

const EXT_TO_LANG: Record<string, string> = {
  ".rs": "rust",
  ".ts": "typescript",
  ".tsx": "tsx",
  ".js": "javascript",
  ".jsx": "jsx",
  ".json": "json",
  ".toml": "toml",
  ".yaml": "yaml",
  ".yml": "yaml",
  ".py": "python",
  ".go": "go",
  ".css": "css",
  ".html": "html",
  ".md": "markdown",
  ".sh": "bash",
  ".sql": "sql",
  ".swift": "swift",
};

let highlighterPromise: Promise<Highlighter> | null = null;
function getHighlighter(): Promise<Highlighter> {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({
      themes: ["github-light"],
      langs: Object.values(EXT_TO_LANG),
    });
  }
  return highlighterPromise;
}

function parseLineRange(url: string): { startLine?: number; endLine?: number } {
  const lineMatch = url.match(/#L(\d+)(?:-L(\d+))?$/);
  if (!lineMatch) return {};
  return {
    startLine: parseInt(lineMatch[1], 10),
    endLine: lineMatch[2] ? parseInt(lineMatch[2], 10) : undefined,
  };
}

function extractLines(
  content: string,
  startLine?: number,
  endLine?: number,
): string {
  if (!startLine) return content;
  const lines = content.split("\n");
  const start = startLine - 1;
  const end = endLine ?? lines.length;
  return lines.slice(start, end).join("\n");
}

async function embedGithubCode(content: string): Promise<string> {
  const githubCodeRegex = /<GithubCode\s+url="([^"]+)"\s*\/>/g;
  let result = content;

  const matches = [...content.matchAll(githubCodeRegex)];
  for (const match of matches) {
    const [fullMatch, url] = match;

    const repoMatch = url.match(
      /github\.com\/fastrepl\/(hyprnote|char)\/blob\/[^/]+\/(.+?)(?:#L\d+(?:-L\d+)?)?$/,
    );
    if (repoMatch) {
      const filePath = repoMatch[2];
      const fileName = path.basename(filePath);
      const localPath = path.resolve(process.cwd(), "..", "..", filePath);
      const { startLine, endLine } = parseLineRange(url);

      try {
        const fileContent = fs.readFileSync(localPath, "utf-8");

        const codeBlockMatch = fileContent.match(/```(\w+)\n([\s\S]*?)```/);
        const rawCode = codeBlockMatch ? codeBlockMatch[2] : fileContent;
        const lang = codeBlockMatch
          ? codeBlockMatch[1]
          : EXT_TO_LANG[path.extname(fileName)];

        const slicedCode = extractLines(rawCode.trimEnd(), startLine, endLine);
        const escapedCode = JSON.stringify(slicedCode);
        const lineNum = startLine ?? 1;

        let highlightedAttr = "";
        if (lang) {
          const highlighter = await getHighlighter();
          const html = highlighter.codeToHtml(slicedCode, {
            lang,
            theme: "github-light",
          });
          highlightedAttr = ` highlightedHtml={${JSON.stringify(html)}}`;
        }

        const langAttr = lang ? ` language="${lang}"` : "";
        result = result.replace(
          fullMatch,
          `<GithubEmbed code={${escapedCode}} fileName="${fileName}" url="${url}" startLine={${lineNum}}${langAttr}${highlightedAttr} />`,
        );
      } catch {
        console.warn(`Failed to read local file: ${localPath}`);
      }
    }
  }

  return result;
}

function extractToc(
  content: string,
): Array<{ id: string; text: string; level: number }> {
  const toc: Array<{ id: string; text: string; level: number }> = [];
  const slugger = new GithubSlugger();
  const lines = content.split("\n");

  for (const line of lines) {
    const match = line.match(/^(#{2,4})\s+(.+)$/);
    if (match) {
      const level = match[1].length;
      const text = match[2].trim();
      const id = slugger.slug(text);

      toc.push({ id, text, level });
    }
  }

  return toc;
}

const articles = defineCollection({
  name: "articles",
  directory: "content/articles",
  include: "*.mdx",
  exclude: "AGENTS.md",
  schema: z.object({
    display_title: z.string().optional(),
    meta_title: z.string().default(""),
    meta_description: z.string().default(""),
    author: z.union([z.string(), z.array(z.string())]),
    date: z.string(),
    coverImage: z.string().optional(),
    featured: z.boolean().optional(),
    ready_for_review: z.boolean().default(false),
    category: z
      .enum([
        "Product",
        "Comparisons",
        "Engineering",
        "Founders' notes",
        "Guides",
        "Char Weekly",
      ])
      .optional(),
  }),
  transform: async (document, context) => {
    const toc = extractToc(document.content);

    const mdx = await compileMDX(context, document, {
      remarkPlugins: [remarkGfm, mdxMermaid],
      rehypePlugins: [
        rehypeSlug,
        [
          rehypeAutolinkHeadings,
          {
            behavior: "wrap",
            properties: {
              className: ["anchor"],
            },
          },
        ],
      ],
    });

    const slug = document._meta.path.replace(/\.mdx$/, "");

    const rawAuthor = document.author || "Char Team";
    const author = Array.isArray(rawAuthor) ? rawAuthor : [rawAuthor];
    const title = document.display_title || document.meta_title;

    return {
      ...document,
      mdx,
      slug,
      author,
      title,
      toc,
    };
  },
});

const changelog = defineCollection({
  name: "changelog",
  directory: "../../packages/changelog/content",
  include: "*.md",
  exclude: "AGENTS.md",
  schema: z.object({
    date: z
      .string()
      .trim()
      .transform((value) => (value === "" ? undefined : value))
      .optional(),
  }),
  transform: async (document, { skip }) => {
    if (!document.date) {
      return skip("missing changelog date");
    }

    const version = document._meta.path.replace(/\.md$/, "");
    const baseUrl = `https://github.com/fastrepl/char/releases/download/desktop_v${version}`;
    const downloads: Record<VersionPlatform, string> = {
      "dmg-aarch64": `${baseUrl}/char-macos-aarch64.dmg`,
      "appimage-x86_64": `${baseUrl}/char-linux-x86_64.AppImage`,
      "deb-x86_64": `${baseUrl}/char-linux-x86_64.deb`,
      "appimage-aarch64": `${baseUrl}/char-linux-aarch64.AppImage`,
      "deb-aarch64": `${baseUrl}/char-linux-aarch64.deb`,
    };

    return {
      ...document,
      slug: version,
      version,
      downloads,
    };
  },
});

const docs = defineCollection({
  name: "docs",
  directory: "content/docs",
  include: "**/*.mdx",
  exclude: "AGENTS.md",
  schema: z.object({
    title: z.string(),
    section: z.string(),
    summary: z.string().optional(),
  }),
  transform: async (document, context) => {
    const processedContent = await embedGithubCode(document.content);
    const processedDocument = { ...document, content: processedContent };

    const toc = extractToc(processedContent);

    const mdx = await compileMDX(context, processedDocument, {
      remarkPlugins: [remarkGfm, mdxMermaid],
      rehypePlugins: [
        rehypeSlug,
        [
          rehypeAutolinkHeadings,
          {
            behavior: "wrap",
            properties: {
              className: ["anchor"],
            },
          },
        ],
      ],
    });

    const pathParts = document._meta.path.split("/");
    const fileName = pathParts.pop()!.replace(/\.mdx$/, "");

    const sectionFolder = pathParts[0] || "general";

    const isIndex = fileName === "index";

    const orderMatch = fileName.match(/^(\d+)\./);
    const order = orderMatch ? parseInt(orderMatch[1], 10) : 999;

    const cleanFileName = fileName.replace(/^\d+\./, "");
    const filteredPathParts = pathParts.filter((part) => part !== "_");
    const cleanPath =
      filteredPathParts.length > 0
        ? `${filteredPathParts.join("/")}/${cleanFileName}`
        : cleanFileName;
    const slug = cleanPath;

    return {
      ...document,
      mdx,
      slug,
      sectionFolder,
      isIndex,
      order,
      toc,
    };
  },
});

const legal = defineCollection({
  name: "legal",
  directory: "content/legal",
  include: "*.mdx",
  exclude: "AGENTS.md",
  schema: z.object({
    title: z.string(),
    summary: z.string(),
    date: z.string(),
  }),
  transform: async (document, context) => {
    const mdx = await compileMDX(context, document, {
      remarkPlugins: [remarkGfm, mdxMermaid],
      rehypePlugins: [
        rehypeSlug,
        [
          rehypeAutolinkHeadings,
          {
            behavior: "wrap",
            properties: {
              className: ["anchor"],
            },
          },
        ],
      ],
    });

    const slug = document._meta.path.replace(/\.mdx$/, "");

    return {
      ...document,
      mdx,
      slug,
    };
  },
});

const templates = defineCollection({
  name: "templates",
  directory: "content/templates",
  include: "*.mdx",
  exclude: "AGENTS.md",
  schema: z.object({
    title: z.string(),
    description: z.string(),
    category: z.string(),
    targets: z.array(z.string()),
    banner: z.string().optional(),
    sections: z.array(
      z.object({
        title: z.string(),
        description: z.string().optional(),
      }),
    ),
  }),
  transform: async (document, context) => {
    const mdx = await compileMDX(context, document, {
      remarkPlugins: [remarkGfm, mdxMermaid],
      rehypePlugins: [
        rehypeSlug,
        [
          rehypeAutolinkHeadings,
          {
            behavior: "wrap",
            properties: {
              className: ["anchor"],
            },
          },
        ],
      ],
    });

    const slug = document._meta.path.replace(/\.mdx$/, "");

    return {
      ...document,
      mdx,
      slug,
    };
  },
});

const hooks = defineCollection({
  name: "hooks",
  directory: "content/hooks",
  include: "*.mdx",
  exclude: "AGENTS.md",
  schema: z.object({
    name: z.string(),
    description: z.string().nullable(),
    args: z
      .array(
        z.object({
          name: z.string(),
          type_name: z.string(),
          description: z.string().nullable(),
          optional: z.boolean().default(false),
        }),
      )
      .optional(),
  }),
  transform: async (document, context) => {
    const mdx = await compileMDX(context, document, {
      remarkPlugins: [remarkGfm, mdxMermaid],
      rehypePlugins: [
        rehypeSlug,
        [
          rehypeAutolinkHeadings,
          {
            behavior: "wrap",
            properties: {
              className: ["anchor"],
            },
          },
        ],
      ],
    });

    const slug = document._meta.path.replace(/\.mdx$/, "");

    return {
      ...document,
      mdx,
      slug,
    };
  },
});

const deeplinks = defineCollection({
  name: "deeplinks",
  directory: "content/deeplinks",
  include: "*.mdx",
  exclude: "AGENTS.md",
  schema: z.object({
    path: z.string(),
    description: z.string().nullable(),
    params: z
      .array(
        z.object({
          name: z.string(),
          type_name: z.string(),
          description: z.string().nullable(),
        }),
      )
      .optional(),
  }),
  transform: async (document, context) => {
    const mdx = await compileMDX(context, document, {
      remarkPlugins: [remarkGfm, mdxMermaid],
      rehypePlugins: [
        rehypeSlug,
        [
          rehypeAutolinkHeadings,
          {
            behavior: "wrap",
            properties: {
              className: ["anchor"],
            },
          },
        ],
      ],
    });

    const slug = document._meta.path.replace(/\.mdx$/, "");

    return {
      ...document,
      mdx,
      slug,
    };
  },
});

const vs = defineCollection({
  name: "vs",
  directory: "content/vs",
  include: "*.mdx",
  exclude: "AGENTS.md",
  schema: z.object({
    name: z.string(),
    icon: z.string(),
    headline: z.string(),
    description: z.string(),
    metaDescription: z.string(),
  }),
  transform: async (document, context) => {
    const mdx = await compileMDX(context, document, {
      remarkPlugins: [remarkGfm, mdxMermaid],
      rehypePlugins: [
        rehypeSlug,
        [
          rehypeAutolinkHeadings,
          {
            behavior: "wrap",
            properties: {
              className: ["anchor"],
            },
          },
        ],
      ],
    });

    const slug = document._meta.path.replace(/\.mdx$/, "");

    return {
      ...document,
      mdx,
      slug,
    };
  },
});

const integrations = defineCollection({
  name: "integrations",
  directory: "content/integrations",
  include: "**/*.mdx",
  exclude: "AGENTS.md",
  schema: z.object({
    platform: z.string(),
    icon: z.string(),
    headline: z.string(),
    description: z.string(),
    metaDescription: z.string(),
    features: z.array(z.string()).optional(),
  }),
  transform: async (document, context) => {
    const mdx = await compileMDX(context, document, {
      remarkPlugins: [remarkGfm, mdxMermaid],
      rehypePlugins: [
        rehypeSlug,
        [
          rehypeAutolinkHeadings,
          {
            behavior: "wrap",
            properties: {
              className: ["anchor"],
            },
          },
        ],
      ],
    });

    const pathParts = document._meta.path.split("/");
    const fileName = pathParts.pop()!.replace(/\.mdx$/, "");
    const category = pathParts[0] || "general";
    const slug = fileName;

    return {
      ...document,
      mdx,
      slug,
      category,
    };
  },
});

const shortcuts = defineCollection({
  name: "shortcuts",
  directory: "content/shortcuts",
  include: "*.mdx",
  exclude: "AGENTS.md",
  schema: z.object({
    title: z.string(),
    description: z.string(),
    category: z.string(),
    prompt: z.string(),
    banner: z.string().optional(),
    targets: z.array(z.string()).optional(),
  }),
  transform: async (document, context) => {
    const mdx = await compileMDX(context, document, {
      remarkPlugins: [remarkGfm, mdxMermaid],
      rehypePlugins: [
        rehypeSlug,
        [
          rehypeAutolinkHeadings,
          {
            behavior: "wrap",
            properties: {
              className: ["anchor"],
            },
          },
        ],
      ],
    });

    const slug = document._meta.path.replace(/\.mdx$/, "");

    return {
      ...document,
      mdx,
      slug,
    };
  },
});

const roadmap = defineCollection({
  name: "roadmap",
  directory: "content/roadmap",
  include: "*.mdx",
  exclude: "AGENTS.md",
  schema: z.object({
    title: z.string(),
    status: z.enum(["todo", "in-progress", "done"]),
    date: z.string(),
    labels: z.array(z.string()).optional(),
    priority: z.enum(["high", "mid", "low"]),
  }),
  transform: async (document, context) => {
    const mdx = await compileMDX(context, document, {
      remarkPlugins: [remarkGfm, mdxMermaid],
      rehypePlugins: [
        rehypeSlug,
        [
          rehypeAutolinkHeadings,
          {
            behavior: "wrap",
            properties: {
              className: ["anchor"],
            },
          },
        ],
      ],
    });

    const slug = document._meta.path.replace(/\.mdx$/, "");

    const githubIssueRegex =
      /https:\/\/github\.com\/[^/\s]+\/[^/\s]+\/issues\/\d+/g;
    const githubIssues = document.content.match(githubIssueRegex) || [];

    return {
      ...document,
      mdx,
      slug,
      githubIssues,
    };
  },
});

const ossFriends = defineCollection({
  name: "ossFriends",
  directory: "content/oss-friends",
  include: "*.mdx",
  exclude: "AGENTS.md",
  schema: z.object({
    name: z.string(),
    description: z.string(),
    href: z.string(),
    image: z.string().optional(),
    github: z.string(),
  }),
  transform: async (document) => {
    const slug = document._meta.path.replace(/\.mdx$/, "");

    return {
      ...document,
      slug,
    };
  },
});

const handbook = defineCollection({
  name: "handbook",
  directory: "content/handbook",
  include: "**/*.mdx",
  exclude: "AGENTS.md",
  schema: z.object({
    title: z.string(),
    section: z.string(),
    summary: z.string().optional(),
  }),
  transform: async (document, context) => {
    const toc = extractToc(document.content);

    const mdx = await compileMDX(context, document, {
      remarkPlugins: [remarkGfm, mdxMermaid],
      rehypePlugins: [
        rehypeSlug,
        [
          rehypeAutolinkHeadings,
          {
            behavior: "wrap",
            properties: {
              className: ["anchor"],
            },
          },
        ],
      ],
    });

    const pathParts = document._meta.path.split("/");
    const fileName = pathParts.pop()!.replace(/\.mdx$/, "");

    const sectionFolder = pathParts[0] || "general";

    const isIndex = fileName === "index";

    const orderMatch = fileName.match(/^(\d+)\./);
    const order = orderMatch ? parseInt(orderMatch[1], 10) : 999;

    const cleanFileName = fileName.replace(/^\d+\./, "");
    const cleanPath =
      pathParts.length > 0
        ? `${pathParts.join("/")}/${cleanFileName}`
        : cleanFileName;
    const slug = cleanPath;

    return {
      ...document,
      mdx,
      slug,
      sectionFolder,
      isIndex,
      order,
      toc,
    };
  },
});

const updates = defineCollection({
  name: "updates",
  directory: "content/updates",
  include: "*.mdx",
  exclude: "AGENTS.md",
  schema: z.object({
    title: z.string(),
    date: z.string(),
  }),
  transform: async (document, context) => {
    const mdx = await compileMDX(context, document, {
      remarkPlugins: [remarkGfm, mdxMermaid],
      rehypePlugins: [
        rehypeSlug,
        [
          rehypeAutolinkHeadings,
          {
            behavior: "wrap",
            properties: {
              className: ["anchor"],
            },
          },
        ],
      ],
    });

    const slug = document._meta.path.replace(/\.mdx$/, "");

    return {
      ...document,
      mdx,
      slug,
    };
  },
});

const jobs = defineCollection({
  name: "jobs",
  directory: "content/jobs",
  include: "*.mdx",
  exclude: "AGENTS.md",
  schema: z.object({
    title: z.string(),
    description: z.string(),
    cardDescription: z.string(),
    backgroundImage: z.string(),
    applyUrl: z.string().optional(),
    published: z.boolean().default(true),
  }),
  transform: async (document, context) => {
    const mdx = await compileMDX(context, document, {
      remarkPlugins: [remarkGfm, mdxMermaid],
      rehypePlugins: [
        rehypeSlug,
        [
          rehypeAutolinkHeadings,
          {
            behavior: "wrap",
            properties: {
              className: ["anchor"],
            },
          },
        ],
      ],
    });

    const slug = document._meta.path.replace(/\.mdx$/, "");

    return {
      ...document,
      mdx,
      slug,
    };
  },
});

export default defineConfig({
  collections: [
    articles,
    changelog,
    docs,
    legal,
    templates,
    shortcuts,
    hooks,
    deeplinks,
    vs,
    integrations,
    handbook,
    roadmap,
    ossFriends,
    jobs,
    updates,
  ],
});
