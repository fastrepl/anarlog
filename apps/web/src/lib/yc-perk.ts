import { z } from "zod";

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
    }
  | {
      status: "invalid";
      reason: "not_verified" | "email_missing" | "email_mismatch";
    };

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
  email: z
    .string()
    .trim()
    .min(1, "Enter your work email")
    .email("Enter a valid email")
    .max(320),
  verificationUrl: z
    .string()
    .trim()
    .min(1, "Paste your YC verification link")
    .max(2_048)
    .refine(isYcVerificationUrl, "Use your ycombinator.com/verify link"),
  additionalComments: z.string().max(200),
});

export function validateYcPerkEmail(value: string) {
  const result = ycPerkRequestSchema.shape.email.safeParse(value);
  return result.success ? undefined : result.error.issues[0]?.message;
}

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
  email,
  verificationUrl,
  fetcher = fetch,
  signal = AbortSignal.timeout(5_000),
}: {
  email: string;
  verificationUrl: string;
  fetcher?: typeof fetch;
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
  if (
    verification.data.email.trim().toLowerCase() !== email.trim().toLowerCase()
  ) {
    return { status: "invalid", reason: "email_mismatch" };
  }

  return {
    status: "verified",
    firstName: verification.data.name?.trim().split(/\s+/)[0] || "there",
  };
}
