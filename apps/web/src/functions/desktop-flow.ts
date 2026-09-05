import { z } from "zod";

export const DESKTOP_SCHEMES = [
  "anarlog",
  "anarlog-staging",
  "anarlog-dev",
  "hypr",
  "hyprnote",
  "hyprnote-staging",
  "char",
  "char-staging",
] as const;

export const DEFAULT_DESKTOP_SCHEME = "anarlog";
export const desktopSchemeSchema = z.enum(DESKTOP_SCHEMES);
export type DesktopScheme = z.infer<typeof desktopSchemeSchema>;

export const flowSearchSchema = <T extends z.ZodRawShape>(
  common: T,
  opts: { defaultFlow?: "desktop" | "web" } = {},
) => {
  const defaultFlow = opts.defaultFlow ?? "web";
  const desktopFlowSchema =
    defaultFlow === "desktop"
      ? z.literal("desktop").default("desktop")
      : z.literal("desktop");
  const webFlowSchema =
    defaultFlow === "web" ? z.literal("web").default("web") : z.literal("web");

  return z.union([
    z.object({
      ...common,
      flow: desktopFlowSchema,
      scheme: desktopSchemeSchema,
    }),
    z.object({
      ...common,
      flow: webFlowSchema,
      scheme: desktopSchemeSchema.optional(),
    }),
  ]);
};
