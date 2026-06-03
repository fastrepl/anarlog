import type { SessionMentionDropConfig } from "@hypr/editor/note";

import { readSessionMentionDragData } from "~/chat/context/session-drag";

export const sessionMentionDropConfig = {
  read: readSessionMentionDragData,
} satisfies SessionMentionDropConfig;
