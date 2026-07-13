import type { SessionEvent } from "@hypr/store";

import { executeTransaction, liveQueryClient } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import {
  buildPastSessionNotes,
  buildSessionKeyFactsStatements,
  type PastSessionNotesData,
} from "~/session/insights/past-notes";
import { DEFAULT_USER_ID } from "~/shared/utils";

type SqlStatement = { sql: string; params: unknown[] };
type SeedParticipant = {
  humanId: string;
  name: string;
  email: string;
  jobTitle: string;
};

const CURRENT_SESSION_ID = "devtools-recurring-notes-current";
const SERIES_ID = "devtools-recurring-product-sync";
const CALENDAR_ID = "devtools-calendar";
const MEETING_TITLE = "Devtools Weekly Product Sync";
const DAY_MS = 24 * 60 * 60 * 1000;

const PARTICIPANTS = [
  {
    humanId: "devtools-human-alex-rivera",
    name: "Alex Rivera",
    email: "alex.rivera@example.com",
    jobTitle: "Product Lead",
  },
  {
    humanId: "devtools-human-maya-chen",
    name: "Maya Chen",
    email: "maya.chen@example.com",
    jobTitle: "Design Lead",
  },
  {
    humanId: "devtools-human-jordan-lee",
    name: "Jordan Lee",
    email: "jordan.lee@example.com",
    jobTitle: "Engineering Lead",
  },
] as const;

const PAST_NOTES = [
  {
    sessionId: "devtools-recurring-notes-week-1",
    daysAgo: 7,
    rawMd: [
      "# Product sync",
      "- Shipped the condensed transcript panel and agreed to keep Insights below three visible lines per fact.",
      "- Alex owns the launch checklist and will confirm analytics events before the next review.",
      "- Maya wants another pass on empty states after the first beta feedback lands.",
    ].join("\n"),
    facts: [
      "Transcript controls shipped with a condensed panel layout.",
      "Alex owns the launch checklist and analytics confirmation.",
      "Maya wants another empty-state pass after beta feedback.",
    ],
  },
  {
    sessionId: "devtools-recurring-notes-week-2",
    daysAgo: 14,
    rawMd: [
      "# Product sync",
      "- The team decided Insights should match by recurring calendar series before falling back to participants.",
      "- Jordan called out that cached key facts should avoid requiring a model just to inspect the UI.",
      "- Follow-up: compare date labels against the meeting start time instead of the note creation time.",
    ].join("\n"),
    facts: [
      "Insights should prefer recurring series matches before participant fallback.",
      "Cached key facts should make the UI inspectable without a model.",
      "Date labels should come from meeting start time.",
    ],
  },
  {
    sessionId: "devtools-recurring-notes-week-3",
    daysAgo: 21,
    rawMd: [
      "# Product sync",
      "- We agreed the insights tab should stay hidden until there is useful post-session content.",
      "- The first version of Insights will stay read-only and focus on short reusable facts.",
      "- Alex and Jordan will validate that future sessions are excluded from the timeline.",
    ].join("\n"),
    facts: [
      "The insights tab should stay hidden without useful post-session content.",
      "Insights will start as a read-only timeline of reusable facts.",
      "Future sessions should be excluded from the Insights timeline.",
    ],
  },
] as const;

