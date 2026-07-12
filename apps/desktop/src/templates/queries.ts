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
import {
  DEFAULT_TEMPLATE_ICON,
  normalizeTemplateIcon,
  type TemplateIcon,
} from "./template-icon";

import { db, useDrizzleLiveQuery } from "~/db";

type TemplateRow = (typeof templates)["$inferSelect"];
type NewTemplateRow = (typeof templates)["$inferInsert"];
type TemplateLiveRow = {
  id: string;
  title: string;
  description: string;
  pinned: boolean;
  pin_order: number | null;
  category: string | null;
  icon_json: unknown;
  targets_json: unknown;
  sections_json: unknown;
};

export type UserTemplate = {
  id: string;
  title: string;
  description: string;
  pinned: boolean;
  pinOrder?: number;
  category?: string;
  icon: TemplateIcon;
  targets?: string[];
  sections: TemplateSection[];
};

export type UserTemplateDraft = Pick<
  UserTemplate,
  "title" | "description" | "category" | "targets" | "sections"
> & { icon?: TemplateIcon };

const templateRowSelection = {
  id: templates.id,
  title: templates.title,
  description: templates.description,
  pinned: templates.pinned,
  pinOrder: templates.pinOrder,
  category: templates.category,
  iconJson: templates.iconJson,
  targetsJson: templates.targetsJson,
  sectionsJson: templates.sectionsJson,
  createdAt: templates.createdAt,
  updatedAt: templates.updatedAt,
};

function toUserTemplate(
  id: string,
  title: string,
  description: string,
  pinned: boolean,
  pinOrder: number | null,
  category: string | null,
  iconJson: unknown,
  targetsJson: unknown,
  sectionsJson: unknown,
): UserTemplate {
  return {
    id,
    title,
    description,
    pinned,
    pinOrder: pinOrder ?? undefined,
    category: category ?? undefined,
    icon: normalizeTemplateIcon(iconJson),
    targets: parseStoredTemplateTargets(targetsJson, id),
    sections: parseStoredTemplateSections(sectionsJson, id),
  };
}

function mapTemplateRows(rows: TemplateRow[]): UserTemplate[] {
  return rows.map((row) =>
    toUserTemplate(
      row.id,
      row.title,
      row.description,
      row.pinned,
      row.pinOrder,
      row.category,
      row.iconJson,
      row.targetsJson,
      row.sectionsJson,
    ),
  );
}

function mapTemplateLiveRows(rows: TemplateLiveRow[]): UserTemplate[] {
  return rows.map((row) =>
    toUserTemplate(
      row.id,
      row.title,
      row.description,
      row.pinned,
      row.pin_order,
      row.category,
      row.icon_json,
      row.targets_json,
      row.sections_json,
    ),
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
  const { mutateAsync } = useMutation({
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
        iconJson: template.icon ?? DEFAULT_TEMPLATE_ICON,
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

  return mutateAsync;
}

export function useSaveTemplate() {
  const { mutateAsync } = useMutation({
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
          iconJson: normalizeTemplateIcon(template.icon),
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

  return mutateAsync;
}

export function useDeleteTemplate() {
  const { mutateAsync } = useMutation({
    mutationFn: async (id: string) => {
      await db.delete(templates).where(eq(templates.id, id));
    },
    onError: (error) => {
      console.error("[useDeleteTemplate]", error);
    },
  });

  return mutateAsync;
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
