import { createServerFn } from "@tanstack/react-start";
import { createHash } from "node:crypto";

import { env, requireEnv } from "@/env";
import {
  desktopDownloadPageUrl,
  downloadLinkRequestSchema,
  getDownloadLinkIdempotencyKey,
} from "@/lib/download-link";
import { sendLoopsEvent, sendLoopsTransactional } from "@/lib/loops";

const DOWNLOAD_LINK_TRANSACTIONAL_ID = "REPLACE_WITH_LOOPS_TRANSACTIONAL_ID";

export const sendDesktopDownloadLink = createServerFn({ method: "POST" })
  .inputValidator(downloadLinkRequestSchema)
  .handler(async ({ data }) => {
    const loopsKey = requireEnv(env.LOOPS_KEY, "LOOPS_KEY");
    const requestId = createHash("sha256")
      .update(getDownloadLinkIdempotencyKey(data.email, new Date()))
      .digest("hex");

    await sendLoopsTransactional({
      apiKey: loopsKey,
      transactionalId: DOWNLOAD_LINK_TRANSACTIONAL_ID,
      email: data.email,
      dataVariables: { downloadUrl: desktopDownloadPageUrl },
      idempotencyKey: `desktop-download-link-email:${requestId}`,
    });

    try {
      await sendLoopsEvent({
        apiKey: loopsKey,
        email: data.email,
        eventName: "anarlogDesktopDownloadLinkRequested",
        eventProperties: { source: data.source },
        idempotencyKey: `desktop-download-link-event:${requestId}`,
      });
    } catch (error) {
      console.error("Failed to record desktop download link request:", error);
    }

    return { status: "sent" as const };
  });
