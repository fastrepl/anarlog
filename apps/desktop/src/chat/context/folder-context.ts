import { loadSessionSummariesByFolder } from "~/session/queries";

const FOLDER_CONTEXT_SESSION_LIMIT = 50;

export async function renderFolderContext(
  folderId: string,
): Promise<string | null> {
  const sessions = await loadSessionSummariesByFolder(folderId);
  const label = folderId || "No folder";
  const lines = [
    `Folder context: ${label}`,
    "Answer from notes in this folder. Use get_meeting or search_meetings with the listed IDs when you need full notes or transcripts.",
  ];

  if (sessions.length === 0) {
    lines.push("This folder has no notes yet.");
    return lines.join("\n");
  }

  const visible = sessions.slice(0, FOLDER_CONTEXT_SESSION_LIMIT);
  lines.push("");
  for (const session of visible) {
    const title = session.title.trim() || "Untitled";
    const date = session.created_at.trim();
    lines.push(`- ${title}${date ? ` (${date})` : ""} [${session.id}]`);
  }

  const hiddenCount = sessions.length - visible.length;
  if (hiddenCount > 0) {
    lines.push(`- and ${hiddenCount} more`);
  }

  return lines.join("\n");
}
