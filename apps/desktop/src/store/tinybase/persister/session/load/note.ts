import { commands as fsSyncCommands } from "@hypr/plugin-fs-sync";
import { md2json } from "@hypr/tiptap/shared";

import type { LoadedSessionData } from "./types";

import type { NoteFrontmatter } from "~/store/tinybase/persister/session/types";
import { SESSION_MEMO_FILE } from "~/store/tinybase/persister/shared";

const LABEL = "SessionPersister";

export async function processMdFile(
  path: string,
  content: string,
  result: LoadedSessionData,
): Promise<void> {
  try {
    const parseResult = await fsSyncCommands.deserialize(content);

    if (parseResult.status === "error") {
      console.error(
        `[${LABEL}] Failed to parse frontmatter from ${path}:`,
        parseResult.error,
      );
      return;
    }

    const { frontmatter, content: markdownBody } = parseResult.data;
    const fm = frontmatter as NoteFrontmatter;

    if (!fm.id || !fm.session_id) {
      return;
    }

    const tiptapJson = md2json(markdownBody);
    const tiptapContent = JSON.stringify(tiptapJson);

    if (path.endsWith(SESSION_MEMO_FILE)) {
      if (result.sessions[fm.session_id]) {
        result.sessions[fm.session_id].raw_md = tiptapContent;
      }
    } else {
      result.enhanced_notes[fm.id] = {
        user_id: "",
        session_id: fm.session_id,
        content: tiptapContent,
        template_id: fm.template_id ?? "",
        position: fm.position ?? 0,
        title: fm.title ?? "",
      };
    }
  } catch (error) {
    console.error(`[${LABEL}] Failed to load note from ${path}:`, error);
  }
}
