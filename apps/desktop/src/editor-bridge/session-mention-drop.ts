import type { SessionMentionDropConfig } from "@hypr/editor/note";

import {
  hasSessionContextDragData,
  readSessionMentionDragData,
} from "~/chat/context/session-drag";

export const sessionMentionDropConfig = {
  has: hasSessionContextDragData,
  read: readSessionMentionDragData,
} satisfies SessionMentionDropConfig;
