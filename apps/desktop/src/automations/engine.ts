import type { Session } from "@supabase/supabase-js";

import {
  linearCreateIssue,
  listConnections,
  notionAppendUpdate,
} from "@anlg/api-client";
import { createClient } from "@anlg/api-client/client";
import { json2md } from "@anlg/editor/markdown";
import { commands as localApiCommands } from "@anlg/plugin-local-api";

import { supabase } from "~/auth/client";
import { liveQueryClient } from "~/db";
import { env } from "~/env";
import {
  buildSlackRecap,
  sendSlackRecap,
} from "~/session-sharing/delivery-client";
import { getSessionShareSenderName } from "~/session-sharing/invitation-management";
import { getStoredSettingValues, setSettingValue } from "~/settings/queries";

const MAX_LINEAR_ISSUES_PER_MEETING = 10;
const MAX_PROCESSED_SESSIONS = 50;

export type AutomationRunRecord = {
  at: string;
  status: "success" | "error";
  detail: string;
};

export function parseAutomationRunRecord(
  value: string | undefined,
): AutomationRunRecord | null {
  if (!value) {
    return null;
  }
  try {
    const parsed = JSON.parse(value);
    if (
      typeof parsed?.at === "string" &&
      (parsed.status === "success" || parsed.status === "error") &&
      typeof parsed.detail === "string"
    ) {
      return parsed as AutomationRunRecord;
    }
  } catch {
    // fall through
  }
  return null;
}

export type AutomationTargetRef = {
  id: string;
  name: string;
};

export function parseAutomationTargetRef(
  value: string | undefined,
): AutomationTargetRef | null {
  if (!value) {
    return null;
  }
  try {
    const parsed = JSON.parse(value);
    if (typeof parsed?.id === "string" && typeof parsed.name === "string") {
      return { id: parsed.id, name: parsed.name };
    }
  } catch {
    // fall through
  }
  return null;
}

export async function runMeetingCompletedAutomations(
  sessionId: string,
): Promise<void> {
  try {
    await runMarkdownExport(sessionId);
  } catch (error) {
    console.error("[automations] meeting.completed run failed", error);
  }
}

export async function runNoteEnhancedAutomations(
  sessionId: string,
): Promise<void> {
  const runners = [runSlackRecap, runLinearIssues, runNotionUpdate];
  for (const runner of runners) {
    try {
      await runner(sessionId);
    } catch (error) {
      console.error("[automations] note.enhanced run failed", error);
    }
  }
}

async function runMarkdownExport(sessionId: string): Promise<void> {
  const { values } = await getStoredSettingValues();
  if (!values.automation_markdown_export_enabled) {
    return;
  }
  const directory = (values.automation_markdown_export_directory ?? "").trim();
  if (!directory) {
    return;
  }

  const record: AutomationRunRecord = {
    at: new Date().toISOString(),
    status: "success",
    detail: "",
  };
  try {
    const result = await localApiCommands.exportMeetingMarkdown(
      sessionId,
      directory,
    );
    if (result.status === "error") {
      throw new Error(result.error);
    }
    record.detail = result.data;
  } catch (error) {
    record.status = "error";
    record.detail = error instanceof Error ? error.message : String(error);
    console.error("[automations] markdown export failed", error);
  }
  await setSettingValue(
    "automation_markdown_export_last_run",
    JSON.stringify(record),
  );
}

async function runSlackRecap(sessionId: string): Promise<void> {
  const { values } = await getStoredSettingValues();
  if (!values.automation_slack_recap_enabled) {
    return;
  }
  const channel = parseAutomationTargetRef(
    values.automation_slack_recap_channel,
  );
  if (!channel) {
    return;
  }
  const processed = parseProcessedSessions(
    values.automation_slack_recap_processed,
  );
  if (processed.includes(sessionId)) {
    return;
  }

  const record: AutomationRunRecord = {
    at: new Date().toISOString(),
    status: "success",
    detail: "",
  };
  try {
    const recap = await loadMeetingRecap(sessionId);
    if (!recap) {
      throw new Error("no meeting summary is available yet");
    }
    const session = await requireSupabaseSession();
    await sendSlackRecap({
      apiBaseUrl: env.VITE_API_URL,
      accessToken: session.access_token,
      channel: channel.id,
      text: buildSlackRecap({
        senderName: getSessionShareSenderName(session.user),
        noteTitle: recap.title,
        noteBody: recap.body,
      }),
    });
    record.detail = `#${channel.name}`;
    await recordProcessedSession(
      "automation_slack_recap_processed",
      processed,
      sessionId,
    );
  } catch (error) {
    record.status = "error";
    record.detail = error instanceof Error ? error.message : String(error);
    console.error("[automations] slack recap failed", error);
  }
  await setSettingValue(
    "automation_slack_recap_last_run",
    JSON.stringify(record),
  );
}

