import { md2json, parseJsonContent } from "@anlg/editor/markdown";
import type { JSONContent } from "@anlg/editor/note";

import {
  createEmptyWorkflow,
  parseAutomationWorkflows,
  saveAutomationWorkflows,
  type AutomationWorkflow,
  type WorkflowStep,
} from "~/automations/workflows";
import { liveQueryClient } from "~/db";
import {
  createNamedFolder,
  updateFolderInstructions,
} from "~/session/folder-catalog";
import { folderDisplayName, normalizeFolderPath } from "~/session/folders";
import { createSession } from "~/session/queries/creation";
import { getStoredSettingValues } from "~/settings/queries";
import {
  assertCanonicalTemplateSections,
  assertCanonicalTemplateTargets,
} from "~/templates/codec";
import type { UserTemplate, UserTemplateDraft } from "~/templates/queries";
import { normalizeTemplateIcon } from "~/templates/template-icon";

const MAX_SHARED_PAYLOAD_BYTES = 1_900_000;

type SharedFolderNote = {
  title: string;
  relativeFolderPath: string;
  body: JSONContent;
};

export type SharedFolderPayload = {
  version: 1;
  path: string;
  instructions: string;
  notes: SharedFolderNote[];
};

export type SharedTemplatePayload = {
  version: 1;
  template: UserTemplateDraft;
};

export type SharedAutomationPayload = {
  version: 1;
  workflow: AutomationWorkflow;
};

type FolderShareRow = {
  title: string;
  folder_path: string;
  body: string;
  body_format: string;
};

export async function sharedFolderPayload(
  folderPath: string,
): Promise<Record<string, unknown>> {
  const path = requireFolderPath(folderPath);
  const [folderRow, notes] = await Promise.all([
    liveQueryClient.execute<{ instructions: string }>(
      `
        SELECT instructions
        FROM folders
        WHERE path = ? AND deleted_at IS NULL
        LIMIT 1
      `,
      [path],
    ),
    liveQueryClient.execute<FolderShareRow>(
      `
        SELECT
          session.title,
          session.folder_path,
          COALESCE(enhanced.body, note.body, '') AS body,
          COALESCE(
            enhanced.body_format,
            note.body_format,
            'prosemirror_json'
          ) AS body_format
        FROM sessions AS session
        LEFT JOIN session_documents AS enhanced
          ON enhanced.id = (
            SELECT candidate.id
            FROM session_documents AS candidate
            WHERE candidate.session_id = session.id
              AND candidate.kind IN ('summary', 'template_output')
              AND candidate.deleted_at IS NULL
            ORDER BY candidate.sort_order, candidate.id
            LIMIT 1
          )
        LEFT JOIN session_documents AS note
          ON note.id = (
            SELECT candidate.id
            FROM session_documents AS candidate
            WHERE candidate.session_id = session.id
              AND candidate.kind = 'note'
              AND candidate.deleted_at IS NULL
            ORDER BY candidate.sort_order, candidate.id
            LIMIT 1
          )
        WHERE session.deleted_at IS NULL
          AND (
            session.folder_path = ?
            OR session.folder_path LIKE ?
            OR session.folder_path LIKE ?
          )
        ORDER BY session.created_at, session.id
      `,
      [path, `${path}/%`, `${path}\\%`],
    ),
  ]);

  const payload: SharedFolderPayload = {
    version: 1,
    path: folderDisplayName(path),
    instructions: folderRow[0]?.instructions ?? "",
    notes: notes.map((note) => ({
      title: note.title,
      relativeFolderPath: relativeFolderPath(path, note.folder_path),
      body:
        note.body_format === "markdown"
          ? md2json(note.body)
          : parseJsonContent(note.body),
    })),
  };
  assertPayloadSize(payload);
  return payload as unknown as Record<string, unknown>;
}

export function sharedTemplatePayload(
  template: UserTemplate,
): Record<string, unknown> {
  const payload: SharedTemplatePayload = {
    version: 1,
    template: {
      title: template.title,
      description: template.description,
      category: template.category,
      icon: template.icon,
      targets: template.targets,
      sections: template.sections,
    },
  };
  assertPayloadSize(payload);
  return payload as unknown as Record<string, unknown>;
}

export function sharedAutomationPayload(
  workflow: AutomationWorkflow,
): Record<string, unknown> {
  const payload: SharedAutomationPayload = {
    version: 1,
    workflow: {
      ...workflow,
      enabled: false,
      steps: workflow.steps.map(clearAutomationDestination),
      lastRun: null,
      processedSessionIds: [],
      chatGroupId: null,
    },
  };
  assertPayloadSize(payload);
  return payload as unknown as Record<string, unknown>;
}

export function parseSharedFolderPayload(
  value: Record<string, unknown>,
): SharedFolderPayload {
  if (
    value.version !== 1 ||
    typeof value.path !== "string" ||
    typeof value.instructions !== "string" ||
    !Array.isArray(value.notes)
  ) {
    throw new Error("This shared folder is invalid");
  }
  const path = requireFolderPath(value.path);
  const notes = value.notes.map((note) => {
    if (!isRecord(note) || typeof note.title !== "string") {
      throw new Error("This shared folder is invalid");
    }
    const relativeFolderPath = requireRelativeFolderPath(
      note.relativeFolderPath,
    );
    const body = parseSharedDocument(note.body);
    return { title: note.title, relativeFolderPath, body };
  });
  return { version: 1, path, instructions: value.instructions, notes };
}

