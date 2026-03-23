import { type Mark, type Node as PMNode } from "prosemirror-model";
import { Plugin, PluginKey, type Transaction } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";
import tldList from "tlds";

import {
  findHashtags,
  parseYouTubeClipId,
  parseYouTubeEmbedSnippet,
  parseYouTubeUrl,
  resolveYouTubeClipUrl,
} from "@hypr/tiptap/shared";

import { schema } from "./schema";

// ---------------------------------------------------------------------------
// Hashtag decorations
// ---------------------------------------------------------------------------
export const hashtagPluginKey = new PluginKey("hashtagDecoration");

export function hashtagPlugin() {
  return new Plugin({
    key: hashtagPluginKey,
    props: {
      decorations(state) {
        const { doc } = state;
        const decorations: Decoration[] = [];

        doc.descendants((node: PMNode, pos: number) => {
          if (!node.isText || !node.text) return;
          for (const match of findHashtags(node.text)) {
            decorations.push(
              Decoration.inline(pos + match.start, pos + match.end, {
                class: "hashtag",
              }),
            );
          }
        });

        return DecorationSet.create(doc, decorations);
      },
    },
  });
}

// ---------------------------------------------------------------------------
// Search and Replace
// ---------------------------------------------------------------------------
export interface SearchAndReplaceStorage {
  searchTerm: string;
  replaceTerm: string;
  results: { from: number; to: number }[];
  lastSearchTerm: string;
  caseSensitive: boolean;
  lastCaseSensitive: boolean;
  resultIndex: number;
  lastResultIndex: number;
}

export function createSearchStorage(): SearchAndReplaceStorage {
  return {
    searchTerm: "",
    replaceTerm: "",
    results: [],
    lastSearchTerm: "",
    caseSensitive: false,
    lastCaseSensitive: false,
    resultIndex: 0,
    lastResultIndex: 0,
  };
}

export const searchPluginKey = new PluginKey("searchAndReplace");

function processSearches(
  doc: PMNode,
  searchTerm: RegExp,
  searchResultClass: string,
  resultIndex: number,
): { decorations: DecorationSet; results: { from: number; to: number }[] } {
  const decorations: Decoration[] = [];
  const results: { from: number; to: number }[] = [];

  let textNodesWithPosition: { text: string; pos: number }[] = [];
  let index = 0;

  doc.descendants((node, pos) => {
    if (node.isText) {
      if (textNodesWithPosition[index]) {
        textNodesWithPosition[index] = {
          text: textNodesWithPosition[index].text + node.text,
          pos: textNodesWithPosition[index].pos,
        };
      } else {
        textNodesWithPosition[index] = { text: `${node.text}`, pos };
      }
    } else {
      index += 1;
    }
  });

  textNodesWithPosition = textNodesWithPosition.filter(Boolean);

  for (const element of textNodesWithPosition) {
    const { text, pos } = element;
    const matches = Array.from(text.matchAll(searchTerm)).filter(
      ([matchText]) => matchText.trim(),
    );

    for (const m of matches) {
      if (m[0] === "" || m.index === undefined) continue;
      results.push({ from: pos + m.index, to: pos + m.index + m[0].length });
    }
  }

  for (let i = 0; i < results.length; i++) {
    const r = results[i];
    const className =
      i === resultIndex
        ? `${searchResultClass} ${searchResultClass}-current`
        : searchResultClass;
    decorations.push(Decoration.inline(r.from, r.to, { class: className }));
  }

  return {
    decorations: DecorationSet.create(doc, decorations),
    results,
  };
}

export function searchAndReplacePlugin(storage: SearchAndReplaceStorage) {
  return new Plugin({
    key: searchPluginKey,
    state: {
      init: () => DecorationSet.empty,
      apply({ doc, docChanged }, oldState) {
        const {
          searchTerm,
          lastSearchTerm,
          caseSensitive,
          lastCaseSensitive,
          resultIndex,
          lastResultIndex,
        } = storage;

        if (
          !docChanged &&
          lastSearchTerm === searchTerm &&
          lastCaseSensitive === caseSensitive &&
          lastResultIndex === resultIndex
        ) {
          return oldState;
        }

        storage.lastSearchTerm = searchTerm;
        storage.lastCaseSensitive = caseSensitive;
        storage.lastResultIndex = resultIndex;

        if (!searchTerm) {
          storage.results = [];
          return DecorationSet.empty;
        }

        const escaped = searchTerm.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
        const regex = new RegExp(escaped, caseSensitive ? "gu" : "gui");

        const { decorations, results } = processSearches(
          doc,
          regex,
          "search-result",
          resultIndex,
        );

        storage.results = results;
        return decorations;
      },
    },
    props: {
      decorations(state) {
        return this.getState(state);
      },
    },
  });
}

