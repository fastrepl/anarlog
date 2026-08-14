import type {
  DeliveryItem,
  PendingAcknowledgement,
  PendingCompletion,
} from "./types";

import { executeTransaction, liveQueryClient } from "~/db";

export async function loadDeliveryCursor(input: {
  serverUrl: string;
  workspaceId: string;
  consumerId: string;
}): Promise<number> {
  const rows = await liveQueryClient.execute<{ cursor: number }>(
    `
      SELECT cursor
      FROM enterprise_session_delivery_state
      WHERE server_url = ? AND workspace_id = ? AND consumer_id = ?
    `,
    [input.serverUrl, input.workspaceId, input.consumerId],
  );
  return rows[0]?.cursor ?? 0;
}

export async function recordAppliedDelivery(input: {
  serverUrl: string;
  workspaceId: string;
  consumerId: string;
  item: DeliveryItem;
}): Promise<void> {
  await recordDelivery(input, true);
}

export async function recordRejectedDelivery(input: {
  serverUrl: string;
  workspaceId: string;
  consumerId: string;
  item: DeliveryItem;
}): Promise<void> {
  await recordDelivery(input, false);
}

async function recordDelivery(
  input: {
    serverUrl: string;
    workspaceId: string;
    consumerId: string;
    item: DeliveryItem;
  },
  enqueueCompletion: boolean,
): Promise<void> {
  const now = new Date().toISOString();
  const statements = [
    {
      sql: `
        INSERT INTO enterprise_session_delivery_state (
          server_url, workspace_id, consumer_id, cursor, updated_at
        ) VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(server_url, workspace_id, consumer_id) DO UPDATE SET
          cursor = MAX(cursor, excluded.cursor),
          updated_at = excluded.updated_at
      `,
      params: [
        input.serverUrl,
        input.workspaceId,
        input.consumerId,
        input.item.cursor,
        now,
      ],
    },
    {
      sql: `
        INSERT INTO enterprise_session_delivery_receipts (
          server_url, workspace_id, consumer_id, job_id, revision,
          content_hash, acknowledged_at, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, NULL, ?, ?)
        ON CONFLICT(server_url, workspace_id, consumer_id, job_id, revision)
        DO UPDATE SET updated_at = excluded.updated_at
      `,
      params: [
        input.serverUrl,
        input.workspaceId,
        input.consumerId,
        input.item.jobId,
        input.item.revision,
        input.item.contentHash,
        now,
        now,
      ],
    },
  ];
  if (enqueueCompletion && input.item.finalized) {
    statements.push({
      sql: `
        INSERT INTO enterprise_session_completion_outbox (
          source_id, workspace_id, session_id, revision, created_at, dispatched_at
        ) VALUES (?, ?, ?, ?, ?, NULL)
        ON CONFLICT(source_id) DO NOTHING
      `,
      params: [
        input.item.envelope.source_id,
        input.workspaceId,
        input.item.envelope.session.id,
        input.item.revision,
        now,
      ],
    });
  }
  await executeTransaction(statements);
}

export async function listPendingAcknowledgements(input: {
  serverUrl: string;
  workspaceId: string;
  consumerId: string;
}): Promise<PendingAcknowledgement[]> {
  return liveQueryClient
    .execute<{
      job_id: string;
      revision: number;
      content_hash: string;
    }>(
      `
      SELECT job_id, revision, content_hash
      FROM enterprise_session_delivery_receipts
      WHERE server_url = ? AND workspace_id = ? AND consumer_id = ?
        AND acknowledged_at IS NULL
      ORDER BY revision, job_id
    `,
      [input.serverUrl, input.workspaceId, input.consumerId],
    )
    .then((rows) =>
      rows.map((row) => ({
        jobId: row.job_id,
        revision: row.revision,
        contentHash: row.content_hash,
      })),
    );
}

export async function markDeliveryAcknowledged(input: {
  serverUrl: string;
  workspaceId: string;
  consumerId: string;
  jobId: string;
  revision: number;
}): Promise<void> {
  const now = new Date().toISOString();
  await liveQueryClient.execute(
    `
      UPDATE enterprise_session_delivery_receipts
      SET acknowledged_at = ?, updated_at = ?
      WHERE server_url = ? AND workspace_id = ? AND consumer_id = ?
        AND job_id = ? AND revision = ?
    `,
    [
      now,
      now,
      input.serverUrl,
      input.workspaceId,
      input.consumerId,
      input.jobId,
      input.revision,
    ],
  );
}

export async function listPendingCompletions(): Promise<PendingCompletion[]> {
  return liveQueryClient
    .execute<{
      source_id: string;
      workspace_id: string;
      session_id: string;
      revision: number;
    }>(
      `
      SELECT source_id, workspace_id, session_id, revision
      FROM enterprise_session_completion_outbox
      WHERE dispatched_at IS NULL
      ORDER BY created_at, source_id
    `,
    )
    .then((rows) =>
      rows.map((row) => ({
        sourceId: row.source_id,
        workspaceId: row.workspace_id,
        sessionId: row.session_id,
        revision: row.revision,
      })),
    );
}

export async function markCompletionDispatched(
  sourceId: string,
): Promise<void> {
  await liveQueryClient.execute(
    `
      UPDATE enterprise_session_completion_outbox
      SET dispatched_at = ?
      WHERE source_id = ? AND dispatched_at IS NULL
    `,
    [new Date().toISOString(), sourceId],
  );
}
