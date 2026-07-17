import { z } from "zod";

export const jsonObject = <T extends z.ZodTypeAny>(schema: T) => {
  return z.union([z.string(), z.any()]).transform((input, ctx) => {
    try {
      const parsed = typeof input === "string" ? JSON.parse(input) : input;
      return schema.parse(parsed);
    } catch (e) {
      ctx.addIssue({ code: "custom", message: String(e) });
      return z.NEVER;
    }
  });
};

type TransformForSchema<T> = T extends undefined
  ? undefined
  : T extends string
    ? string | undefined
    : T extends number
      ? number | undefined
      : T extends boolean
        ? boolean | undefined
        : T extends Array<any>
          ? string
          : T extends object
            ? string
            : T;

export type ToStorageType<T> = T extends { _output: infer Output }
  ? {
      [K in keyof Omit<Output, "id">]: TransformForSchema<Output[K]>;
    }
  : never;
