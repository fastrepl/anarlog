import { useMutation } from "@tanstack/react-query";
import { useCallback } from "react";

import { db, eq, max, ne, sql, templates } from "@hypr/db";
import type { TemplateSection } from "@hypr/store";

import { useDrizzleLiveQuery } from "~/db/use-drizzle-live-query";
import * as main from "~/store/tinybase/store/main"; // still used by useTemplateCreatorName

export type WebTemplate = {
  slug: string;
  title: string;
  description: string;
  category: string;
  targets?: string[];
  sections: TemplateSection[];
};

export type UserTemplate = {
  id: string;
  title: string;
  description: string;
  pinned: boolean;
  pin_order?: number;
  category?: string;
  targets?: string[];
  sections: TemplateSection[];
};

type TemplateDraft = {
  title: string;
  description: string;
  category?: string;
  targets?: string[];
  sections: TemplateSection[];
};

function parseJsonString(value: string): unknown {
  try {
    return JSON.parse(value);
  } catch {
    return value;
  }
}

function normalizeTemplateSection(value: unknown): TemplateSection | null {
  if (typeof value === "string") {
    const title = value.trim();
    if (!title) {
      return null;
    }

    return {
      title,
      description: "",
    };
  }

  if (!value || typeof value !== "object") {
    return null;
  }

  const section = value as Record<string, unknown>;
  const title = typeof section.title === "string" ? section.title.trim() : "";
  if (!title) {
    return null;
  }

  const description =
    typeof section.description === "string" && section.description.trim()
      ? section.description.trim()
      : "";

  return {
    title,
    description,
  };
}

export function normalizeTemplateSections(value: unknown): TemplateSection[] {
  if (typeof value === "string") {
    const parsed = parseJsonString(value);
    if (parsed !== value) {
      return normalizeTemplateSections(parsed);
    }

    const normalizedSection = normalizeTemplateSection(value);
    return normalizedSection ? [normalizedSection] : [];
  }

  if (!Array.isArray(value)) {
    return [];
  }

  return value.flatMap((section) => {
    const normalizedSection = normalizeTemplateSection(section);
    return normalizedSection ? [normalizedSection] : [];
  });
}

function normalizeTemplateTargets(value: unknown): string[] | undefined {
  if (typeof value === "string") {
    const parsed = parseJsonString(value);
    if (parsed !== value) {
      return normalizeTemplateTargets(parsed);
    }

    const target = value.trim();
    return target ? [target] : undefined;
  }

  if (!Array.isArray(value)) {
    return undefined;
  }

  const targets = value.flatMap((target) => {
    if (typeof target !== "string") {
      return [];
    }

    const trimmed = target.trim();
    return trimmed ? [trimmed] : [];
  });

  return targets.length > 0 ? targets : undefined;
}

export function normalizeWebTemplates(
  templates: Record<string, unknown>[],
): WebTemplate[] {
  return templates.flatMap((template, index) => {
    const slug =
      typeof template.slug === "string" && template.slug.trim()
        ? template.slug.trim()
        : `template-${index}`;
    const title =
      typeof template.title === "string" ? template.title.trim() : "";

    if (!title) {
      return [];
    }

    return [
      {
        slug,
        title,
        description:
          typeof template.description === "string" ? template.description : "",
        category:
          typeof template.category === "string" ? template.category : "",
        targets: normalizeTemplateTargets(template.targets),
        sections: normalizeTemplateSections(template.sections),
      },
    ];
  });
}

function mapTemplateRows(rows: Record<string, unknown>[]): UserTemplate[] {
  return rows.map(mapTemplateRow);
}

function mapTemplateRow(row: Record<string, unknown>): UserTemplate {
  const sections = normalizeTemplateSections(
    row.sections_json ?? row.sectionsJson,
  );
  const targets = normalizeTemplateTargets(row.targets_json ?? row.targetsJson);

  return {
    id: row.id as string,
    title: row.title as string,
    description: row.description as string,
    pinned: Boolean(row.pinned),
    pin_order:
      ((row.pin_order ?? row.pinOrder) as number | undefined) ?? undefined,
    category: (row.category as string | null) ?? undefined,
    targets,
    sections,
  };
}

export function resolveTemplateTabSelection({
  isWebMode,
  selectedMineId,
  selectedWebIndex,
  userTemplates,
  webTemplates,
}: {
  isWebMode: boolean | null | undefined;
  selectedMineId: string | null | undefined;
  selectedWebIndex: number | null | undefined;
  userTemplates: UserTemplate[];
  webTemplates: WebTemplate[];
}) {
  const hasUserTemplates = userTemplates.length > 0;
  const hasWebTemplates = webTemplates.length > 0;

  let effectiveIsWebMode = isWebMode ?? !hasUserTemplates;

  if (effectiveIsWebMode && !hasWebTemplates && hasUserTemplates) {
    effectiveIsWebMode = false;
  }

  if (!effectiveIsWebMode && !hasUserTemplates && hasWebTemplates) {
    effectiveIsWebMode = true;
  }

  if (effectiveIsWebMode) {
    const effectiveSelectedWebIndex =
      selectedWebIndex !== null &&
      selectedWebIndex !== undefined &&
      selectedWebIndex >= 0 &&
      selectedWebIndex < webTemplates.length
        ? selectedWebIndex
        : hasWebTemplates
          ? 0
          : null;

    return {
      isWebMode: true,
      selectedMineId: null,
      selectedWebIndex: effectiveSelectedWebIndex,
      selectedWebTemplate:
        effectiveSelectedWebIndex !== null
          ? (webTemplates[effectiveSelectedWebIndex] ?? null)
          : null,
    };
  }

  return {
    isWebMode: false,
    selectedMineId:
      userTemplates.find((template) => template.id === selectedMineId)?.id ??
      userTemplates[0]?.id ??
      null,
    selectedWebIndex: null,
    selectedWebTemplate: null,
  };
}

