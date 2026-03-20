import { sep } from "@tauri-apps/api/path";

import type { ParsedDocument } from "@hypr/plugin-fs-sync";
import { isValidTiptapContent, json2md } from "@hypr/tiptap/shared";

import type { NoteFrontmatter } from "~/store/tinybase/persister/session/types";
import {
  buildSessionPath,
  iterateTableRows,
  sanitizeFilename,
  SESSION_MEMO_FILE,
  type TablesContent,
  type WriteOperation,
} from "~/store/tinybase/persister/shared";
import type { Store } from "~/store/tinybase/store/main";

type DocumentItem = [ParsedDocument, string];

type BuildContext = {
  store: Store;
  tables: TablesContent;
  dataDir: string;
  changedSessionIds?: Set<string>;
};

export function buildNoteSaveOps(
  store: Store,
  tables: TablesContent,
  dataDir: string,
  changedSessionIds?: Set<string>,
): WriteOperation[] {
  const ctx: BuildContext = { store, tables, dataDir, changedSessionIds };

  const enhancedNoteItems = collectEnhancedNotes(ctx);
  const { items: memoItems, deletePaths: memoDeletePaths } = collectMemos(ctx);

  return buildOperations([...enhancedNoteItems, ...memoItems], memoDeletePaths);
}

function collectEnhancedNotes(ctx: BuildContext): DocumentItem[] {
  const { store, tables, dataDir, changedSessionIds } = ctx;

  return Array.from(iterateTableRows(tables, "enhanced_notes"))
    .filter((note) => note.content && note.session_id)
    .filter(
      (note) => !changedSessionIds || changedSessionIds.has(note.session_id!),
    )
    .map((note) => {
      const markdown = tryParseAndConvertToMarkdown(note.content!);
      if (!markdown) return null;

      const session = tables.sessions?.[note.session_id!];
      const sessionDir = buildSessionPath(
        dataDir,
        note.session_id!,
        session?.folder_id ?? "",
      );
      const path = [sessionDir, getEnhancedNoteFilename(store, note)].join(
        sep(),
      );

      const frontmatter: NoteFrontmatter = {
        id: note.id,
        session_id: note.session_id!,
        template_id: note.template_id || undefined,
        position: note.position,
        title: note.title || undefined,
      };

      return [{ frontmatter, content: markdown }, path] as DocumentItem;
    })
    .filter((item): item is DocumentItem => item !== null);
}

function collectMemos(ctx: BuildContext): {
  items: DocumentItem[];
  deletePaths: string[];
} {
  const { tables, dataDir, changedSessionIds } = ctx;
  const items: DocumentItem[] = [];
  const deletePaths: string[] = [];

  for (const session of iterateTableRows(tables, "sessions")) {
    if (changedSessionIds && !changedSessionIds.has(session.id)) continue;

    const sessionDir = buildSessionPath(
      dataDir,
      session.id,
      session.folder_id ?? "",
    );
    const memoPath = [sessionDir, SESSION_MEMO_FILE].join(sep());

    const markdown = session.raw_md
      ? tryParseAndConvertToMarkdown(session.raw_md)
      : null;
    if (!markdown) {
      deletePaths.push(memoPath);
      continue;
    }

    const frontmatter: NoteFrontmatter = {
      id: session.id,
      session_id: session.id,
    };

    items.push([{ frontmatter, content: markdown }, memoPath]);
  }

  return { items, deletePaths };
}

function buildOperations(
  items: DocumentItem[],
  deletePaths: string[],
): WriteOperation[] {
  const operations: WriteOperation[] = [];

  if (items.length > 0) {
    operations.push({ type: "write-document-batch", items });
  }
  if (deletePaths.length > 0) {
    operations.push({ type: "delete", paths: deletePaths });
  }

  return operations;
}

function tryParseAndConvertToMarkdown(content: string): string | undefined {
  let parsed: unknown;
  try {
    parsed = JSON.parse(content);
  } catch {
    return content.trim() || undefined;
  }

  if (!isValidTiptapContent(parsed)) {
    return undefined;
  }

  return json2md(parsed);
}

function getEnhancedNoteFilename(
  store: Store,
  enhancedNote: ReturnType<typeof iterateTableRows<"enhanced_notes">>[number],
): string {
  if (enhancedNote.template_id) {
    const templateTitle = store.getCell(
      "templates",
      enhancedNote.template_id,
      "title",
    );
    const safeName = sanitizeFilename(
      templateTitle || enhancedNote.template_id,
    );
    return `${safeName}.md`;
  }
  return "_summary.md";
}
