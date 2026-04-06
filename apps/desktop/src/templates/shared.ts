import { useCallback, useEffect, useMemo } from "react";

import type { Template, TemplateSection, TemplateStorage } from "@hypr/store";

import * as main from "~/store/tinybase/store/main";

export type WebTemplate = {
  slug: string;
  title: string;
  description: string;
  category: string;
  targets?: string[];
  sections: TemplateSection[];
};

export type UserTemplate = Template & { id: string };

type TemplateDraft = {
  title: string;
  description: string;
  category?: string;
  targets?: string[];
  sections: TemplateSection[];
};

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
  const { user_id } = main.UI.useValues(main.STORE_ID);
  const queries = main.UI.useQueries(main.STORE_ID);

  useEffect(() => {
    queries?.setParamValue(
      main.QUERIES.userTemplates,
      "user_id",
      user_id ?? "",
    );
  }, [queries, user_id]);

  const templates = main.UI.useResultTable(
    main.QUERIES.userTemplates,
    main.STORE_ID,
  );

  return useMemo(() => {
    return Object.entries(templates).map(([id, template]) =>
      normalizeTemplateWithId(id, template as unknown),
    );
  }, [templates]);
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
  const { user_id } = main.UI.useValues(main.STORE_ID);

  const setRow = main.UI.useSetRowCallback(
    "templates",
    (p: {
      id: string;
      user_id: string;
      created_at: string;
      title: string;
      description: string;
      category?: string;
      targets?: string[];
      sections: TemplateSection[];
    }) => p.id,
    (p: {
      id: string;
      user_id: string;
      created_at: string;
      title: string;
      description: string;
      category?: string;
      targets?: string[];
      sections: TemplateSection[];
    }) =>
      ({
        user_id: p.user_id,
        title: p.title,
        description: p.description,
        pinned: false,
        pin_order: undefined,
        category: p.category,
        targets: p.targets ? JSON.stringify(p.targets) : undefined,
        sections: JSON.stringify(p.sections),
      }) satisfies TemplateStorage,
    [],
    main.STORE_ID,
  );

  return useCallback(
    (template: TemplateDraft) => {
      if (!user_id) return null;

      const id = crypto.randomUUID();
      const now = new Date().toISOString();

      setRow({
        id,
        user_id,
        created_at: now,
        title: template.title,
        description: template.description,
        category: template.category,
        targets: template.targets,
        sections: template.sections.map((section) => ({ ...section })),
      });

      return id;
    },
    [user_id, setRow],
  );
}

export function useToggleTemplateFavorite() {
  const store = main.UI.useStore(main.STORE_ID);

  return useCallback(
    (templateId: string) => {
      if (!store) return;

      const isPinned = Boolean(
        store.getCell("templates", templateId, "pinned"),
      );
      if (isPinned) {
        store.setPartialRow("templates", templateId, {
          pinned: false,
          pin_order: 0,
        });
        return;
      }

      const allTemplates = store.getTable("templates");
      const maxPinOrder = Object.entries(allTemplates).reduce(
        (max, [id, template]) => {
          if (id === templateId) return max;

          const order =
            typeof template.pin_order === "number" ? template.pin_order : 0;
          return Math.max(max, order);
        },
        0,
      );

      store.setPartialRow("templates", templateId, {
        pinned: true,
        pin_order: maxPinOrder + 1,
      });
    },
    [store],
  );
}

export function normalizeTemplatePayload(template: unknown): Template {
  const record = (
    template && typeof template === "object" ? template : {}
  ) as Record<string, unknown>;

  let sections: Array<{ title: string; description: string }> = [];
  if (typeof record.sections === "string") {
    try {
      sections = JSON.parse(record.sections);
    } catch {
      sections = [];
    }
  } else if (Array.isArray(record.sections)) {
    sections = record.sections;
  }

  return {
    user_id: typeof record.user_id === "string" ? record.user_id : "",
    title: typeof record.title === "string" ? record.title : "",
    description:
      typeof record.description === "string" ? record.description : "",
    pinned: Boolean(record.pinned),
    pin_order:
      typeof record.pin_order === "number" ? record.pin_order : undefined,
    category: typeof record.category === "string" ? record.category : undefined,
    targets:
      typeof record.targets === "string"
        ? (() => {
            try {
              const parsed = JSON.parse(record.targets);
              return Array.isArray(parsed)
                ? parsed.filter(
                    (target): target is string => typeof target === "string",
                  )
                : undefined;
            } catch {
              return undefined;
            }
          })()
        : Array.isArray(record.targets)
          ? record.targets.filter(
              (target): target is string => typeof target === "string",
            )
          : undefined,
    sections,
  };
}

function normalizeTemplateWithId(id: string, template: unknown) {
  return { id, ...normalizeTemplatePayload(template) };
}
