import { z } from "zod";

export const MEETING_SCHEMA_VERSION = "2";
export const MEETING_PROMPT_VERSION = "3";
export const CLOUDFLARE_MEETING_MODELS = [
  "@cf/meta/llama-3.3-70b-instruct-fp8-fast",
  "@cf/qwen/qwen3-30b-a3b-fp8",
  "@cf/qwen/qwen3.8-27b",
] as const;
export const CLOUDFLARE_JSON_MODELS = [
  "@cf/meta/llama-3.3-70b-instruct-fp8-fast",
] as const;

export const TranscriptSegmentSchema = z
  .object({
    id: z.string().min(1).max(128),
    speaker: z.string().max(160).nullable(),
    text: z.string().min(1).max(8_000),
    start_ms: z.number().int().nonnegative().safe(),
    end_ms: z.number().int().nonnegative().safe(),
  })
  .strict()
  .refine(({ start_ms, end_ms }) => end_ms >= start_ms, {
    message: "Segment end must not precede its start.",
  });

export const MeetingMinutesRequestSchema = z
  .object({
    model: z.enum(CLOUDFLARE_MEETING_MODELS),
    meeting_date: z.string().date().nullable(),
    timezone: z.string().min(1).max(80),
    language: z.string().min(2).max(40),
    transcript_revision: z.string().min(1).max(128),
    notes: z.string().max(20_000).default(""),
    segments: z.array(TranscriptSegmentSchema).min(1).max(2_000),
  })
  .strict()
  .superRefine(({ segments }, context) => {
    const ids = new Set<string>();
    for (const [index, segment] of segments.entries()) {
      if (ids.has(segment.id))
        context.addIssue({
          code: "custom",
          path: ["segments", index, "id"],
          message: "Segment IDs must be unique.",
        });
      ids.add(segment.id);
    }
  });

const EvidenceIdsSchema = z
  .array(z.string().min(1).max(128))
  .min(1)
  .max(20)
  .superRefine((ids, context) => {
    if (new Set(ids).size !== ids.length)
      context.addIssue({
        code: "custom",
        message: "Evidence segment IDs must be unique.",
      });
  });
const EvidenceQuotesSchema = z
  .array(
    z
      .object({
        segment_id: z.string().min(1).max(128),
        quote: z.string().min(1).max(1_000),
      })
      .strict(),
  )
  .min(1)
  .max(20)
  .superRefine((quotes, context) => {
    const ids = quotes.map(({ segment_id }) => segment_id);
    if (new Set(ids).size !== ids.length)
      context.addIssue({
        code: "custom",
        message: "Evidence quotes must reference unique segments.",
      });
  });

export const DecisionSchema = z
  .object({
    id: z.string().min(1).max(64),
    text: z.string().min(1).max(2_000),
    evidence_segment_ids: EvidenceIdsSchema,
    evidence_quotes: EvidenceQuotesSchema,
  })
  .strict();

export const ActionItemSchema = z
  .object({
    id: z.string().min(1).max(64),
    task: z.string().min(1).max(2_000),
    owner_id: z.null(),
    owner_text: z.string().max(160).nullable(),
    deadline_text: z.string().max(160).nullable(),
    deadline_iso: z.string().date().nullable(),
    status: z.enum(["confirmed", "needs_confirmation", "suggested"]),
    evidence_segment_ids: EvidenceIdsSchema,
    evidence_quotes: EvidenceQuotesSchema,
  })
  .strict();

export const MeetingMinutesSchema = z
  .object({
    title: z.string().min(1).max(160),
    summary: z.string().max(20_000),
    decisions: z.array(DecisionSchema).max(100),
    action_items: z.array(ActionItemSchema).max(100),
    open_questions: z
      .array(
        z
          .object({
            id: z.string().min(1).max(64),
            text: z.string().min(1).max(2_000),
            evidence_segment_ids: EvidenceIdsSchema,
            evidence_quotes: EvidenceQuotesSchema,
          })
          .strict(),
      )
      .max(100),
    suggestions: z
      .array(
        z
          .object({
            id: z.string().min(1).max(64),
            text: z.string().min(1).max(2_000),
          })
          .strict(),
      )
      .max(100),
  })
  .strict();

