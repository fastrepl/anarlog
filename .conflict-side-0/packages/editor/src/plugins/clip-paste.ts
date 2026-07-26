import type { NodeSpec } from "prosemirror-model";
import { Plugin, PluginKey } from "prosemirror-state";

// ---------------------------------------------------------------------------
// YouTube URL parsing utilities
// ---------------------------------------------------------------------------

export function parseYouTubeClipId(url: string): string | null {
  const match = url
    .trim()
    .match(/(?:youtube\.com|youtu\.be)\/clip\/([a-zA-Z0-9_-]+)/);
  return match ? match[1] : null;
}

function normalizeYouTubeTime(value: string | null): string | null {
  if (!value) return null;
  return value.replace(/s$/, "");
}

function buildYouTubeEmbedUrl(videoId: string, url: URL): string {
  const params = new URLSearchParams();
  const clip = url.searchParams.get("clip");
  const clipt = url.searchParams.get("clipt");
  const start =
    normalizeYouTubeTime(url.searchParams.get("t")) ||
    normalizeYouTubeTime(url.searchParams.get("start"));

  if (clip) params.set("clip", clip);
  if (clipt) params.set("clipt", clipt);
  if (start) params.set("start", start);

  const qs = params.toString();

  return `https://www.youtube.com/embed/${videoId}${qs ? `?${qs}` : ""}`;
}

function extractHtmlAttributeValue(
  html: string,
  attributeName: string,
): string | null {
  const match = html.match(
    new RegExp(`\\b${attributeName}\\s*=\\s*["']([^"']+)["']`, "i"),
  );

  return match?.[1] ?? null;
}

function parseClipMarkdown(
  markdown: string,
): { raw: string; embedUrl: string } | null {
  const clipMatch = markdown.match(
    /^<Clip\b[^>]*\bsrc\s*=\s*["']([^"']+)["'][^>]*(?:\/>|><\/Clip>)/i,
  );

  if (clipMatch) {
    const parsed = parseYouTubeUrl(clipMatch[1]);
    if (parsed) {
      return { raw: clipMatch[0], embedUrl: parsed.embedUrl };
    }
  }

  const iframeMatch = markdown.match(
    /^<iframe\b[^>]*\bsrc\s*=\s*["']([^"']+)["'][^>]*>\s*<\/iframe>/i,
  );

  if (iframeMatch) {
    const parsed = parseYouTubeUrl(iframeMatch[1]);
    if (parsed) {
      return { raw: iframeMatch[0], embedUrl: parsed.embedUrl };
    }
  }

  return null;
}

export function parseYouTubeUrl(url: string): { embedUrl: string } | null {
  const trimmed = url.trim();

  if (parseYouTubeClipId(trimmed)) return null;

  try {
    const urlObj = new URL(trimmed);
    const hostname = urlObj.hostname.toLowerCase().replace(/^www\./, "");
    const pathParts = urlObj.pathname.split("/").filter(Boolean);

    let videoId = "";

    if (hostname === "youtu.be") {
      videoId = pathParts[0] || "";
    } else if (
      hostname === "youtube.com" ||
      hostname === "m.youtube.com" ||
      hostname === "youtube-nocookie.com"
    ) {
      if (pathParts[0] === "watch") {
        videoId = urlObj.searchParams.get("v") || "";
      } else if (pathParts[0] === "embed" || pathParts[0] === "shorts") {
        videoId = pathParts[1] || "";
      }
    }

    if (!videoId) {
      return null;
    }

    return { embedUrl: buildYouTubeEmbedUrl(videoId, urlObj) };
  } catch {
    return null;
  }
}

export function parseYouTubeEmbedSnippet(
  snippet: string,
): { embedUrl: string } | null {
  const trimmed = snippet.trim();

  if (!trimmed) {
    return null;
  }

  const parsedMarkdown = parseClipMarkdown(trimmed);
  if (parsedMarkdown) {
    return { embedUrl: parsedMarkdown.embedUrl };
  }

  if (!/^<iframe\b/i.test(trimmed)) {
    return null;
  }

  const src = extractHtmlAttributeValue(trimmed, "src");
  return src ? parseYouTubeUrl(src) : null;
}

export async function resolveYouTubeClipUrl(
  clipId: string,
): Promise<{ embedUrl: string } | null> {
  try {
    const res = await fetch(`https://www.youtube.com/clip/${clipId}`);
    const html = await res.text();

    const videoIdMatch = html.match(/"videoId":"([a-zA-Z0-9_-]+)"/);
    if (!videoIdMatch) return null;

    return {
      embedUrl: `https://www.youtube.com/embed/${videoIdMatch[1]}`,
    };
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// Node spec
// ---------------------------------------------------------------------------

export const clipNodeSpec: NodeSpec = {
  group: "block",
  atom: true,
  attrs: { src: { default: null } },
  parseDOM: [
    {
      tag: 'div[data-type="clip"]',
      getAttrs(dom) {
        const src = (dom as HTMLElement).getAttribute("data-src");
        const parsed = src ? parseYouTubeUrl(src) : null;
        return parsed ? { src: parsed.embedUrl } : false;
      },
    },
    {
      tag: "iframe[src]",
      getAttrs(dom) {
        const src = (dom as HTMLElement).getAttribute("src");
        const parsed = src ? parseYouTubeUrl(src) : null;
        return parsed ? { src: parsed.embedUrl } : false;
      },
    },
  ],
  toDOM(node) {
    return ["div", { "data-type": "clip", "data-src": node.attrs.src }];
  },
};

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

export function clipPastePlugin() {
  return new Plugin({
    key: new PluginKey("clipPaste"),
    props: {
      handlePaste(view, event) {
        const nodeType = view.state.schema.nodes.clip;
        if (!nodeType) return false;

        const text = event.clipboardData?.getData("text/plain") || "";
        const html = event.clipboardData?.getData("text/html") || "";

        const embedSnippet = parseYouTubeEmbedSnippet(html || text);
        if (embedSnippet) {
          const { tr } = view.state;
          const node = nodeType.create({ src: embedSnippet.embedUrl });
          tr.replaceSelectionWith(node);
          view.dispatch(tr);
          return true;
        }

        if (!text) return false;

        const clipId = parseYouTubeClipId(text);
        if (clipId) {
          resolveYouTubeClipUrl(clipId).then((resolved) => {
            if (!resolved) return;
            const node = nodeType.create({ src: resolved.embedUrl });
            const tr = view.state.tr.replaceSelectionWith(node);
            view.dispatch(tr);
          });
          return true;
        }

        const parsed = parseYouTubeUrl(text);
        if (!parsed) return false;

        const { tr } = view.state;
        const node = nodeType.create({ src: parsed.embedUrl });
        tr.replaceSelectionWith(node);
        view.dispatch(tr);
        return true;
      },
    },
  });
}
