import { applySessionIngest } from "@anlg/plugin-db";
import { commands as localApiCommands } from "@anlg/plugin-local-api";

import { acknowledgeSessionDelivery, listSessionDeliveries } from "./client";
import {
  listPendingAcknowledgements,
  listPendingCompletions,
  loadDeliveryCursor,
  markCompletionDispatched,
  markDeliveryAcknowledged,
  recordAppliedDelivery,
  recordRejectedDelivery,
} from "./store";

import { runMeetingCompletedAutomations } from "~/automations/engine";

const MAX_PAGES_PER_SYNC = 100;

export async function syncEnterpriseWorkspace(input: {
  serverUrl: string;
  accessToken: string;
  workspaceId: string;
  consumerId: string;
}): Promise<void> {
  await flushAcknowledgements(input);
  let cursor = await loadDeliveryCursor(input);

  for (let pageIndex = 0; pageIndex < MAX_PAGES_PER_SYNC; pageIndex += 1) {
    const page = await listSessionDeliveries({ ...input, after: cursor });
    if (page.items.length === 0) {
      if (page.hasMore)
        throw new Error("capture delivery cursor did not advance");
      return;
    }

    for (const item of page.items) {
      if (
        item.cursor <= cursor ||
        item.envelope.workspace_id !== input.workspaceId
      ) {
        throw new Error(
          "capture delivery is outside the requested cursor or workspace",
        );
      }
      const result = await applySessionIngest(input.workspaceId, item.envelope);
      if (result === "rejected") {
        await recordRejectedDelivery({ ...input, item });
      } else {
        await recordAppliedDelivery({ ...input, item });
      }
      await acknowledgeSessionDelivery({
        ...input,
        jobId: item.jobId,
        revision: item.revision,
        contentHash: item.contentHash,
      });
      await markDeliveryAcknowledged({
        ...input,
        jobId: item.jobId,
        revision: item.revision,
      });
      cursor = item.cursor;
    }

    if (page.nextCursor !== cursor) {
      throw new Error("capture delivery response has an inconsistent cursor");
    }
    if (!page.hasMore) return;
  }
  throw new Error("capture delivery exceeded the bounded page limit");
}

async function flushAcknowledgements(input: {
  serverUrl: string;
  accessToken: string;
  workspaceId: string;
  consumerId: string;
}): Promise<void> {
  const pending = await listPendingAcknowledgements(input);
  for (const acknowledgement of pending) {
    await acknowledgeSessionDelivery({ ...input, ...acknowledgement });
    await markDeliveryAcknowledged({
      ...input,
      jobId: acknowledgement.jobId,
      revision: acknowledgement.revision,
    });
  }
}

export async function dispatchPendingEnterpriseCompletions(): Promise<void> {
  const pending = await listPendingCompletions();
  for (const completion of pending) {
    const result = await localApiCommands.dispatchEvent(
      "meeting.completed",
      completion.sessionId,
    );
    if (result.status === "error") throw new Error(result.error);
    await runMeetingCompletedAutomations(completion.sessionId);
    await markCompletionDispatched(completion.sourceId);
  }
}
