import { z } from "zod";

const searchEntityTypeSchema = z.enum(["session", "human", "organization"]);
export type SearchEntityType = z.infer<typeof searchEntityTypeSchema>;

export const searchDocumentSchema = z.object({
  id: z.string(),
  type: searchEntityTypeSchema,
  title: z.string(),
  content: z.string(),
  created_at: z.number(),
});

export type SearchDocument = z.infer<typeof searchDocumentSchema>;

const numberFilterSchema = z
  .object({
    gte: z.number().optional(),
    lte: z.number().optional(),
    gt: z.number().optional(),
    lt: z.number().optional(),
    eq: z.number().optional(),
  })
  .optional();

export const searchFiltersSchema = z.object({
  created_at: numberFilterSchema,
});

export type SearchFilters = z.infer<typeof searchFiltersSchema>;

export type SearchHit = {
  score: number;
  document: SearchDocument;
};