export function parseSharedTemplatePayload(
  value: Record<string, unknown>,
): UserTemplateDraft {
  const template = value.template;
  if (
    value.version !== 1 ||
    !isRecord(template) ||
    typeof template.title !== "string" ||
    typeof template.description !== "string"
  ) {
    throw new Error("This shared template is invalid");
  }
  return {
    title: template.title,
    description: template.description,
    category:
      typeof template.category === "string" ? template.category : undefined,
    icon: normalizeTemplateIcon(template.icon),
    targets: assertCanonicalTemplateTargets(
      template.targets,
      "shared template targets",
    ),
    sections: assertCanonicalTemplateSections(
      template.sections,
      "shared template sections",
    ),
  };
}

export function parseSharedAutomationPayload(
  value: Record<string, unknown>,
): AutomationWorkflow {
  if (value.version !== 1 || !isRecord(value.workflow)) {
    throw new Error("This shared automation is invalid");
  }
  const workflow = parseAutomationWorkflows(
    JSON.stringify([value.workflow]),
  )[0];
  if (!workflow) {
    throw new Error("This shared automation is invalid");
  }
  return {
    ...workflow,
    enabled: false,
    steps: workflow.steps.map(clearAutomationDestination),
    lastRun: null,
    processedSessionIds: [],
    chatGroupId: null,
  };
}

export async function importSharedFolder(
  payload: SharedFolderPayload,
  userId: string,
): Promise<string> {
  const root = await availableFolderPath(payload.path);
  await createNamedFolder(root);
  if (payload.instructions) {
    await updateFolderInstructions(root, payload.instructions);
  }
  for (const note of payload.notes) {
    const folderPath = note.relativeFolderPath
      ? `${root}/${note.relativeFolderPath}`
      : root;
    await createSession(note.title, userId, {
      folder_id: folderPath,
      raw_md: JSON.stringify(note.body),
    });
  }
  return root;
}

export async function importSharedAutomation(
  payload: SharedAutomationPayload,
): Promise<string> {
  const stored = await getStoredSettingValues();
  const workflows = parseAutomationWorkflows(
    stored.values.automation_workflows,
  );
  const workflow = createEmptyWorkflow({
    ...payload.workflow,
    id: undefined,
    enabled: false,
    steps: payload.workflow.steps.map(clearAutomationDestination),
    lastRun: null,
    processedSessionIds: [],
    chatGroupId: null,
  });
  await saveAutomationWorkflows([workflow, ...workflows]);
  return workflow.id;
}

function clearAutomationDestination(step: WorkflowStep): WorkflowStep {
  if (step.type === "markdown_export") {
    return { ...step, directory: "" };
  }
  return { ...step, target: null };
}

function relativeFolderPath(root: string, child: string): string {
  const normalized = normalizeFolderPath(child);
  if (!normalized || normalized === root) return "";
  const prefix = `${root}/`;
  return normalized.startsWith(prefix) ? normalized.slice(prefix.length) : "";
}

function requireFolderPath(value: string): string {
  const normalized = normalizeFolderPath(value);
  if (!normalized) throw new Error("This shared folder is invalid");
  return normalized;
}

function requireRelativeFolderPath(value: unknown): string {
  if (value === "" || value === undefined) return "";
  if (typeof value !== "string") {
    throw new Error("This shared folder is invalid");
  }
  const normalized = normalizeFolderPath(value);
  if (!normalized) throw new Error("This shared folder is invalid");
  return normalized;
}

function parseSharedDocument(value: unknown): JSONContent {
  if (
    !isRecord(value) ||
    value.type !== "doc" ||
    !Array.isArray(value.content)
  ) {
    throw new Error("This shared folder is invalid");
  }
  return value as JSONContent;
}

async function availableFolderPath(base: string): Promise<string> {
  const rows = await liveQueryClient.execute<{ path: string }>(
    `
      SELECT path FROM folders WHERE deleted_at IS NULL
      UNION
      SELECT folder_path AS path
      FROM sessions
      WHERE deleted_at IS NULL AND folder_path <> ''
    `,
  );
  const occupied = new Set(rows.map((row) => row.path.toLowerCase()));
  if (!occupied.has(base.toLowerCase())) return base;
  for (let suffix = 2; suffix < 1_000; suffix += 1) {
    const candidate = `${base} (${suffix})`;
    if (!occupied.has(candidate.toLowerCase())) return candidate;
  }
  throw new Error("Could not create a copy of this folder");
}

function assertPayloadSize(value: object): void {
  if (
    new TextEncoder().encode(JSON.stringify(value)).byteLength >
    MAX_SHARED_PAYLOAD_BYTES
  ) {
    throw new Error("This item is too large to share");
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
