import pg from "pg";

import { env } from "./env";
import { captureOperationalError } from "./error-reporting";
import { stripe } from "./integration/stripe";
import { processWorkspaceSeatEvent } from "./workspace-seat-queue";
import { reconcileWorkspaceSeatEvent } from "./workspace-seat-reconciliation";

export function startWorkspaceSeatWorker() {
  const pool = new pg.Pool({
    connectionString: env.DATABASE_URL,
    max: 1,
    connectionTimeoutMillis: 10_000,
    idleTimeoutMillis: 30_000,
  });
  pool.on("error", (error) => {
    captureOperationalError(error, { operation: "workspace_seat_connection" });
  });
  let stopped = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const run = async () => {
    try {
      for (let count = 0; count < 100 && !stopped; count++) {
        if (
          !(await processWorkspaceSeatEvent(
            pool,
            (event) => reconcileWorkspaceSeatEvent(event, stripe),
            (error, event) =>
              captureOperationalError(error, {
                operation: "workspace_seat_reconciliation",
                context: { event_id: event.id },
              }),
          ))
        )
          break;
      }
    } catch (error) {
      captureOperationalError(error, { operation: "workspace_seat_worker" });
    } finally {
      if (!stopped) timer = setTimeout(run, 15_000);
    }
  };
  void run();
  return async () => {
    stopped = true;
    clearTimeout(timer);
    await pool.end();
  };
}
