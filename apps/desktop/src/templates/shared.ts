import { useMutation } from "@tanstack/react-query";
import { useCallback } from "react";

import { eq, max, ne, sql, templates } from "@hypr/db";
import type { TemplateSection } from "@hypr/store";

import {
  parseStoredTemplateSections,
  parseStoredTemplateTargets,
  parseWebTemplates,
  serializeTemplateSections,
  serializeTemplateTargets,
  type WebTemplate,
} from "./codec";

import { db, useDrizzleLiveQuery } from "~/db";
import * as main from "~/store/tinybase/store/main"; // still used by useTemplateCreatorName

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

function mapTemplateRows(rows: Record<string, unknown>[]): UserTemplate[] {
  return rows.map(mapTemplateRow);
}

function mapTemplateRow(row: Record<string, unknown>): UserTemplate {
  const id = row.id as string;
  const sections = parseStoredTemplateSections(
    row.sections_json ?? row.sectionsJson,
    id,
  );
  const targets = parseStoredTemplateTargets(
    row.targets_json ?? row.targetsJson,
    id,
  );

  return {
    id,
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
      const targets = serializeTemplateTargets(
        template.targets,
        `create template ${template.title || id} targets`,
      );
      const sections = serializeTemplateSections(
        template.sections,
        `create template ${template.title || id} sections`,
      );

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
      const targets = serializeTemplateTargets(
        template.targets,
        `save template ${template.id} targets`,
      );
      const sections = serializeTemplateSections(
        template.sections,
        `save template ${template.id} sections`,
      );

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

export { parseWebTemplates };
export type { WebTemplate };
