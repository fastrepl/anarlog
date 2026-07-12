import type { TemplateSection } from "@hypr/store";

import { normalizeTemplateIcon, type TemplateIcon } from "./template-icon";

export type WebTemplate = {
  slug: string;
  title: string;
  description: string;
  category: string;
  icon: TemplateIcon;
  targets?: string[];
  sections: TemplateSection[];
};

function templateDataError(context: string, detail: string): never {
  throw new Error(`[templates] ${context}: ${detail}`);
}

function parseJsonText(value: string, context: string): unknown {
  try {
    return JSON.parse(value);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return templateDataError(context, `invalid JSON (${message})`);
  }
}

function validateSections(
  value: unknown,
  context: string,
  {
    allowBareStrings,
    requireDescription,
    allowBlankDrafts = false,
  }: {
    allowBareStrings: boolean;
    requireDescription: boolean;
    allowBlankDrafts?: boolean;
  },
): TemplateSection[] {
  if (!Array.isArray(value)) {
    return templateDataError(context, "sections must be an array");
  }

  return value.flatMap((section, index) => {
    if (allowBareStrings && typeof section === "string") {
      const title = section.trim();
      if (!title) {
        if (allowBlankDrafts) {
          return [];
        }
        return templateDataError(
          context,
          `sections[${index}] must not be an empty string`,
        );
      }
      return { title, description: "" };
    }

    if (!section || typeof section !== "object") {
      return templateDataError(
        context,
        `sections[${index}] must be an object with title and description`,
      );
    }

    const next = section as Record<string, unknown>;

    if (requireDescription) {
      if (typeof next.title !== "string") {
        return templateDataError(
          context,
          `sections[${index}].title must be a string`,
        );
      }
      if (typeof next.description !== "string") {
        return templateDataError(
          context,
          `sections[${index}].description must be a string`,
        );
      }
      const title = next.title.trim();
      if (!title && !allowBlankDrafts) {
        return templateDataError(
          context,
          `sections[${index}].title must be a non-empty string`,
        );
      }
      return { title, description: next.description };
    }

    if (typeof next.title !== "string") {
      return templateDataError(
        context,
        `sections[${index}].title must be a string`,
      );
    }

    if (
      next.description !== undefined &&
      typeof next.description !== "string"
    ) {
      return templateDataError(
        context,
        `sections[${index}].description must be a string when present`,
      );
    }

    const title = next.title.trim();
    const description =
      typeof next.description === "string" ? next.description : "";

    if (!title) {
      if (allowBlankDrafts) {
        return { title, description };
      }
      return templateDataError(
        context,
        `sections[${index}].title must be a non-empty string`,
      );
    }

    return {
      title,
      description,
    };
  });
}

function validateTargets(
  value: unknown,
  context: string,
  { lenient }: { lenient: boolean },
): string[] | undefined {
  if (value === null || value === undefined) {
    return undefined;
  }

  if (lenient && typeof value === "string") {
    const target = value.trim();
    return target ? [target] : undefined;
  }

  if (!Array.isArray(value)) {
    return templateDataError(context, "targets must be an array of strings");
  }

  if (lenient) {
    const targets = value.flatMap((target, index) => {
      if (typeof target !== "string") {
        return templateDataError(context, `targets[${index}] must be a string`);
      }
      const trimmed = target.trim();
      return trimmed ? [trimmed] : [];
    });
    return targets.length > 0 ? targets : undefined;
  }

  return value.map((target, index) => {
    if (typeof target !== "string") {
      return templateDataError(context, `targets[${index}] must be a string`);
    }
    return target;
  });
}

export function assertCanonicalTemplateSections(
  value: unknown,
  context: string,
): TemplateSection[] {
  return validateSections(value, context, {
    allowBareStrings: false,
    requireDescription: true,
    allowBlankDrafts: true,
  });
}

export function assertCanonicalTemplateTargets(
  value: unknown,
  context: string,
): string[] | undefined {
  return validateTargets(value, context, { lenient: false });
}

export function parseStoredTemplateSections(
  value: unknown,
  templateId: string,
): TemplateSection[] {
  const context = `template ${templateId} sections_json`;
  try {
    const parsed =
      typeof value === "string" ? parseJsonText(value, context) : value;
    return validateSections(parsed, context, {
      allowBareStrings: true,
      requireDescription: false,
      allowBlankDrafts: true,
    });
  } catch (error) {
    console.error(
      "[templates] dropping invalid stored template sections",
      error,
    );
    return [];
  }
}

export function parseStoredTemplateTargets(
  value: unknown,
  templateId: string,
): string[] | undefined {
  if (value === null || value === undefined) {
    return undefined;
  }

  const context = `template ${templateId} targets_json`;
  try {
    const parsed =
      typeof value === "string" ? parseJsonText(value, context) : value;
    return validateTargets(parsed, context, { lenient: true });
  } catch (error) {
    console.error(
      "[templates] dropping invalid stored template targets",
      error,
    );
    return undefined;
  }
}

export function parseWebTemplates(
  templates: Record<string, unknown>[],
): WebTemplate[] {
  return templates.flatMap((template, index) => {
    try {
      return [parseWebTemplate(template, index)];
    } catch (error) {
      console.error("[templates] dropping invalid web template", error);
      return [];
    }
  });
}

function parseWebTemplate(
  template: Record<string, unknown>,
  index: number,
): WebTemplate {
  if (typeof template.title !== "string" || !template.title.trim()) {
    return templateDataError(
      `web template ${index}`,
      "title must be a non-empty string",
    );
  }

  return {
    slug:
      typeof template.slug === "string" && template.slug.trim()
        ? template.slug.trim()
        : `template-${index}`,
    title: template.title.trim(),
    description:
      typeof template.description === "string" ? template.description : "",
    category: typeof template.category === "string" ? template.category : "",
    icon: normalizeTemplateIcon(template.icon ?? template.icon_json),
    targets: validateTargets(
      template.targets ?? undefined,
      `web template ${template.title} targets`,
      { lenient: true },
    ),
    sections: validateSections(
      template.sections,
      `web template ${template.title} sections`,
      { allowBareStrings: false, requireDescription: false },
    ),
  };
}
