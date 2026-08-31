import {
  DEFAULT_TEMPLATE_ICON,
  normalizeTemplateIcon,
  type TemplateIcon,
} from "~/templates/template-icon";

export const DEFAULT_FOLDER_ICON = {
  type: "icon",
  value: "folder",
  color: DEFAULT_TEMPLATE_ICON.color,
} as const satisfies TemplateIcon;

export function normalizeFolderIcon(value: unknown): TemplateIcon {
  const candidate = unwrapJsonValue(value);
  if (candidate == null || candidate === "") {
    return DEFAULT_FOLDER_ICON;
  }

  const normalized = normalizeTemplateIcon(candidate);
  if (isExplicitTemplateIcon(candidate)) {
    return normalized;
  }
  return DEFAULT_FOLDER_ICON;
}

export function resolvedFolderIcon(
  path: string,
  persisted: Record<string, TemplateIcon | undefined>,
  overrides: Record<string, TemplateIcon | undefined> = {},
): TemplateIcon {
  return normalizeFolderIcon(
    overrides[path] ?? persisted[path] ?? DEFAULT_FOLDER_ICON,
  );
}

function unwrapJsonValue(value: unknown): unknown {
  let current = value;
  for (let i = 0; i < 3; i += 1) {
    if (typeof current !== "string") {
      return current;
    }
    const trimmed = current.trim();
    if (!trimmed) {
      return "";
    }
    try {
      current = JSON.parse(trimmed);
    } catch {
      return DEFAULT_FOLDER_ICON;
    }
  }
  return current;
}

function isExplicitTemplateIcon(value: unknown): boolean {
  if (!value || typeof value !== "object") {
    return false;
  }
  const icon = value as { type?: unknown; value?: unknown };
  return (
    (icon.type === "emoji" || icon.type === "icon") &&
    typeof icon.value === "string" &&
    icon.value.trim().length > 0
  );
}