async function runLinearIssues(sessionId: string): Promise<void> {
  const { values } = await getStoredSettingValues();
  if (!values.automation_linear_issues_enabled) {
    return;
  }
  const team = parseAutomationTargetRef(values.automation_linear_issues_team);
  if (!team) {
    return;
  }
  const processed = parseProcessedSessions(
    values.automation_linear_issues_processed,
  );
  if (processed.includes(sessionId)) {
    return;
  }

  const record: AutomationRunRecord = {
    at: new Date().toISOString(),
    status: "success",
    detail: "",
  };
  try {
    const actionItems = await loadMeetingActionItems(sessionId);
    if (actionItems.length === 0) {
      record.detail = "no action items found for this meeting";
    } else {
      const session = await requireSupabaseSession();
      const client = apiClientForSession(session);
      const connectionId = await findConnectionId(client, "linear");
      const recap = await loadMeetingRecap(sessionId);
      const description = recap
        ? `Action item from the Anarlog meeting "${recap.title}" (${recap.date}).`
        : "Action item from an Anarlog meeting.";
      const items = actionItems.slice(0, MAX_LINEAR_ISSUES_PER_MEETING);
      // Mark the session processed before creating anything: a mid-loop
      // failure must not re-create the earlier issues on the next enhance.
      await recordProcessedSession(
        "automation_linear_issues_processed",
        processed,
        sessionId,
      );
      for (const item of items) {
        const { error } = await linearCreateIssue({
          client,
          body: {
            connection_id: connectionId,
            team_id: team.id,
            title: item,
            description,
          },
        });
        if (error) {
          throw new Error(apiErrorMessage(error));
        }
      }
      record.detail =
        items.length === 1
          ? `1 issue in ${team.name}`
          : `${items.length} issues in ${team.name}`;
    }
  } catch (error) {
    record.status = "error";
    record.detail = error instanceof Error ? error.message : String(error);
    console.error("[automations] linear issues failed", error);
  }
  await setSettingValue(
    "automation_linear_issues_last_run",
    JSON.stringify(record),
  );
}

async function runNotionUpdate(sessionId: string): Promise<void> {
  const { values } = await getStoredSettingValues();
  if (!values.automation_notion_update_enabled) {
    return;
  }
  const page = parseAutomationTargetRef(values.automation_notion_update_page);
  if (!page) {
    return;
  }
  const processed = parseProcessedSessions(
    values.automation_notion_update_processed,
  );
  if (processed.includes(sessionId)) {
    return;
  }

  const record: AutomationRunRecord = {
    at: new Date().toISOString(),
    status: "success",
    detail: "",
  };
  try {
    const recap = await loadMeetingRecap(sessionId);
    if (!recap) {
      throw new Error("no meeting summary is available yet");
    }
    const session = await requireSupabaseSession();
    const client = apiClientForSession(session);
    const connectionId = await findConnectionId(client, "notion");
    const { error } = await notionAppendUpdate({
      client,
      body: {
        connection_id: connectionId,
        page_id: page.id,
        heading: `${recap.date} — ${recap.title}`,
        markdown: recap.body,
      },
    });
    if (error) {
      throw new Error(apiErrorMessage(error));
    }
    record.detail = page.name;
    await recordProcessedSession(
      "automation_notion_update_processed",
      processed,
      sessionId,
    );
  } catch (error) {
    record.status = "error";
    record.detail = error instanceof Error ? error.message : String(error);
    console.error("[automations] notion update failed", error);
  }
  await setSettingValue(
    "automation_notion_update_last_run",
    JSON.stringify(record),
  );
}

async function requireSupabaseSession(): Promise<Session> {
  if (!supabase) {
    throw new Error("sign in to run this automation");
  }
  const { data, error } = await supabase.auth.getSession();
  if (error) {
    throw error;
  }
  const session = data.session;
  if (!session || session.user.is_anonymous === true) {
    throw new Error("sign in to run this automation");
  }
  return session;
}

function apiClientForSession(session: Session) {
  return createClient({
    baseUrl: env.VITE_API_URL,
    headers: { Authorization: `Bearer ${session.access_token}` },
  });
}

async function findConnectionId(
  client: ReturnType<typeof createClient>,
  integrationId: string,
): Promise<string> {
  const { data, error } = await listConnections({ client });
  if (error) {
    throw new Error(apiErrorMessage(error));
  }
  const connection = (data?.connections ?? []).find(
    (candidate) => candidate.integration_id === integrationId,
  );
  if (!connection) {
    throw new Error(`connect ${integrationId} to run this automation`);
  }
  if (connection.status === "reconnect_required") {
    throw new Error(`reconnect ${integrationId} to run this automation`);
  }
  return connection.connection_id;
}

