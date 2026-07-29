import type { SessionMentionDropConfig } from "@anlg/editor/note";

import {
  hasSessionContextDragData,
  readSessionMentionDragData,
} from "~/chat/context/session-drag";

export const sessionMentionDropConfig = {
  has: hasSessionContextDragData,
  read: readSessionMentionDragData,
} satisfies SessionMentionDropConfig;
