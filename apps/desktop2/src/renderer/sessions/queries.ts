import { desc, sessions } from "@hypr/db";

import { db } from "~/db";

// Shared drizzle query + row type for listings of sessions (tab strip,
// open-note dialog, …). Hoisted so the query stays single-sourced across
// consumers and the renderer's live-query cache can dedupe it.
export const sessionsListQuery = db
  .select({
    id: sessions.id,
    title: sessions.title,
    updatedAt: sessions.updatedAt,
  })
  .from(sessions)
  .orderBy(desc(sessions.updatedAt), desc(sessions.id));

export type SessionSummaryRow = {
  id: string;
  title: string;
  updatedAt: string;
};