function apiErrorMessage(error: unknown): string {
  if (error && typeof error === "object") {
    const envelope = error as { error?: { message?: string } };
    if (typeof envelope.error?.message === "string") {
      return envelope.error.message;
    }
  }
  return "the API request failed";
}

function parseProcessedSessions(value: string | undefined): string[] {
  if (!value) {
    return [];
  }
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed)
      ? parsed.filter((entry): entry is string => typeof entry === "string")
      : [];
  } catch {
    return [];
  }
}

async function recordProcessedSession(
  settingKey:
    | "automation_linear_issues_processed"
    | "automation_slack_recap_processed"
    | "automation_notion_update_processed",
  processed: string[],
  sessionId: string,
): Promise<void> {
  const next = [...processed, sessionId].slice(-MAX_PROCESSED_SESSIONS);
  await setSettingValue(settingKey, JSON.stringify(next));
}

async function loadMeetingActionItems(sessionId: string): Promise<string[]> {
  const rows = await liveQueryClient.execute<{ text: string | null }>(
    `
      SELECT text
      FROM action_items
      WHERE session_id = ?
        AND deleted_at IS NULL
        AND completed_at IS NULL
        AND status NOT IN ('done', 'completed')
      ORDER BY source_order, id
    `,
    [sessionId],
  );
  const fromDb = rows
    .map((row) => (row.text ?? "").trim())
    .filter((text) => text.length > 0);
  if (fromDb.length > 0) {
    return fromDb;
  }
  return await loadSummaryTaskItems(sessionId);
}

// The editor syncs task rows only once a note is opened, so fall back to the
// task items embedded in the enhanced summary document itself.
async function loadSummaryTaskItems(sessionId: string): Promise<string[]> {
  const rows = await liveQueryClient.execute<{
    body: string | null;
    body_format: string | null;
  }>(
    `
      SELECT body, body_format
      FROM session_documents
      WHERE session_id = ?
        AND kind IN ('summary', 'template_output')
        AND deleted_at IS NULL
      ORDER BY sort_order, id
      LIMIT 1
    `,
    [sessionId],
  );
  const row = rows[0];
  if (!row?.body || row.body_format !== "prosemirror_json") {
    return [];
  }
  try {
    return collectUncheckedTaskItems(JSON.parse(row.body));
  } catch {
    return [];
  }
}

type ProsemirrorNode = {
  type?: string;
  text?: string;
  attrs?: { checked?: boolean };
  content?: ProsemirrorNode[];
};

function collectUncheckedTaskItems(node: ProsemirrorNode): string[] {
  const items: string[] = [];
  if (node.type === "taskItem") {
    if (node.attrs?.checked !== true) {
      const text = collectNodeText(node).trim();
      if (text) {
        items.push(text);
      }
    }
    return items;
  }
  for (const child of node.content ?? []) {
    items.push(...collectUncheckedTaskItems(child));
  }
  return items;
}

function collectNodeText(node: ProsemirrorNode): string {
  if (typeof node.text === "string") {
    return node.text;
  }
  return (node.content ?? []).map(collectNodeText).join("");
}

async function loadMeetingRecap(
  sessionId: string,
): Promise<{ title: string; date: string; body: string } | null> {
  const rows = await liveQueryClient.execute<{
    session_title: string | null;
    occurred_at: string | null;
    body: string | null;
    body_format: string | null;
  }>(
    `
      SELECT
        s.title AS session_title,
        COALESCE(NULLIF(s.started_at, ''), s.created_at) AS occurred_at,
        d.body,
        d.body_format
      FROM sessions s
      LEFT JOIN session_documents d
        ON d.session_id = s.id
        AND d.kind IN ('summary', 'template_output')
        AND d.deleted_at IS NULL
      WHERE s.id = ?
      ORDER BY d.sort_order, d.id
      LIMIT 1
    `,
    [sessionId],
  );
  const row = rows[0];
  if (!row?.body) {
    return null;
  }
  const body =
    row.body_format === "prosemirror_json"
      ? markdownFromProsemirror(row.body)
      : row.body.trim();
  if (!body) {
    return null;
  }
  return {
    title: row.session_title?.trim() || "Untitled meeting",
    date: (row.occurred_at ?? "").slice(0, 10),
    body,
  };
}

function markdownFromProsemirror(body: string): string {
  try {
    return json2md(JSON.parse(body)).trim();
  } catch {
    return "";
  }
}
