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
  if (value == null || value === "") {
    return DEFAULT_FOLDER_ICON;
  }
  return normalizeTemplateIcon(value);
}
