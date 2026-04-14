import type { TemplateSection } from "@hypr/store";

export type WebTemplate = {
  slug: string;
  title: string;
  description: string;
  category: string;
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

export function assertCanonicalTemplateSections(
  value: unknown,
  context: string,
): TemplateSection[] {
  if (!Array.isArray(value)) {
    return templateDataError(context, "sections must be an array");
  }

  return value.map((section, index) => {
    if (!section || typeof section !== "object") {
      return templateDataError(
        context,
        `sections[${index}] must be an object with title and description`,
      );
    }

    const next = section as Record<string, unknown>;
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

    return {
      title: next.title,
      description: next.description,
    };
  });
}

export function assertCanonicalTemplateTargets(
  value: unknown,
  context: string,
): string[] | undefined {
  if (value === null || value === undefined) {
    return undefined;
  }

  if (!Array.isArray(value)) {
    return templateDataError(context, "targets must be an array of strings");
  }

  return value.map((target, index) => {
    if (typeof target !== "string") {
      return templateDataError(context, `targets[${index}] must be a string`);
    }

    return target;
  });
}

export function parseStoredTemplateSections(
  value: unknown,
  templateId: string,
): TemplateSection[] {
  const context = `template ${templateId} sections_json`;
  const parsed =
    typeof value === "string" ? parseJsonText(value, context) : value;
  return normalizeStoredTemplateSections(parsed, context);
}

export function parseStoredTemplateTargets(
  value: unknown,
  templateId: string,
): string[] | undefined {
  if (value === null || value === undefined) {
    return undefined;
  }

  const context = `template ${templateId} targets_json`;
  const parsed =
    typeof value === "string" ? parseJsonText(value, context) : value;
  return normalizeStoredTemplateTargets(parsed, context);
}

function normalizeStoredTemplateSections(
  value: unknown,
  context: string,
): TemplateSection[] {
  if (!Array.isArray(value)) {
    return templateDataError(context, "sections must be an array");
  }

  return value.map((section, index) => {
    if (typeof section === "string") {
      const title = section.trim();
      if (!title) {
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
    if (typeof next.title !== "string" || !next.title.trim()) {
      return templateDataError(
        context,
        `sections[${index}].title must be a non-empty string`,
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

    return {
      title: next.title.trim(),
      description: typeof next.description === "string" ? next.description : "",
    };
  });
}

function normalizeStoredTemplateTargets(
  value: unknown,
  context: string,
): string[] | undefined {
  if (value === null || value === undefined) {
    return undefined;
  }

  if (typeof value === "string") {
    const target = value.trim();
    return target ? [target] : undefined;
  }

  if (!Array.isArray(value)) {
    return templateDataError(context, "targets must be an array of strings");
  }

  const targets = value.flatMap((target, index) => {
    if (typeof target !== "string") {
      return templateDataError(context, `targets[${index}] must be a string`);
    }

    const trimmed = target.trim();
    return trimmed ? [trimmed] : [];
  });

  return targets.length > 0 ? targets : undefined;
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
    targets: assertCanonicalTemplateTargets(
      template.targets ?? undefined,
      `web template ${template.title} targets`,
    ),
    sections: parseWebTemplateSections(
      template.sections,
      `web template ${template.title} sections`,
    ),
  };
}

function parseWebTemplateSections(
  value: unknown,
  context: string,
): TemplateSection[] {
  if (!Array.isArray(value)) {
    return templateDataError(context, "sections must be an array");
  }

  return value.map((section, index) => {
    if (!section || typeof section !== "object") {
      return templateDataError(context, `sections[${index}] must be an object`);
    }

    const next = section as Record<string, unknown>;
    if (typeof next.title !== "string" || !next.title.trim()) {
      return templateDataError(
        context,
        `sections[${index}].title must be a non-empty string`,
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

    return {
      title: next.title,
      description: typeof next.description === "string" ? next.description : "",
    };
  });
}