// ---------------------------------------------------------------------------
// Placeholder
// ---------------------------------------------------------------------------
export type PlaceholderFunction = (props: {
  node: PMNode;
  pos: number;
  hasAnchor: boolean;
}) => string;

export const placeholderPluginKey = new PluginKey("placeholder");

export function placeholderPlugin(placeholder?: PlaceholderFunction) {
  return new Plugin({
    key: placeholderPluginKey,
    props: {
      decorations(state) {
        const { doc, selection } = state;
        const { anchor } = selection;
        const decorations: Decoration[] = [];

        const isEmptyDoc =
          doc.childCount === 1 &&
          doc.firstChild!.isTextblock &&
          doc.firstChild!.content.size === 0;

        doc.descendants((node, pos) => {
          const hasAnchor = anchor >= pos && anchor <= pos + node.nodeSize;
          const isEmpty = !node.isLeaf && node.content.size === 0;

          if (hasAnchor && isEmpty) {
            const classes = ["is-empty"];
            if (isEmptyDoc) classes.push("is-editor-empty");

            const text = placeholder
              ? placeholder({ node, pos, hasAnchor })
              : "";

            if (text) {
              decorations.push(
                Decoration.node(pos, pos + node.nodeSize, {
                  class: classes.join(" "),
                  "data-placeholder": text,
                }),
              );
            }
          }

          return false;
        });

        return DecorationSet.create(doc, decorations);
      },
    },
  });
}

// ---------------------------------------------------------------------------
// Clear marks on enter
// ---------------------------------------------------------------------------
const INLINE_MARK_NAMES = ["bold", "italic"];

export function clearMarksOnEnterPlugin() {
  return new Plugin({
    key: new PluginKey("clearMarksOnEnter"),
    appendTransaction(transactions, oldState, newState) {
      if (!transactions.some((tr) => tr.docChanged)) return null;
      if (newState.doc.content.size <= oldState.doc.content.size) return null;

      const { $head } = newState.selection;
      const currentNode = $head.parent;

      if (
        currentNode.type.name !== "paragraph" ||
        currentNode.content.size !== 0 ||
        $head.parentOffset !== 0
      ) {
        return null;
      }

      const storedMarks = newState.storedMarks;
      if (!storedMarks || storedMarks.length === 0) return null;

      const filtered = storedMarks.filter(
        (mark) => !INLINE_MARK_NAMES.includes(mark.type.name),
      );

      if (filtered.length === storedMarks.length) return null;
      return newState.tr.setStoredMarks(filtered);
    },
  });
}

// ---------------------------------------------------------------------------
// Clip paste handler (YouTube embeds)
// ---------------------------------------------------------------------------
export function clipPastePlugin() {
  const nodeType = schema.nodes.clip;
  return new Plugin({
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
  });
}

// ---------------------------------------------------------------------------
// Link boundary guard
// ---------------------------------------------------------------------------
const VALID_TLDS = new Set(tldList.map((t: string) => t.toLowerCase()));

function isValidUrl(url: string): boolean {
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return false;
    }
    const parts = parsed.hostname.split(".");
    if (parts.length < 2) return false;
    return VALID_TLDS.has(parts[parts.length - 1].toLowerCase());
  } catch {
    return false;
  }
}

