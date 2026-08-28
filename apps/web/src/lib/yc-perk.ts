import { z } from "zod";

type Fetcher = (...args: Parameters<typeof fetch>) => ReturnType<typeof fetch>;

const YC_VERIFICATION_HOSTS = new Set([
  "www.ycombinator.com",
  "ycombinator.com",
]);

const ycFounderVerificationSchema = z
  .object({
    verified: z.boolean(),
    name: z.string().optional(),
    email: z.string().email().optional(),
    companies: z
      .array(
        z
          .object({
            name: z.string(),
            batch: z.string().optional(),
          })
          .passthrough(),
      )
      .optional(),
  })
  .passthrough();

export type YcFounderVerificationResult =
  | {
      status: "verified";
      firstName: string;
      email: string;
    }
  | {
      status: "invalid";
      reason: "not_verified" | "email_missing";
    };

export function isYcPromotionCode(value: string) {
  return /^YC-[A-F0-9]{24}$/i.test(value.trim());
}

export function normalizeYcPromotionCode(value: string) {
  return value.trim().toUpperCase();
}

export type YcPerkApplyValue =
  | { type: "verification_url"; verificationUrl: string }
  | { type: "promotion_code"; code: string }
  | { type: "invalid"; message: string };

export const ycPerkApplyInputSchema = z.object({
  value: z
    .string()
    .trim()
    .min(1, "Paste your YC verification link or YC- code")
    .max(2_048),
});

export function parseYcPerkApplyValue(value: string): YcPerkApplyValue {
  const trimmed = value.trim();
  if (!trimmed) {
    return {
      type: "invalid",
      message: "Paste your YC verification link or YC- code",
    };
  }

  if (isYcPromotionCode(trimmed)) {
    return { type: "promotion_code", code: normalizeYcPromotionCode(trimmed) };
  }

  if (/^https?:\/\//i.test(trimmed) || trimmed.includes("ycombinator.com")) {
    if (!isYcVerificationUrl(trimmed)) {
      return {
        type: "invalid",
        message: "Use your ycombinator.com/verify link",
      };
    }
    return {
      type: "verification_url",
      verificationUrl: normalizeYcVerificationUrl(trimmed),
    };
  }

  return {
    type: "invalid",
    message: "Paste your YC verification link or YC- code",
  };
}

export function validateYcPerkApplyValue(value: string) {
  const parsed = parseYcPerkApplyValue(value);
  return parsed.type === "invalid" ? parsed.message : undefined;
}

export function isYcVerificationUrl(value: string) {
  try {
    const url = new URL(value.trim());
    const segments = url.pathname.split("/").filter(Boolean);

    return (
      url.protocol === "https:" &&
      YC_VERIFICATION_HOSTS.has(url.hostname.toLowerCase()) &&
      !url.username &&
      !url.password &&
      segments.length === 2 &&
      segments[0] === "verify" &&
      segments[1].length > 0
    );
  } catch {
    return false;
  }
}

export const ycPerkRequestSchema = z.object({
  verificationUrl: z
    .string()
    .trim()
    .min(1, "Paste your YC verification link")
    .max(2_048)
    .refine(isYcVerificationUrl, "Use your ycombinator.com/verify link"),
  additionalComments: z.string().max(200),
});

export function validateYcVerificationUrl(value: string) {
  const result = ycPerkRequestSchema.shape.verificationUrl.safeParse(value);
  return result.success ? undefined : result.error.issues[0]?.message;
}

export function normalizeYcVerificationUrl(value: string) {
  const url = new URL(value.trim());
  url.hostname = "www.ycombinator.com";
  url.hash = "";
  url.search = "";
  url.pathname = url.pathname.replace(/\/$/, "");
  return url.toString();
}

export function getYcVerificationApiUrl(value: string) {
  return `${normalizeYcVerificationUrl(value)}.json`;
}

export async function verifyYcFounder({
  verificationUrl,
  fetcher = fetch,
  signal = AbortSignal.timeout(5_000),
}: {
  verificationUrl: string;
  fetcher?: Fetcher;
  signal?: AbortSignal;
}): Promise<YcFounderVerificationResult> {
  const response = await fetcher(getYcVerificationApiUrl(verificationUrl), {
    headers: { Accept: "application/json" },
    redirect: "error",
    signal,
  });
  if (!response.ok) {
    throw new Error("YC verification is temporarily unavailable");
  }

  const verification = ycFounderVerificationSchema.safeParse(
    await response.json(),
  );
  if (!verification.success) {
    throw new Error("YC verification returned an unexpected response");
  }
  if (!verification.data.verified) {
    return { status: "invalid", reason: "not_verified" };
  }
  if (!verification.data.email) {
    return { status: "invalid", reason: "email_missing" };
  }

  return {
    status: "verified",
    firstName: verification.data.name?.trim().split(/\s+/)[0] || "there",
    email: verification.data.email.trim().toLowerCase(),
  };
}
