import { defineCollection, defineConfig } from "@content-collections/core";
import { compileMDX } from "@content-collections/mdx";
import mdxMermaid from "mdx-mermaid";
import rehypeAutolinkHeadings from "rehype-autolink-headings";
import rehypeSlug from "rehype-slug";
import remarkGfm from "remark-gfm";
import { z } from "zod";

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
    featured: z.boolean().optional(),
    ready_for_review: z.boolean().default(false),
    category: z
      .enum([
        "Product",
        "Comparisons",
        "Engineering",
        "Founders' notes",
        "Guides",
      ])
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

    const rawAuthor = document.author || "Anarlog Team";
    const author = Array.isArray(rawAuthor) ? rawAuthor : [rawAuthor];
    const title = document.display_title || document.meta_title;

    return {
      ...document,
      mdx,
      slug,
      author,
      title,
    };
  },
});

const legal = defineCollection({
  name: "legal",
  directory: "content/legal",
  include: "*.mdx",
  schema: z.object({
    title: z.string(),
    summary: z.string().optional(),
    date: z.string(),
  }),
  transform: async (document, context) => {
    const mdx = await compileMDX(context, document, {
      remarkPlugins: [remarkGfm],
      rehypePlugins: [rehypeSlug],
    });
    const slug = document._meta.path.replace(/\.mdx$/, "");
    return { ...document, mdx, slug };
  },
});

const emptyDocumentSchema = z.object({}).passthrough();

const docs = defineCollection({
  name: "docs",
  directory: "content/articles",
  include: "__no_docs_files__.mdx",
  schema: emptyDocumentSchema,
  transform: async (document) => document,
});

const handbook = defineCollection({
  name: "handbook",
  directory: "content/articles",
  include: "__no_handbook_files__.mdx",
  schema: emptyDocumentSchema,
  transform: async (document) => document,
});

const templates = defineCollection({
  name: "templates",
  directory: "content/articles",
  include: "__no_templates_files__.mdx",
  schema: emptyDocumentSchema,
  transform: async (document) => document,
});

const shortcuts = defineCollection({
  name: "shortcuts",
  directory: "content/articles",
  include: "__no_shortcuts_files__.mdx",
  schema: emptyDocumentSchema,
  transform: async (document) => document,
});

export default defineConfig({
  content: [articles, legal, docs, handbook, templates, shortcuts],
} as any);
