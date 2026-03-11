import { createFileRoute } from "@tanstack/react-router";
import { generateJSON } from "@tiptap/html";
import { Markdown } from "@tiptap/markdown";
import type { JSONContent } from "@tiptap/react";

import { getExtensions, json2md } from "@hypr/tiptap/shared";

import { fetchAdminUser } from "@/functions/admin";
import { getSupabaseServerClient } from "@/functions/supabase";
import { uploadMediaFile } from "@/functions/supabase-media";

import { getExtensionFromMimeType } from "../content/save";

interface ImportRequest {
  url: string;
  title?: string;
  author?: string[];
  description?: string;
  coverImage?: string;
  slug?: string;
}

interface ImportResponse {
  success: boolean;
  md?: string;
  frontmatter?: Record<string, string | boolean | string[]>;
  error?: string;
}

interface ParsedGoogleDocsUrl {
  docId: string;
  tabParam: string | null;
}

function parseGoogleDocsUrl(url: string): ParsedGoogleDocsUrl | null {
  const docIdPatterns = [
    /docs\.google\.com\/document\/d\/([a-zA-Z0-9_-]+)/,
    /docs\.google\.com\/document\/u\/\d+\/d\/([a-zA-Z0-9_-]+)/,
    /drive\.google\.com\/file\/d\/([a-zA-Z0-9_-]+)/,
  ];

  let docId: string | null = null;
  for (const pattern of docIdPatterns) {
    const match = url.match(pattern);
    if (match) {
      docId = match[1];
      break;
    }
  }

  if (!docId) {
    return null;
  }

  let tabParam: string | null = null;
  try {
    const urlObj = new URL(url);
    const tabValue = urlObj.searchParams.get("tab");
    if (tabValue) {
      tabParam = tabValue;
    }
  } catch {
    const tabMatch = url.match(/[?&]tab=([^&#]+)/);
    if (tabMatch) {
      tabParam = tabMatch[1];
    }
  }

  return { docId, tabParam };
}

interface Base64ImageNode {
  node: JSONContent;
  mimeType: string;
  base64Data: string;
}

function extractBase64ImageNodes(json: JSONContent): Base64ImageNode[] {
  const results: Base64ImageNode[] = [];

  function walk(node: JSONContent) {
    if (node.type === "image" && typeof node.attrs?.src === "string") {
      const match = node.attrs.src.match(/^data:image\/([^;]+);base64,(.+)$/);
      if (match) {
        results.push({ node, mimeType: match[1], base64Data: match[2] });
      }
    }
    if (node.content) {
      for (const child of node.content) {
        walk(child);
      }
    }
  }

  walk(json);
  return results;
}

function isEmptyNode(node: JSONContent): boolean {
  if (node.type === "text") {
    return !node.text || node.text.trim() === "";
  }
  if (node.type === "paragraph") {
    return !node.content || node.content.every(isEmptyNode);
  }
  if (node.type === "image") {
    return false;
  }
  if (!node.content || node.content.length === 0) {
    return true;
  }
  return node.content.every(isEmptyNode);
}

function clean(node: JSONContent): void {
  if (node.type === "text" && node.text) {
    node.text = node.text.replace(/\u00a0/g, " ");
  }

  if (node.content) {
    for (const child of node.content) {
      clean(child);
    }

    if (node.type === "listItem") {
      node.content = node.content.filter((child) => !isEmptyNode(child));
      if (node.content.length === 0) {
        node.content = [{ type: "paragraph" }];
      }
    } else if (
      node.type === "orderedList" ||
      node.type === "bulletList" ||
      node.type === "taskList"
    ) {
      node.content = node.content.filter(
        (child) => child.type !== "listItem" || !isEmptyNode(child),
      );
    } else if (node.type === "table") {
      node.content = node.content.filter(
        (row) => row.type !== "tableRow" || !isEmptyTableRow(row),
      );
      promoteTableHeader(node);
    } else if (node.type === "doc") {
      node.content = node.content.filter((child) => !isEmptyNode(child));
    } else {
      node.content = node.content.filter(
        (child) => !(child.type === "paragraph" && isEmptyNode(child)),
      );
      if (node.content.length === 0) {
        node.content = [{ type: "paragraph" }];
      }
    }
  }
}

function isEmptyTableRow(row: JSONContent): boolean {
  if (!row.content) return true;
  return row.content.every((cell) => {
    if (!cell.content) return true;
    return cell.content.every((node) => {
      if (node.type !== "paragraph") return false;
      if (!node.content || node.content.length === 0) return true;
      return node.content.every(
        (child) =>
          child.type === "text" && (!child.text || child.text.trim() === ""),
      );
    });
  });
}

function promoteTableHeader(table: JSONContent): void {
  if (!table.content || table.content.length === 0) return;
  const firstRow = table.content[0];
  if (!firstRow.content) return;
  const hasHeaderCells = firstRow.content.some(
    (cell) => cell.type === "tableHeader",
  );
  if (hasHeaderCells) return;
  for (const cell of firstRow.content) {
    if (cell.type === "tableCell") {
      cell.type = "tableHeader";
    }
  }
}

function cleanGoogleRedirectUrls(node: JSONContent): void {
  if (node.marks) {
    for (const mark of node.marks) {
      if (mark.type === "link" && typeof mark.attrs?.href === "string") {
        mark.attrs.href = resolveGoogleRedirect(mark.attrs.href);
      }
    }
  }
  if (node.content) {
    for (const child of node.content) {
      cleanGoogleRedirectUrls(child);
    }
  }
}

function resolveGoogleRedirect(url: string): string {
  try {
    const parsed = new URL(url);
    if (parsed.hostname === "www.google.com" && parsed.pathname === "/url") {
      const target = parsed.searchParams.get("q");
      if (target) return target;
    }
  } catch {}
  return url;
}

function extractTitle(html: string): string | null {
  const titleMatch = html.match(/<title[^>]*>([\s\S]*?)<\/title>/i);
  if (titleMatch) {
    let title = titleMatch[1].trim();
    title = title.replace(/ - Google Docs$/, "");
    return title;
  }
  return null;
}

function removeTabTitleFromContent(html: string): string {
  const bodyMatch = html.match(/<body[^>]*>([\s\S]*?)<\/body>/i);
  if (!bodyMatch) {
    return html;
  }

  let bodyContent = bodyMatch[1];

  const tabTitlePattern =
    /<p[^>]+class="[^"]*title[^"]*"[^>]*><span[^>]*>[^<]+<\/span><\/p>/gi;
  bodyContent = bodyContent.replace(tabTitlePattern, "");

  return html.replace(bodyMatch[1], bodyContent);
}

export const Route = createFileRoute("/api/admin/import/google-docs")({
  server: {
    handlers: {
      POST: async ({ request }) => {
        const isDev = process.env.NODE_ENV === "development";
        if (!isDev) {
          const user = await fetchAdminUser();
          if (!user?.isAdmin) {
            return new Response(JSON.stringify({ error: "Unauthorized" }), {
              status: 401,
              headers: { "Content-Type": "application/json" },
            });
          }
        }

        const supabase = getSupabaseServerClient();

        try {
          const body: ImportRequest = await request.json();
          const { url, title, author, description, coverImage, slug } = body;

          if (!url) {
            return new Response(
              JSON.stringify({ success: false, error: "URL is required" }),
              { status: 400, headers: { "Content-Type": "application/json" } },
            );
          }

          const parsedUrl = parseGoogleDocsUrl(url);
          if (!parsedUrl) {
            return new Response(
              JSON.stringify({
                success: false,
                error: "Invalid Google Docs URL",
              }),
              { status: 400, headers: { "Content-Type": "application/json" } },
            );
          }

          const { docId, tabParam } = parsedUrl;
          const tabQueryParam = tabParam || "t.0";

          let html: string;
          let response: Response;

          const publishedUrl = `https://docs.google.com/document/d/${docId}/pub?tab=${tabQueryParam}`;
          response = await fetch(publishedUrl);

          if (!response.ok) {
            const exportUrl = `https://docs.google.com/document/d/${docId}/export?format=html&tab=${tabQueryParam}`;
            response = await fetch(exportUrl);

            if (!response.ok) {
              return new Response(
                JSON.stringify({
                  success: false,
                  error:
                    "Failed to fetch document. Make sure it is either published to the web (File > Share > Publish to web) or shared with 'Anyone with the link can view' permissions.",
                }),
                {
                  status: 400,
                  headers: { "Content-Type": "application/json" },
                },
              );
            }
          }

          html = await response.text();

          html = removeTabTitleFromContent(html);
          const extractedTitle = extractTitle(html) || "Untitled";
          const finalTitle = title || extractedTitle;

          const bodyMatch = html.match(/<body[^>]*>([\s\S]*?)<\/body>/i);
          let bodyContent = bodyMatch ? bodyMatch[1] : html;
          bodyContent = bodyContent.replace(/&nbsp;/g, " ");

          const rawJson: JSONContent = generateJSON(bodyContent, [
            ...getExtensions(),
            Markdown,
          ]);
          clean(rawJson);
          cleanGoogleRedirectUrls(rawJson);

          const base64Images = extractBase64ImageNodes(rawJson);
          if (base64Images.length > 0) {
            if (!slug) {
              return new Response(
                JSON.stringify({
                  success: false,
                  error: "slug is required for image uploads",
                }),
                { status: 400 },
              );
            }

            const folder = `articles/${slug}`;
            for (let i = 0; i < base64Images.length; i++) {
              const image = base64Images[i];
              const extension = getExtensionFromMimeType(image.mimeType);
              const filename = `image-${i + 1}.${extension}`;
              const uploadResult = await uploadMediaFile(
                supabase,
                filename,
                image.base64Data,
                folder,
              );
              if (uploadResult.success && uploadResult.publicUrl) {
                image.node.attrs!.src = uploadResult.publicUrl;
              }
            }
          }

          const md = json2md(rawJson);

          const today = new Date().toISOString().split("T")[0];
          const finalAuthor = author || "Unknown";
          const finalDescription = description || "";

          const frontmatter = {
            meta_title: finalTitle,
            display_title: "",
            meta_description: finalDescription,
            author: finalAuthor,
            coverImage: coverImage || "",
            featured: false,
            date: today,
          };

          const result: ImportResponse = {
            success: true,
            md,
            frontmatter,
          };

          return new Response(JSON.stringify(result), {
            status: 200,
            headers: { "Content-Type": "application/json" },
          });
        } catch (err) {
          console.error(err);
          return new Response(
            JSON.stringify({
              success: false,
              error: (err as Error).message,
            }),
            { status: 500, headers: { "Content-Type": "application/json" } },
          );
        }
      },
    },
  },
});
