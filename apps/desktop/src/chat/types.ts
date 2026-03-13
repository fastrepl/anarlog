import type { UIMessage } from "ai";
import { z } from "zod";

import { CONTEXT_ENTITY_SOURCES } from "~/chat/context/entities";
import type { ContextRef } from "~/chat/context/entities";

const messageMetadataSchema = z.object({
  createdAt: z.number().optional(),
  contextRefs: z
    .array(
      z.object({
        kind: z.literal("session"),
        key: z.string(),
        source: z.enum(CONTEXT_ENTITY_SOURCES).optional(),
        sessionId: z.string(),
      }),
    )
    .optional(),
});

type MessageMetadata = z.infer<typeof messageMetadataSchema>;
export type HyprUIMessage = UIMessage<
  MessageMetadata & { contextRefs?: ContextRef[] }
>;
