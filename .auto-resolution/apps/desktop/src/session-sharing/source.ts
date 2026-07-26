import { md2json } from "@hypr/editor/markdown";
import type { JSONContent } from "@hypr/editor/note";

import { liveQueryClient, useLiveQuery } from "~/db";
import { flushDatabaseWrites } from "~/db/write-queue";
import { DEFAULT_USER_ID } from "~/shared/utils";

const EMPTY_DOCUMENT: JSONContent = { type: "doc", content: [] };
const EMPTY_WORKSPACES: AvailableShareWorkspace[] = [];
const MAX_DOCUMENT_DEPTH = 64;
const MAX_DOCUMENT_NODES = 50_000;

type SessionShareSourceSqlRow = {
  id: string;
  document_id: string | null;
  workspace_id: string;
  title: string;
  body: string;
  body_format: string;
  personal_workspace_available: number | boolean;
  assigned_workspace_kind: string | null;
  assigned_workspace_deleted_at: string | null;
  assigned_workspace_role: string | null;
  binding_json: string | null;
};

type AvailableShareWorkspaceSqlRow = {
  id: string;
  name: string;
};

export type SessionShareSource = {
  sessionId: string;
  documentId: string | null;
  workspaceId: string;
  title: string;
  body: JSONContent;
  rawBody: string;
  bodyFormat: string;
};

export type AvailableShareWorkspace = {
  id: string;
  name: string;
};

const SESSION_SHARE_SOURCE_SQL = `
  SELECT
    session.id,
    note.id AS document_id,
    session.workspace_id,
    session.title,
    COALESCE(note.body, '') AS body,
    COALESCE(note.body_format, 'prosemirror_json') AS body_format,
    EXISTS (
      SELECT 1
      FROM workspaces AS personal_workspace
      JOIN workspace_memberships AS personal_membership
        ON personal_membership.workspace_id = personal_workspace.id
        AND personal_membership.user_id = ?
        AND personal_membership.role = 'owner'
        AND personal_membership.deleted_at IS NULL
      WHERE personal_workspace.id = ?
        AND personal_workspace.owner_user_id = ?
        AND personal_workspace.kind = 'personal'
        AND personal_workspace.deleted_at IS NULL
    ) AS personal_workspace_available,
    (
      SELECT workspace.kind
      FROM workspaces AS workspace
      WHERE workspace.id = session.workspace_id
      LIMIT 1
    ) AS assigned_workspace_kind,
    (
      SELECT workspace.deleted_at
      FROM workspaces AS workspace
      WHERE workspace.id = session.workspace_id
      LIMIT 1
    ) AS assigned_workspace_deleted_at,
    (
      SELECT membership.role
      FROM workspace_memberships AS membership
      WHERE membership.workspace_id = session.workspace_id
        AND membership.user_id = ?
        AND membership.deleted_at IS NULL
      LIMIT 1
    ) AS assigned_workspace_role,
    (
      SELECT value_json
      FROM app_settings
      WHERE id = 'cloudsync_workspace_binding'
      LIMIT 1
    ) AS binding_json
  FROM sessions AS session
  LEFT JOIN session_documents AS note
    ON note.id = COALESCE(
      (
        SELECT canonical.id
        FROM session_documents AS canonical
        WHERE canonical.id = session.id
          AND canonical.session_id = session.id
          AND canonical.kind = 'note'
          AND canonical.deleted_at IS NULL
        LIMIT 1
      ),
      (
        SELECT fallback.id
        FROM session_documents AS fallback
        WHERE fallback.session_id = session.id
          AND fallback.kind = 'note'
          AND fallback.deleted_at IS NULL
        ORDER BY fallback.updated_at DESC, fallback.created_at DESC, fallback.id
        LIMIT 1
      )
    )
  WHERE session.id = ?
    AND session.deleted_at IS NULL
  LIMIT 1
`;

const AVAILABLE_SHARE_WORKSPACES_SQL = `
  SELECT workspace.id, workspace.name
  FROM workspaces AS workspace
  JOIN workspace_memberships AS membership
    ON membership.workspace_id = workspace.id
    AND membership.user_id = ?
    AND membership.deleted_at IS NULL
  WHERE workspace.kind = 'shared'
    AND workspace.deleted_at IS NULL
  ORDER BY workspace.name COLLATE NOCASE, workspace.id
`;

export async function loadSessionShareSource(
  sessionId: string,
  accountUserId: string,
): Promise<SessionShareSource> {
  const normalizedSessionId = requireIdentifier(sessionId, "session");
  const normalizedAccountUserId = requireIdentifier(
    accountUserId,
    "account user",
  );
  if (normalizedAccountUserId === DEFAULT_USER_ID) {
    throw new Error("A signed-in account is required to share a note");
  }

  await flushDatabaseWrites([`session:${normalizedSessionId}`]);
  const [row] = await liveQueryClient.execute<SessionShareSourceSqlRow>(
    SESSION_SHARE_SOURCE_SQL,
    [
      normalizedAccountUserId,
      normalizedAccountUserId,
      normalizedAccountUserId,
      normalizedAccountUserId,
      normalizedSessionId,
    ],
  );
  if (!row) {
    throw new Error("The note is unavailable for sharing");
  }

  return {
    sessionId: row.id,
    documentId: row.document_id,
    workspaceId: resolveSourceWorkspace(row, normalizedAccountUserId),
    title: row.title,
    body: parseShareDocument(row.body, row.body_format),
    rawBody: row.body,
    bodyFormat: row.body_format,
  };
}