export async function populateRecurringMeetingNotes({
  userId,
  now = new Date(),
}: {
  userId: string | null | undefined;
  now?: Date;
}): Promise<string> {
  const workspaceId = await loadCloudsyncWorkspaceId();
  const ownerUserId = normalizeUserId(userId, workspaceId);
  const currentSessionId = namespacedId(workspaceId, CURRENT_SESSION_ID);
  const participants = PARTICIPANTS.map((participant) => ({
    ...participant,
    humanId: namespacedId(workspaceId, participant.humanId),
  }));
  const createdAt = now.toISOString();
  const statements: SqlStatement[] = [];
  const data: PastSessionNotesData = {
    sessions: {},
    participants: [],
    enhancedNotes: [],
    keyFacts: {},
  };

  for (const participant of participants) {
    statements.push(
      buildHumanStatement(participant, workspaceId, ownerUserId, createdAt),
    );
  }

  const seeds = [
    {
      sessionId: currentSessionId,
      startedAt: now,
      rawMd:
        "Use the Insights tab to inspect cached facts from previous occurrences.",
    },
    ...PAST_NOTES.map((note) => ({
      sessionId: namespacedId(workspaceId, note.sessionId),
      startedAt: new Date(now.getTime() - note.daysAgo * DAY_MS),
      rawMd: note.rawMd,
    })),
  ];

  for (const seed of seeds) {
    const event = buildSessionEvent(seed.startedAt);
    const eventJson = JSON.stringify(event);
    data.sessions[seed.sessionId] = {
      id: seed.sessionId,
      user_id: ownerUserId,
      title: MEETING_TITLE,
      created_at: seed.startedAt.toISOString(),
      event_json: eventJson,
    };
    data.enhancedNotes.push({
      session_id: seed.sessionId,
      content: seed.rawMd,
      position: 0,
    });
    data.participants.push(
      ...participants.map((participant) => ({
        session_id: seed.sessionId,
        human_id: participant.humanId,
        user_id: ownerUserId,
        source: "auto",
        name: participant.name,
      })),
    );
    statements.push(
      ...buildSessionStatements({
        ownerUserId,
        workspaceId,
        sessionId: seed.sessionId,
        startedAt: seed.startedAt,
        rawMd: seed.rawMd,
        event,
        eventJson,
        participants,
      }),
    );
  }

  const factsBySessionId = new Map<string, string>(
    PAST_NOTES.map((note) => [
      namespacedId(workspaceId, note.sessionId),
      note.facts.join("\n"),
    ]),
  );
  const { missing } = buildPastSessionNotes(
    data,
    currentSessionId,
    ownerUserId,
  );
  statements.push(
    ...buildSessionKeyFactsStatements(
      missing.flatMap((request) => {
        const content = factsBySessionId.get(request.sessionId);
        return content
          ? [
              {
                sessionId: request.sessionId,
                userId: ownerUserId,
                content,
                sourceHash: request.sourceHash,
              },
            ]
          : [];
      }),
      createdAt,
    ),
  );

  await enqueueDatabaseWrite("devtools-recurring-notes", async () => {
    await executeTransaction(statements);
  });

  return currentSessionId;
}

function buildSessionStatements({
  ownerUserId,
  workspaceId,
  sessionId,
  startedAt,
  rawMd,
  event,
  eventJson,
  participants,
}: {
  ownerUserId: string;
  workspaceId: string;
  sessionId: string;
  startedAt: Date;
  rawMd: string;
  event: SessionEvent;
  eventJson: string;
  participants: SeedParticipant[];
}): SqlStatement[] {
  const createdAt = startedAt.toISOString();
  const statements: SqlStatement[] = [
    {
      sql: `
        INSERT INTO sessions (
          id, workspace_id, owner_user_id, title, created_at, updated_at,
          started_at, ended_at, series_id, event_json, deleted_at
        )
        VALUES (
          ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL
        )
        ON CONFLICT(id) DO UPDATE SET
          owner_user_id = excluded.owner_user_id,
          title = excluded.title,
          created_at = excluded.created_at,
          updated_at = excluded.updated_at,
          started_at = excluded.started_at,
          ended_at = excluded.ended_at,
          series_id = excluded.series_id,
          event_json = excluded.event_json,
          deleted_at = NULL
      `,
      params: [
        sessionId,
        workspaceId,
        ownerUserId,
        MEETING_TITLE,
        createdAt,
        createdAt,
        event.started_at,
        event.ended_at,
        SERIES_ID,
        eventJson,
      ],
    },
    buildDocumentStatement({
      id: sessionId,
      sessionId,
      kind: "note",
      title: "",
      body: rawMd,
      ownerUserId,
      createdAt,
    }),
    buildDocumentStatement({
      id: `${sessionId}:summary`,
      sessionId,
      kind: "enhanced_note",
      title: "Summary",
      body: rawMd,
      ownerUserId,
      createdAt,
    }),
  ];

  for (const participant of participants) {
    statements.push({
      sql: `
        INSERT INTO session_participants (
          id, workspace_id, owner_user_id, session_id, human_id, display_name,
          email, source, created_at, updated_at, deleted_at
        )
        SELECT ?, session.workspace_id, ?, session.id, ?, ?, ?, 'auto', ?, ?, NULL
        FROM sessions AS session
        WHERE session.id = ? AND session.deleted_at IS NULL
        ON CONFLICT(id) DO UPDATE SET
          owner_user_id = excluded.owner_user_id,
          session_id = excluded.session_id,
          human_id = excluded.human_id,
          display_name = excluded.display_name,
          email = excluded.email,
          source = excluded.source,
          updated_at = excluded.updated_at,
          deleted_at = NULL
      `,
      params: [
        `${sessionId}:${participant.humanId}`,
        ownerUserId,
        participant.humanId,
        participant.name,
        participant.email,
        createdAt,
        createdAt,
        sessionId,
      ],
    });
  }

  return statements;
}

