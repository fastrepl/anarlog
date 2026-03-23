import { mergeAttributes, Node } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";

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

function getClipSrc(attrs: { src?: string | null }): { src: string } | false {
  const parsed = attrs.src ? parseYouTubeUrl(attrs.src) : null;
  return parsed ? { src: parsed.embedUrl } : false;
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

export const ClipNode = Node.create({
  name: "clip",
  group: "block",
  atom: true,

  addAttributes() {
    return {
      src: { default: null },
    };
  },

  parseHTML() {
    return [
      {
        tag: 'div[data-type="clip"]',
        getAttrs: (dom) =>
          getClipSrc({
            src: (dom as HTMLElement).getAttribute("data-src"),
          }),
      },
      {
        tag: "iframe[src]",
        getAttrs: (dom) =>
          getClipSrc({
            src: (dom as HTMLElement).getAttribute("src"),
          }),
      },
      {
        tag: "clip[src]",
        getAttrs: (dom) =>
          getClipSrc({
            src: (dom as HTMLElement).getAttribute("src"),
          }),
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      "div",
      mergeAttributes(HTMLAttributes, {
        "data-type": "clip",
        "data-src": HTMLAttributes.src,
      }),
    ];
  },

  addProseMirrorPlugins() {
    const nodeType = this.type;
    return [
      new Plugin({
        key: new PluginKey("clipPaste"),
        props: {
          handlePaste(view, event) {
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
      }),
    ];
  },

  markdownTokenizer: {
    name: "clip",
    level: "block",
    start: (src: string) => src.match(/<(?:Clip|iframe)\b/i)?.index ?? -1,
    tokenize: (src: string) => {
      const parsed = parseClipMarkdown(src);
      if (!parsed) {
        return undefined;
      }

      return {
        type: "clip",
        raw: parsed.raw,
        src: parsed.embedUrl,
      };
    },
  },

  parseMarkdown: (token: Record<string, string>) => {
    const parsed =
      (token.src ? parseYouTubeUrl(token.src) : null) ||
      parseYouTubeEmbedSnippet(token.text || token.raw || "");

    return {
      type: "clip",
      attrs: {
        src: parsed?.embedUrl ?? null,
      },
    };
  },

  renderMarkdown: (node: { attrs?: { src?: string } }) => {
    const src = (node.attrs?.src || "").replace(/"/g, "&quot;");
    return `<Clip src="${src}" />`;
  },
});