export function useAvailableShareWorkspaces(
  accountUserId: string | null | undefined,
): AvailableShareWorkspace[] {
  const normalizedAccountUserId = accountUserId?.trim() ?? "";
  const enabled = Boolean(
    normalizedAccountUserId && normalizedAccountUserId !== DEFAULT_USER_ID,
  );
  const { data = EMPTY_WORKSPACES } = useLiveQuery<
    AvailableShareWorkspaceSqlRow,
    AvailableShareWorkspace[]
  >({
    sql: AVAILABLE_SHARE_WORKSPACES_SQL,
    params: [normalizedAccountUserId],
    enabled,
    mapRows: (rows) =>
      rows.map((row) => ({
        id: row.id,
        name: row.name,
      })),
  });

  return enabled ? data : EMPTY_WORKSPACES;
}

function resolveSourceWorkspace(
  row: SessionShareSourceSqlRow,
  accountUserId: string,
): string {
  const personalWorkspaceAvailable = Boolean(row.personal_workspace_available);
  const assignedWorkspaceId = row.workspace_id.trim();

  if (assignedWorkspaceId === accountUserId) {
    if (
      personalWorkspaceAvailable &&
      row.assigned_workspace_kind === "personal" &&
      row.assigned_workspace_role === "owner"
    ) {
      return accountUserId;
    }
    if (
      !row.assigned_workspace_kind &&
      isLegacyWorkspaceBinding(
        assignedWorkspaceId,
        row.binding_json,
        accountUserId,
      )
    ) {
      return accountUserId;
    }
    throw new Error("The personal workspace is unavailable for sharing");
  }

  if (row.assigned_workspace_kind === "shared") {
    if (
      row.assigned_workspace_deleted_at === null &&
      (row.assigned_workspace_role === "owner" ||
        row.assigned_workspace_role === "admin")
    ) {
      return assignedWorkspaceId;
    }
    throw new Error("You can no longer share notes from this workspace");
  }

  if (row.assigned_workspace_kind) {
    throw new Error("The note belongs to an unavailable workspace");
  }

  if (
    isLegacyWorkspaceBinding(
      assignedWorkspaceId,
      row.binding_json,
      accountUserId,
    )
  ) {
    if (!personalWorkspaceAvailable) {
      throw new Error("The personal workspace is unavailable for sharing");
    }
    return accountUserId;
  }

  throw new Error("The note belongs to an unavailable workspace");
}

function isLegacyWorkspaceBinding(
  assignedWorkspaceId: string,
  bindingJson: string | null,
  accountUserId: string,
): boolean {
  if (!assignedWorkspaceId || assignedWorkspaceId === DEFAULT_USER_ID) {
    return true;
  }
  if (!bindingJson) return false;

  let value: unknown;
  try {
    value = JSON.parse(bindingJson) as unknown;
  } catch {
    return false;
  }
  if (!isRecord(value)) return false;

  const workspaceId = value.workspace_id;
  const boundAccountUserId = value.account_user_id;
  if (
    typeof workspaceId !== "string" ||
    workspaceId.trim() !== assignedWorkspaceId
  ) {
    return false;
  }
  return (
    boundAccountUserId == null ||
    boundAccountUserId === "" ||
    boundAccountUserId === accountUserId
  );
}

function parseShareDocument(body: string, bodyFormat: string): JSONContent {
  if (!body.trim()) return EMPTY_DOCUMENT;

  let parsed: unknown;
  if (bodyFormat === "markdown") {
    parsed = md2json(body);
  } else if (bodyFormat === "prosemirror_json") {
    try {
      parsed = JSON.parse(body) as unknown;
    } catch {
      throw new Error("The note content is malformed and cannot be shared");
    }
  } else {
    throw new Error("The note content format cannot be shared");
  }

  if (!isValidDocument(parsed)) {
    throw new Error("The note content is malformed and cannot be shared");
  }
  return parsed;
}

function isValidDocument(value: unknown): value is JSONContent {
  if (
    !isRecord(value) ||
    value.type !== "doc" ||
    !Array.isArray(value.content)
  ) {
    return false;
  }

  const budget = { nodes: 0 };
  return value.content.every((node) => isValidNode(node, 1, budget));
}

function isValidNode(
  value: unknown,
  depth: number,
  budget: { nodes: number },
): value is JSONContent {
  budget.nodes += 1;
  if (depth > MAX_DOCUMENT_DEPTH || budget.nodes > MAX_DOCUMENT_NODES) {
    return false;
  }
  if (!isRecord(value) || typeof value.type !== "string" || !value.type) {
    return false;
  }
  if (value.text !== undefined && typeof value.text !== "string") {
    return false;
  }
  if (value.attrs !== undefined && !isRecord(value.attrs)) {
    return false;
  }
  if (
    value.marks !== undefined &&
    (!Array.isArray(value.marks) ||
      !value.marks.every(
        (mark) =>
          isRecord(mark) &&
          typeof mark.type === "string" &&
          Boolean(mark.type) &&
          (mark.attrs === undefined || isRecord(mark.attrs)),
      ))
  ) {
    return false;
  }
  if (value.content === undefined) return true;
  return (
    Array.isArray(value.content) &&
    value.content.every((node) => isValidNode(node, depth + 1, budget))
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function requireIdentifier(value: string, label: string): string {
  const normalized = value.trim();
  if (!normalized) throw new Error(`${label} is required`);
  return normalized;
}