function buildHumanStatement(
  participant: SeedParticipant,
  workspaceId: string,
  ownerUserId: string,
  now: string,
): SqlStatement {
  return {
    sql: `
      INSERT INTO humans (
        id, workspace_id, owner_user_id, name, email, job_title, created_at,
        updated_at, deleted_at
      )
      VALUES (
        ?, ?, ?, ?, ?, ?, ?, ?, NULL
      )
      ON CONFLICT(id) DO UPDATE SET
        owner_user_id = excluded.owner_user_id,
        name = excluded.name,
        email = excluded.email,
        job_title = excluded.job_title,
        updated_at = excluded.updated_at,
        deleted_at = NULL
    `,
    params: [
      participant.humanId,
      workspaceId,
      ownerUserId,
      participant.name,
      participant.email,
      participant.jobTitle,
      now,
      now,
    ],
  };
}

function buildDocumentStatement({
  id,
  sessionId,
  kind,
  title,
  body,
  ownerUserId,
  createdAt,
}: {
  id: string;
  sessionId: string;
  kind: "note" | "enhanced_note";
  title: string;
  body: string;
  ownerUserId: string;
  createdAt: string;
}): SqlStatement {
  return {
    sql: `
      INSERT INTO session_documents (
        id, workspace_id, session_id, kind, title, body_format, body,
        sort_order, created_by, updated_by, created_at, updated_at, deleted_at
      )
      SELECT ?, session.workspace_id, session.id, ?, ?, 'markdown', ?, 0, ?,
        ?, ?, ?, NULL
      FROM sessions AS session
      WHERE session.id = ? AND session.deleted_at IS NULL
      ON CONFLICT(id) DO UPDATE SET
        session_id = excluded.session_id,
        kind = excluded.kind,
        title = excluded.title,
        body_format = excluded.body_format,
        body = excluded.body,
        sort_order = excluded.sort_order,
        updated_by = excluded.updated_by,
        updated_at = excluded.updated_at,
        deleted_at = NULL
    `,
    params: [
      id,
      kind,
      title,
      body,
      ownerUserId,
      ownerUserId,
      createdAt,
      createdAt,
      sessionId,
    ],
  };
}

function buildSessionEvent(startedAt: Date): SessionEvent {
  const endedAt = new Date(startedAt.getTime() + 45 * 60 * 1000);
  return {
    tracking_id: `${SERIES_ID}:${toDateId(startedAt)}`,
    calendar_id: CALENDAR_ID,
    title: MEETING_TITLE,
    started_at: startedAt.toISOString(),
    ended_at: endedAt.toISOString(),
    is_all_day: false,
    has_recurrence_rules: true,
    meeting_link: "https://zoom.us/j/1234567890",
    description: "Seeded from devtools to exercise the Insights tab.",
    recurrence_series_id: SERIES_ID,
  };
}

async function loadCloudsyncWorkspaceId(): Promise<string> {
  const [binding] = await liveQueryClient.execute<{
    workspace_id: string | null;
  }>(
    `
      SELECT NULLIF(json_extract(value_json, '$.workspace_id'), '') AS workspace_id
      FROM app_settings
      WHERE id = 'cloudsync_workspace_binding'
      LIMIT 1
    `,
  );
  if (!binding?.workspace_id) {
    throw new Error("CloudSync workspace binding is missing");
  }
  return binding.workspace_id;
}

function normalizeUserId(
  userId: string | null | undefined,
  workspaceId: string,
): string {
  const normalized = userId?.trim();
  return normalized && normalized !== DEFAULT_USER_ID
    ? normalized
    : workspaceId;
}

function namespacedId(workspaceId: string, value: string): string {
  return `${workspaceId}:${value}`;
}

function toDateId(date: Date): string {
  return date.toISOString().slice(0, 10);
}
