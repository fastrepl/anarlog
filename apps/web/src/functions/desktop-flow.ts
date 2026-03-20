import { z } from "zod";

export const DESKTOP_SCHEMES = [
  "hypr",
  "hyprnote",
  "hyprnote-nightly",
  "hyprnote-staging",
  "char",
  "char-nightly",
  "char-staging",
] as const;

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

export const normalizeDesktopRedirectUri = (
  value: string | undefined,
): string | undefined => {
  if (!value) {
    return undefined;
  }

  try {
    const url = new URL(value);

    if (url.protocol !== "http:") {
      return undefined;
    }

    if (!url.port || url.username || url.password || url.search || url.hash) {
      return undefined;
    }

    if (url.pathname !== "/" && url.pathname !== "") {
      return undefined;
    }

    const hostname = url.hostname.replace(/^\[(.*)\]$/, "$1").toLowerCase();
    if (
      hostname !== "localhost" &&
      hostname !== "127.0.0.1" &&
      hostname !== "::1"
    ) {
      return undefined;
    }

    const port = Number.parseInt(url.port, 10);
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      return undefined;
    }

    return `http://127.0.0.1:${url.port}`;
  } catch {
    return undefined;
  }
};

export const desktopRedirectUriSchema = z
  .string()
  .optional()
  .transform((v) => normalizeDesktopRedirectUri(v));
