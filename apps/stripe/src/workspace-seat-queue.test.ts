import { afterAll, describe, expect, test } from "bun:test";
import pg from "pg";

import { processWorkspaceSeatEvent } from "./workspace-seat-queue";

const url = process.env.SEAT_BILLING_TEST_DATABASE_URL;
if (
  url &&
  !["127.0.0.1", "localhost", "[::1]"].includes(new URL(url).hostname)
) {
  throw new Error("Seat queue tests require an isolated local database");
}
const pool = url ? new pg.Pool({ connectionString: url, max: 4 }) : undefined;
afterAll(async () => {
  await pool?.end();
});

describe.skipIf(!pool)("durable seat queue (isolated Postgres)", () => {
  test("concurrent workers cannot overtake a workspace event; failures persist and retry in order", async () => {
    const workspace = crypto.randomUUID();
    const another = crypto.randomUUID();
    const seen: number[] = [];
    const errors: unknown[] = [];
    try {
      await pool!.query(
        `INSERT INTO private.workspace_seat_billing_events (workspace_id, customer_id, quantity)
        VALUES ($1, 'cus_test', 2), ($1, 'cus_test', 3), ($2, 'cus_other', 1)`,
        [workspace, another],
      );
      let release!: () => void;
      let acquired!: () => void;
      const barrier = new Promise<void>((resolve) => {
        release = resolve;
      });
      const started = new Promise<void>((resolve) => {
        acquired = resolve;
      });
      const first = processWorkspaceSeatEvent(
        pool!,
        async (event) => {
          expect(event.workspace_id).toBe(workspace);
          seen.push(event.quantity);
          acquired();
          await barrier;
          throw new Error("Stripe unavailable");
        },
        (error) => errors.push(error),
      );
      await started;
      try {
        // The other workspace can proceed, but quantity 3 must wait behind 2.
        await processWorkspaceSeatEvent(
          pool!,
          async (event) => {
            expect(event.workspace_id).toBe(another);
          },
          (error) => errors.push(error),
        );
      } finally {
        release();
      }
      await first;
      expect(errors).toHaveLength(1);
      expect(
        await processWorkspaceSeatEvent(
          pool!,
          async () => {
            throw new Error("overtook retry");
          },
          (error) => errors.push(error),
        ),
      ).toBe(false);
      const state = await pool!.query(
        `SELECT attempts, processed_at, next_attempt_at > now() AS backed_off
        FROM private.workspace_seat_billing_events WHERE workspace_id = $1 ORDER BY id`,
        [workspace],
      );
      expect(state.rows[0]).toMatchObject({
        attempts: 1,
        processed_at: null,
        backed_off: true,
      });
      await pool!.query(
        "UPDATE private.workspace_seat_billing_events SET next_attempt_at = now() WHERE workspace_id = $1",
        [workspace],
      );
      const consume = async () =>
        processWorkspaceSeatEvent(
          pool!,
          async (event) => {
            seen.push(event.quantity);
          },
          (error) => errors.push(error),
        );
      await consume();
      await consume();
      expect(seen).toEqual([2, 2, 3]);
      expect(await consume()).toBe(false);
      expect(errors).toHaveLength(1);
    } finally {
      await pool!.query(
        "DELETE FROM private.workspace_seat_billing_events WHERE workspace_id = ANY($1::uuid[])",
        [[workspace, another]],
      );
    }
  });
  test("concurrent member additions capture successive quantities", async () => {
    const suffix = crypto.randomUUID();
    const names = ["owner", "first", "second"].map(
      (name) => `${name}-${suffix}`,
    );
    const setup = await pool!.connect();
    let workspace: string | undefined;
    let users: string[] = [];
    try {
      users = names.map(() => crypto.randomUUID());
      for (let index = 0; index < users.length; index++) {
        await setup.query(
          "INSERT INTO auth.users (id, email, email_confirmed_at, created_at, updated_at) VALUES ($1, $2, now(), now(), now())",
          [users[index], `${names[index]}@example.com`],
        );
      }
      await setup.query(
        "SELECT set_config('request.jwt.claim.sub', $1, false)",
        [users[0]],
      );
      await setup.query("SET ROLE authenticated");
      const created = await setup.query(
        "SELECT workspace_id FROM public.create_workspace('Concurrent billing test')",
      );
      workspace = created.rows[0].workspace_id;
      await setup.query("RESET ROLE");
      await setup.query(
        "UPDATE public.workspaces SET stripe_customer_id = $2, seat_limit = 1 WHERE id = $1",
        [workspace, `cus_concurrency_${suffix}`],
      );
      await Promise.all(
        users
          .slice(1)
          .map((user) =>
            pool!.query(
              "INSERT INTO public.workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
              [workspace, user],
            ),
          ),
      );
      const events = await setup.query(
        "SELECT quantity FROM private.workspace_seat_billing_events WHERE workspace_id = $1 ORDER BY id",
        [workspace],
      );
      expect(events.rows.map((row) => row.quantity)).toEqual([1, 2, 3]);
    } finally {
      await setup.query("RESET ROLE");
      await setup.query(
        "SELECT set_config('request.jwt.claim.sub', '', false)",
      );
      if (workspace) {
        await setup.query("DELETE FROM public.workspaces WHERE id = $1", [
          workspace,
        ]);
        await setup.query(
          "DELETE FROM private.workspace_seat_billing_events WHERE workspace_id = $1",
          [workspace],
        );
      }
      await setup.query(
        "DELETE FROM public.workspaces WHERE owner_user_id = ANY($1::uuid[])",
        [users],
      );
      await setup.query("DELETE FROM auth.users WHERE id = ANY($1::uuid[])", [
        users,
      ]);
      setup.release();
    }
  });
});
