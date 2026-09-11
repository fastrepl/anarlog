import type pg from "pg";

import type { WorkspaceSeatEvent } from "./workspace-seat-reconciliation";

export async function processWorkspaceSeatEvent(
  pool: Pick<pg.Pool, "connect">,
  reconcile: (event: WorkspaceSeatEvent) => Promise<void>,
  reportError: (error: unknown, event: WorkspaceSeatEvent) => void,
) {
  const client = await pool.connect();
  let event: WorkspaceSeatEvent | undefined;
  try {
    await client.query("BEGIN");
    // Only the oldest unfinished change per workspace is eligible, even when
    // another machine holds its row lock or it is waiting for a retry.
    const result = await client.query<WorkspaceSeatEvent>(`
      SELECT event.id, event.workspace_id, event.customer_id, event.quantity, event.occurred_at
      FROM private.workspace_seat_billing_events AS event
      WHERE event.processed_at IS NULL AND event.next_attempt_at <= now()
        AND NOT EXISTS (
          SELECT 1 FROM private.workspace_seat_billing_events AS earlier
          WHERE earlier.workspace_id = event.workspace_id
            AND earlier.id < event.id AND earlier.processed_at IS NULL
        )
      ORDER BY event.id LIMIT 1 FOR UPDATE OF event SKIP LOCKED
    `);
    event = result.rows[0];
    if (event) {
      try {
        await reconcile(event);
        await client.query(
          "UPDATE private.workspace_seat_billing_events SET processed_at = now() WHERE id = $1",
          [event.id],
        );
      } catch (error) {
        await client.query(
          `UPDATE private.workspace_seat_billing_events
           SET attempts = attempts + 1,
               next_attempt_at = now() + make_interval(secs => LEAST(3600, 15 * power(2, LEAST(attempts, 8))::integer))
           WHERE id = $1`,
          [event.id],
        );
        reportError(error, event);
      }
    }
    await client.query("COMMIT");
    return Boolean(event);
  } catch (error) {
    await client.query("ROLLBACK");
    throw error;
  } finally {
    client.release();
  }
}
