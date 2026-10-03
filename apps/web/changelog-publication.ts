import { z } from "zod";

const availabilitySchema = z.discriminatedUnion("platform", [
  z.object({
    platform: z.literal("ios"),
    destination: z.enum(["testflight", "app-store"]),
    build: z.string().regex(/^\d+$/),
    publishedAt: z.iso.datetime({ offset: true }),
    evidenceUrl: z.url({ protocol: /^https$/ }),
  }),
  z.object({
    platform: z.literal("android"),
    destination: z.enum(["play-internal", "play-beta", "play-production"]),
    build: z.string().regex(/^\d+$/),
    publishedAt: z.iso.datetime({ offset: true }),
    evidenceUrl: z.url({ protocol: /^https$/ }),
  }),
]);

export const mobilePublicationSchema = z
  .object({
    version: z.string().regex(/^\d+\.\d+\.\d+$/),
    sourceSha: z.string().regex(/^[a-f0-9]{40}$/),
    channel: z.enum(["stable", "beta"]),
    availability: z.array(availabilitySchema).min(1),
  })
  .superRefine((release, context) => {
    const platforms = new Set<string>();
    for (const available of release.availability) {
      const stable = ["app-store", "play-production"].includes(
        available.destination,
      );
      if (
        stable !== (release.channel === "stable") ||
        platforms.has(available.platform) ||
        Date.parse(available.publishedAt) > Date.now()
      ) {
        context.addIssue({
          code: "custom",
          message:
            "Availability must be unique, already released, and match the channel",
        });
      }
      platforms.add(available.platform);
    }
  });

export type ChangelogPublication = {
  publishedAt: string;
  channel: "stable" | "beta";
  availability: {
    platform: "ios" | "android";
    destination:
      | "testflight"
      | "app-store"
      | "play-internal"
      | "play-beta"
      | "play-production";
  }[];
};

export function getMobilePublication(
  raw: string,
  version: string,
): ChangelogPublication {
  const release = mobilePublicationSchema.parse(JSON.parse(raw));
  if (release.version !== version) {
    throw new Error(`Mobile publication version does not match ${version}`);
  }
  return {
    channel: release.channel,
    publishedAt: release.availability
      .map((available) => available.publishedAt)
      .sort((a, b) => Date.parse(a) - Date.parse(b))[0],
    availability: release.availability.map(({ platform, destination }) => ({
      platform,
      destination,
    })),
  };
}