export function useUserTemplates(): UserTemplate[] {
  const query = db.select().from(templates).orderBy(templates.id);

  const { data = [] } = useDrizzleLiveQuery<
    Record<string, unknown>,
    UserTemplate[]
  >(query, { mapRows: mapTemplateRows });

  return data;
}

export function useUserTemplate(id: string | null | undefined) {
  const query = db
    .select()
    .from(templates)
    .where(eq(templates.id, id ?? ""))
    .limit(1);

  return useDrizzleLiveQuery<Record<string, unknown>, UserTemplate | null>(
    query,
    {
      mapRows: (rows) => {
        const row = rows[0];
        return row ? mapTemplateRow(row) : null;
      },
    },
  );
}

export async function getTemplateById(
  id: string,
): Promise<UserTemplate | null> {
  if (!id) {
    return null;
  }

  const rows = await db
    .select()
    .from(templates)
    .where(eq(templates.id, id))
    .limit(1);

  const row = rows[0];
  if (!row) {
    return null;
  }

  return mapTemplateRow(row as unknown as Record<string, unknown>);
}

export function useTemplateCreatorName() {
  const userId = main.UI.useValue("user_id", main.STORE_ID);
  const name = main.UI.useCell("humans", userId ?? "", "name", main.STORE_ID);

  return typeof name === "string" && name.trim().length > 0
    ? name.trim()
    : "user";
}

export function getTemplateCreatorLabel({
  isUserTemplate,
  creatorName,
}: {
  isUserTemplate: boolean;
  creatorName?: string | null;
}) {
  return isUserTemplate
    ? `Created by ${creatorName?.trim() || "user"}`
    : "Created by Char";
}

export function getTemplateCreatorByline({
  isUserTemplate,
  creatorName,
}: {
  isUserTemplate: boolean;
  creatorName?: string | null;
}) {
  return isUserTemplate ? `by ${creatorName?.trim() || "user"}` : "by Char";
}

export function useCreateTemplate() {
  const mutation = useMutation({
    mutationFn: async (template: TemplateDraft) => {
      const id = crypto.randomUUID();
      const targets = normalizeTemplateTargets(template.targets);
      const sections = normalizeTemplateSections(template.sections);

      await db.insert(templates).values({
        id,
        title: template.title,
        description: template.description,
        pinned: false,
        category: template.category,
        targetsJson: targets ?? null,
        sectionsJson: sections,
        createdAt: sql`strftime('%Y-%m-%dT%H:%M:%SZ', 'now')`,
        updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%SZ', 'now')`,
      });

      return id;
    },
    onError: (error) => {
      console.error("[useCreateTemplate]", error);
    },
  });

  return useCallback(
    (template: TemplateDraft) => mutation.mutateAsync(template),
    [mutation],
  );
}

export function useSaveTemplate() {
  const mutation = useMutation({
    mutationFn: async (template: UserTemplate) => {
      const targets = normalizeTemplateTargets(template.targets);
      const sections = normalizeTemplateSections(template.sections);

      await db
        .update(templates)
        .set({
          title: template.title,
          description: template.description,
          pinned: template.pinned,
          pinOrder: template.pin_order ?? null,
          category: template.category ?? null,
          targetsJson: targets ?? null,
          sectionsJson: sections,
          updatedAt: sql`strftime('%Y-%m-%dT%H:%M:%SZ', 'now')`,
        })
        .where(eq(templates.id, template.id));

      return template.id;
    },
    onError: (error) => {
      console.error("[useSaveTemplate]", error);
    },
  });

  return useCallback(
    (template: UserTemplate) => mutation.mutateAsync(template),
    [mutation],
  );
}

export function useDeleteTemplate() {
  const mutation = useMutation({
    mutationFn: async (id: string) => {
      await db.delete(templates).where(eq(templates.id, id));
    },
    onError: (error) => {
      console.error("[useDeleteTemplate]", error);
    },
  });

  return useCallback((id: string) => mutation.mutateAsync(id), [mutation]);
}

export function useToggleTemplateFavorite() {
  const saveTemplate = useSaveTemplate();

  return useCallback(
    async (templateId: string) => {
      const template = await getTemplateById(templateId);
      if (!template) {
        return;
      }

      if (template.pinned) {
        await saveTemplate({
          ...template,
          pinned: false,
          pin_order: 0,
        });
        return;
      }

      const [row] = await db
        .select({ maxOrder: max(templates.pinOrder) })
        .from(templates)
        .where(ne(templates.id, templateId));

      await saveTemplate({
        ...template,
        pinned: true,
        pin_order: ((row?.maxOrder as number | null) ?? 0) + 1,
      });
    },
    [saveTemplate],
  );
}