export const MeetingNotesProviderResponseSchema = z
  .object({
    minutes: MeetingMinutesSchema,
    metadata: z
      .object({
        model: z.string().min(1),
        prompt_version: z.string().min(1),
        schema_version: z.string().min(1),
        transcript_revision: z.string().min(1),
        input_tokens: z.number().int().nonnegative().nullable(),
        output_tokens: z.number().int().nonnegative().nullable(),
        total_tokens: z.number().int().nonnegative().nullable(),
        duration_ms: z.number().int().nonnegative(),
      })
      .strict(),
  })
  .strict();

export const MEETING_MINUTES_JSON_SCHEMA = {
  type: "object",
  additionalProperties: false,
  required: [
    "title",
    "summary",
    "decisions",
    "action_items",
    "open_questions",
    "suggestions",
  ],
  properties: {
    title: { type: "string" },
    summary: { type: "string" },
    decisions: {
      type: "array",
      items: {
        type: "object",
        additionalProperties: false,
        required: ["id", "text", "evidence_segment_ids", "evidence_quotes"],
        properties: {
          id: { type: "string" },
          text: { type: "string" },
          evidence_segment_ids: {
            type: "array",
            minItems: 1,
            items: { type: "string" },
          },
          evidence_quotes: {
            type: "array",
            minItems: 1,
            items: {
              type: "object",
              additionalProperties: false,
              required: ["segment_id", "quote"],
              properties: {
                segment_id: { type: "string" },
                quote: { type: "string" },
              },
            },
          },
        },
      },
    },
    action_items: {
      type: "array",
      items: {
        type: "object",
        additionalProperties: false,
        required: [
          "id",
          "task",
          "owner_id",
          "owner_text",
          "deadline_text",
          "deadline_iso",
          "status",
          "evidence_segment_ids",
          "evidence_quotes",
        ],
        properties: {
          id: { type: "string" },
          task: { type: "string" },
          owner_id: { type: "null" },
          owner_text: { type: ["string", "null"] },
          deadline_text: { type: ["string", "null"] },
          deadline_iso: { type: ["string", "null"] },
          status: {
            type: "string",
            enum: ["confirmed", "needs_confirmation", "suggested"],
          },
          evidence_segment_ids: {
            type: "array",
            minItems: 1,
            items: { type: "string" },
          },
          evidence_quotes: {
            type: "array",
            minItems: 1,
            items: {
              type: "object",
              additionalProperties: false,
              required: ["segment_id", "quote"],
              properties: {
                segment_id: { type: "string" },
                quote: { type: "string" },
              },
            },
          },
        },
      },
    },
    open_questions: {
      type: "array",
      items: {
        type: "object",
        additionalProperties: false,
        required: ["id", "text", "evidence_segment_ids", "evidence_quotes"],
        properties: {
          id: { type: "string" },
          text: { type: "string" },
          evidence_segment_ids: {
            type: "array",
            minItems: 1,
            items: { type: "string" },
          },
          evidence_quotes: {
            type: "array",
            minItems: 1,
            items: {
              type: "object",
              additionalProperties: false,
              required: ["segment_id", "quote"],
              properties: {
                segment_id: { type: "string" },
                quote: { type: "string" },
              },
            },
          },
        },
      },
    },
    suggestions: {
      type: "array",
      items: {
        type: "object",
        additionalProperties: false,
        required: ["id", "text"],
        properties: { id: { type: "string" }, text: { type: "string" } },
      },
    },
  },
} as const;

export type MeetingMinutesRequest = z.infer<typeof MeetingMinutesRequestSchema>;
export type MeetingMinutes = z.infer<typeof MeetingMinutesSchema>;

export function validateEvidenceReferences(
  minutes: MeetingMinutes,
  segments: readonly { id: string; text: string }[],
): MeetingMinutes {
  const sourceById = new Map(
    segments.map((segment) => [segment.id, segment.text]),
  );
  const items = [
    ...minutes.decisions,
    ...minutes.action_items,
    ...minutes.open_questions,
  ];
  for (const item of items) {
    const ids = new Set(item.evidence_segment_ids);
    const quoteIds = new Set<string>();
    for (const evidence of item.evidence_quotes) {
      const source = sourceById.get(evidence.segment_id);
      if (!source)
        throw new Error(
          "The generated minutes referenced a segment outside this transcript revision.",
        );
      if (!evidence.quote.trim() || !source.includes(evidence.quote))
        throw new Error(
          "The generated minutes included a quote that does not match its source segment.",
        );
      quoteIds.add(evidence.segment_id);
    }
    if (ids.size !== quoteIds.size || [...ids].some((id) => !quoteIds.has(id)))
      throw new Error(
        "Every evidence segment must have a matching verbatim quote.",
      );
  }
  return minutes;
}