export function linkBoundaryGuardPlugin() {
  return new Plugin({
    key: new PluginKey("linkBoundaryGuard"),
    appendTransaction(transactions, _oldState, newState) {
      if (!transactions.some((tr) => tr.docChanged)) return null;
      const linkType = newState.schema.marks.link;
      if (!linkType) return null;

      let tr: Transaction | null = null;
      let prevLink: {
        startPos: number;
        endPos: number;
        mark: Mark;
      } | null = null;

      newState.doc.descendants((node, pos) => {
        if (!node.isText || !node.text) {
          prevLink = null;
          return;
        }

        const linkMark = node.marks.find((m) => m.type === linkType);

        if (linkMark) {
          const textLooksLikeUrl =
            node.text.startsWith("https://") || node.text.startsWith("http://");

          if (textLooksLikeUrl && !isValidUrl(node.text)) {
            if (!tr) tr = newState.tr;
            tr.removeMark(pos, pos + node.text.length, linkType);
            prevLink = null;
          } else if (node.text === linkMark.attrs.href) {
            prevLink = {
              startPos: pos,
              endPos: pos + node.text.length,
              mark: linkMark,
            };
          } else if (textLooksLikeUrl) {
            const updatedMark = linkType.create({
              ...linkMark.attrs,
              href: node.text,
            });
            if (!tr) tr = newState.tr;
            tr.removeMark(pos, pos + node.text.length, linkType);
            tr.addMark(pos, pos + node.text.length, updatedMark);
            prevLink = {
              startPos: pos,
              endPos: pos + node.text.length,
              mark: updatedMark,
            };
          } else {
            prevLink = null;
          }
        } else if (prevLink && pos === prevLink.endPos && node.text) {
          if (!/^\s/.test(node.text[0])) {
            const wsIdx = node.text.search(/\s/);
            const extendLen = wsIdx >= 0 ? wsIdx : node.text.length;
            const newHref =
              prevLink.mark.attrs.href + node.text.slice(0, extendLen);
            if (isValidUrl(newHref)) {
              if (!tr) tr = newState.tr;
              tr.removeMark(prevLink.startPos, prevLink.endPos, linkType);
              tr.addMark(
                prevLink.startPos,
                pos + extendLen,
                linkType.create({ ...prevLink.mark.attrs, href: newHref }),
              );
            }
          }
          prevLink = null;
        } else {
          prevLink = null;
        }
      });

      return tr;
    },
  });
}

// ---------------------------------------------------------------------------
// File handler (image drop/paste)
// ---------------------------------------------------------------------------
export type FileHandlerConfig = {
  onDrop?: (files: File[], pos?: number) => boolean | void;
  onPaste?: (files: File[]) => boolean | void;
  onImageUpload?: (
    file: File,
  ) => Promise<{ url: string; attachmentId: string }>;
};

const IMAGE_MIME_TYPES = ["image/png", "image/jpeg", "image/gif", "image/webp"];

export function fileHandlerPlugin(config: FileHandlerConfig) {
  const imageType = schema.nodes.image;

  function insertImage(
    view: import("prosemirror-view").EditorView,
    url: string,
    attachmentId: string | null,
    pos?: number,
  ) {
    const node = imageType.create({ src: url, attachmentId });
    const tr =
      pos != null
        ? view.state.tr.insert(pos, node)
        : view.state.tr.replaceSelectionWith(node);
    view.dispatch(tr);
  }

  async function handleFiles(
    view: import("prosemirror-view").EditorView,
    files: File[],
    pos?: number,
  ) {
    for (const file of files) {
      if (!IMAGE_MIME_TYPES.includes(file.type)) continue;

      if (config.onImageUpload) {
        try {
          const { url, attachmentId } = await config.onImageUpload(file);
          insertImage(view, url, attachmentId, pos);
        } catch (error) {
          console.error("Failed to upload image:", error);
        }
      } else {
        const reader = new FileReader();
        reader.readAsDataURL(file);
        reader.onload = () => {
          insertImage(view, reader.result as string, null, pos);
        };
      }
    }
  }

  return new Plugin({
    key: new PluginKey("fileHandler"),
    props: {
      handleDrop(view, event) {
        const files = Array.from(event.dataTransfer?.files ?? []).filter((f) =>
          IMAGE_MIME_TYPES.includes(f.type),
        );
        if (files.length === 0) return false;

        event.preventDefault();
        const pos = view.posAtCoords({
          left: event.clientX,
          top: event.clientY,
        })?.pos;

        if (config.onDrop) {
          const result = config.onDrop(files, pos);
          if (result === false) return false;
        }

        handleFiles(view, files, pos);
        return true;
      },

      handlePaste(view, event) {
        const files = Array.from(event.clipboardData?.files ?? []).filter((f) =>
          IMAGE_MIME_TYPES.includes(f.type),
        );
        if (files.length === 0) return false;

        if (config.onPaste) {
          const result = config.onPaste(files);
          if (result === false) return false;
        }

        handleFiles(view, files);
        return true;
      },
    },
  });
}
