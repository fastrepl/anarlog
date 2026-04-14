import { useMutation } from "@tanstack/react-query";
import { useCallback } from "react";

import { eq, max, ne, sql, templates } from "@hypr/db";
import type { TemplateSection } from "@hypr/store";

import {
  assertCanonicalTemplateSections,
  assertCanonicalTemplateTargets,
  parseStoredTemplateSections,
  parseStoredTemplateTargets,
} from "./codec";

import { db, useDrizzleLiveQuery } from "~/db";

type TemplateRow = (typeof templates)["$inferSelect"];
type NewTemplateRow = (typeof templates)["$inferInsert"];
type TemplateLiveRow = {
  id: string;
  title: string;
  description: string;
  pinned: number | boolean;
  pin_order: number | null;
  category: string | null;
  targets_json: unknown;
  sections_json: unknown;
};
type StoredTemplateRecord = {
  id: string;
  title: string;
  description: string;
  pinned: boolean;
  pinOrder: number | null;
  category: string | null;
  targetsJson: unknown;
  sectionsJson: unknown;
};

export type UserTemplate = {
  id: string;
  title: string;
  description: string;
  pinned: boolean;
  pinOrder?: number;
  category?: string;
  targets?: string[];
  sections: TemplateSection[];
};

export type UserTemplateDraft = Pick<
  UserTemplate,
  "title" | "description" | "category" | "targets" | "sections"
>;

const templateRowSelection = {
  id: templates.id,
  title: templates.title,
  description: templates.description,
  pinned: templates.pinned,
  pinOrder: templates.pinOrder,
  category: templates.category,
  targetsJson: templates.targetsJson,
  sectionsJson: templates.sectionsJson,
  createdAt: templates.createdAt,
  updatedAt: templates.updatedAt,
};

function mapTemplateRows(rows: TemplateRow[]): UserTemplate[] {
  return rows.map((row) =>
    mapStoredTemplateRecord({
      id: row.id,
      title: row.title,
      description: row.description,
      pinned: row.pinned,
      pinOrder: row.pinOrder,
      category: row.category,
      targetsJson: row.targetsJson,
      sectionsJson: row.sectionsJson,
    }),
  );
}

function mapStoredTemplateRecord(row: StoredTemplateRecord): UserTemplate {
  const sections = parseStoredTemplateSections(row.sectionsJson, row.id);
  const targets = parseStoredTemplateTargets(row.targetsJson, row.id);

  return {
    id: row.id,
    title: row.title,
    description: row.description,
    pinned: row.pinned,
    pinOrder: row.pinOrder ?? undefined,
    category: row.category ?? undefined,
    targets,
    sections,
  };
}

function mapTemplateLiveRows(rows: TemplateLiveRow[]): UserTemplate[] {
  return rows.map((row) =>
    mapStoredTemplateRecord({
      id: row.id,
      title: row.title,
      description: row.description,
      pinned: Boolean(row.pinned),
      pinOrder: row.pin_order,
      category: row.category,
      targetsJson: row.targets_json,
      sectionsJson: row.sections_json,
    }),
  );
}

export function useUserTemplates(): UserTemplate[] {
  const query = db.select().from(templates).orderBy(templates.id);

  const { data = [] } = useDrizzleLiveQuery<TemplateLiveRow, UserTemplate[]>(
    query,
    {
      mapRows: mapTemplateLiveRows,
    },
  );

  return data;
}

export function useUserTemplate(id: string | null | undefined) {
  const query = db
    .select()
    .from(templates)
    .where(eq(templates.id, id ?? ""))
    .limit(1);

  return useDrizzleLiveQuery<TemplateLiveRow, UserTemplate | null>(query, {
    mapRows: (rows) => {
      return mapTemplateLiveRows(rows)[0] ?? null;
    },
  });
}

export async function getTemplateById(
  id: string,
): Promise<UserTemplate | null> {
  if (!id) {
    return null;
  }

  const rows = await db
    .select(templateRowSelection)
    .from(templates)
    .where(eq(templates.id, id))
    .limit(1);

  const row = rows[0];
  if (!row) {
    return null;
  }

  return mapTemplateRows([row])[0] ?? null;
}

export function useCreateTemplate() {
  const mutation = useMutation({
    mutationFn: async (template: UserTemplateDraft) => {
      const id = crypto.randomUUID();
      const targets = assertCanonicalTemplateTargets(
        template.targets,
        `create template ${template.title || id} targets`,
      );
      const sections = assertCanonicalTemplateSections(
        template.sections,
        `create template ${template.title || id} sections`,
      );

      const values: Omit<NewTemplateRow, "createdAt" | "updatedAt"> = {
        id,
        title: template.title,
        description: template.description,
        pinned: false,
        category: template.category,
        targetsJson: targets ?? null,
        sectionsJson: sections,
      };

      await db.insert(templates).values({
        ...values,
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
    (template: UserTemplateDraft) => mutation.mutateAsync(template),
    [mutation],
  );
}

export function useSaveTemplate() {
  const mutation = useMutation({
    mutationFn: async (template: UserTemplate) => {
      const targets = assertCanonicalTemplateTargets(
        template.targets,
        `save template ${template.id} targets`,
      );
      const sections = assertCanonicalTemplateSections(
        template.sections,
        `save template ${template.id} sections`,
      );

      await db
        .update(templates)
        .set({
          title: template.title,
          description: template.description,
          pinned: template.pinned,
          pinOrder: template.pinOrder ?? null,
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
          pinOrder: 0,
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
        pinOrder: ((row?.maxOrder as number | null) ?? 0) + 1,
      });
    },
    [saveTemplate],
  );
}

export function getTemplateCopyTitle(title: string) {
  const value = title.trim();

  if (!value) return "Untitled (Copy)";
  if (value.endsWith("(Copy)")) return value;

  return `${value} (Copy)`;
}
