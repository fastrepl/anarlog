import type { JsonValue } from "@hypr/plugin-fs-sync";
import type { HumanStorage } from "@hypr/store";

type HumanFrontmatter = Omit<HumanStorage, "memo">;

function emailsToStore(frontmatter: Record<string, unknown>): string {
  const emails = frontmatter.emails;
  if (Array.isArray(emails)) {
    return emails
      .map((e) => String(e).trim())
      .filter(Boolean)
      .join(",");
  }
  return typeof frontmatter.email === "string" ? frontmatter.email : "";
}

function emailToFrontmatter(email: string | undefined): string[] {
  if (!email) return [];
  return email
    .split(",")
    .map((e) => e.trim())
    .filter(Boolean);
}

function frontmatterToStore(
  frontmatter: Record<string, unknown>,
): HumanFrontmatter {
  return {
    user_id: String(frontmatter.user_id ?? ""),
    created_at: frontmatter.created_at
      ? String(frontmatter.created_at)
      : undefined,
    name: String(frontmatter.name ?? ""),
    email: emailsToStore(frontmatter),
    org_id: String(frontmatter.org_id ?? ""),
    job_title: String(frontmatter.job_title ?? ""),
    linkedin_username: String(frontmatter.linkedin_username ?? ""),
    pinned: Boolean(frontmatter.pinned ?? false),
    pin_order:
      frontmatter.pin_order != null ? Number(frontmatter.pin_order) : undefined,
  };
}

function storeToFrontmatter(
  store: Partial<HumanFrontmatter>,
): Record<string, JsonValue> {
  return {
    user_id: store.user_id ?? "",
    created_at: store.created_at ?? "",
    name: store.name ?? "",
    emails: emailToFrontmatter(store.email),
    org_id: store.org_id ?? "",
    job_title: store.job_title ?? "",
    linkedin_username: store.linkedin_username ?? "",
    pinned: store.pinned ?? false,
    pin_order: store.pin_order ?? 0,
  };
}

export function frontmatterToHuman(
  frontmatter: Record<string, unknown>,
  body: string,
): HumanStorage {
  return {
    ...frontmatterToStore(frontmatter),
    memo: body,
  };
}

export function humanToFrontmatter(human: HumanStorage): {
  frontmatter: Record<string, JsonValue>;
  body: string;
} {
  const { memo, ...storeFields } = human;
  return {
    frontmatter: storeToFrontmatter(storeFields),
    body: memo ?? "",
  };
}
