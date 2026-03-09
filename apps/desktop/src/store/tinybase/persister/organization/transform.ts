import type { JsonValue } from "@hypr/plugin-fs-sync";
import type { OrganizationStorage } from "@hypr/store";

export function frontmatterToOrganization(
  frontmatter: Record<string, unknown>,
  _body: string,
): OrganizationStorage {
  return {
    user_id: String(frontmatter.user_id ?? ""),
    created_at: frontmatter.created_at
      ? String(frontmatter.created_at)
      : undefined,
    name: String(frontmatter.name ?? ""),
    pinned: Boolean(frontmatter.pinned ?? false),
    pin_order:
      frontmatter.pin_order != null ? Number(frontmatter.pin_order) : undefined,
  };
}

export function organizationToFrontmatter(org: OrganizationStorage): {
  frontmatter: Record<string, JsonValue>;
  body: string;
} {
  return {
    frontmatter: {
      user_id: org.user_id ?? "",
      created_at: org.created_at ?? "",
      name: org.name ?? "",
      pinned: org.pinned ?? false,
      pin_order: org.pin_order ?? 0,
    },
    body: "",
  };
}
