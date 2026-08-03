import type { UIMessage } from "ai";
import { z } from "zod";

import { CONTEXT_ENTITY_SOURCES } from "~/chat/context/entities";
import type { ContextRef } from "~/chat/context/entities";

export type ChatScope = "general" | "automations";

const messageMetadataSchema = z.object({
  chatScope: z.enum(["general", "automations"]).optional(),
  createdAt: z.number().optional(),
  contextRefs: z
    .array(
      z.discriminatedUnion("kind", [
        z.object({
          kind: z.literal("session"),
          key: z.string(),
          source: z.enum(CONTEXT_ENTITY_SOURCES).optional(),
          sessionId: z.string(),
        }),
        z.object({
          kind: z.literal("human"),
          key: z.string(),
          source: z.enum(CONTEXT_ENTITY_SOURCES).optional(),
          humanId: z.string(),
        }),
        z.object({
          kind: z.literal("organization"),
          key: z.string(),
          source: z.enum(CONTEXT_ENTITY_SOURCES).optional(),
          organizationId: z.string(),
        }),
      ]),
    )
    .optional(),
});

type MessageMetadata = z.infer<typeof messageMetadataSchema>;
export type AnlgUIMessage = UIMessage<
  MessageMetadata & { contextRefs?: ContextRef[] }
>;

export type ChatSendOptions = {
  chatGroupId?: string;
  beforeSend?: (
    trackCompletion: (completion: Promise<unknown>) => void,
  ) => void | Promise<void>;
};

export type ChatMessageSender = (
  message: AnlgUIMessage,
  options?: ChatSendOptions,
) => void;
