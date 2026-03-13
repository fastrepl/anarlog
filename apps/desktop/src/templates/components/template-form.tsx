import { useForm } from "@tanstack/react-form";

import type { Template, TemplateSection, TemplateStorage } from "@hypr/store";
import { Input } from "@hypr/ui/components/ui/input";
import { Textarea } from "@hypr/ui/components/ui/textarea";
import { cn } from "@hypr/utils";

import { RelatedSessions } from "./related-sessions";
import { SectionsList } from "./sections-editor";

import { DangerZone } from "~/shared/ui/resource-list";
import * as main from "~/store/tinybase/store/main";
import * as settings from "~/store/tinybase/store/settings";

function normalizeTemplatePayload(template: unknown): Template {
  const record = (
    template && typeof template === "object" ? template : {}
  ) as Record<string, unknown>;

  let sections: TemplateSection[] = [];
  if (typeof record.sections === "string") {
    try {
      sections = JSON.parse(record.sections);
    } catch {
      sections = [];
    }
  } else if (Array.isArray(record.sections)) {
    sections = record.sections.map((s: unknown) => {
      const sec = s as Record<string, unknown>;
      return {
        title: typeof sec.title === "string" ? sec.title : "",
        description: typeof sec.description === "string" ? sec.description : "",
      };
    });
  }

  let targets: string[] = [];
  if (typeof record.targets === "string") {
    try {
      targets = JSON.parse(record.targets);
    } catch {
      targets = [];
    }
  } else if (Array.isArray(record.targets)) {
    targets = record.targets.filter((t): t is string => typeof t === "string");
  }

  return {
    user_id: typeof record.user_id === "string" ? record.user_id : "",
    title: typeof record.title === "string" ? record.title : "",
    description:
      typeof record.description === "string" ? record.description : "",
    sections,
    targets,
  };
}

export function TemplateForm({
  id,
  handleDeleteTemplate,
}: {
  id: string;
  handleDeleteTemplate: (id: string) => void;
}) {
  const row = main.UI.useRow("templates", id, main.STORE_ID);
  const value = row ? normalizeTemplatePayload(row) : undefined;

  const selectedTemplateId = settings.UI.useValue(
    "selected_template_id",
    settings.STORE_ID,
  ) as string | undefined;
  const isDefault = selectedTemplateId === id;

  const setSelectedTemplateId = settings.UI.useSetValueCallback(
    "selected_template_id",
    () => (isDefault ? "" : id),
    [id, isDefault],
    settings.STORE_ID,
  );

  const handleUpdate = main.UI.useSetPartialRowCallback(
    "templates",
    id,
    (row: Partial<Template>) =>
      ({
        ...row,
        sections: row.sections ? JSON.stringify(row.sections) : undefined,
        targets: row.targets ? JSON.stringify(row.targets) : undefined,
      }) satisfies Partial<TemplateStorage>,
    [id],
    main.STORE_ID,
  );

  const form = useForm({
    defaultValues: {
      title: value?.title ?? "",
      description: value?.description ?? "",
      sections: value?.sections ?? [],
    },
    listeners: {
      onChange: ({ formApi }) => {
        queueMicrotask(() => {
          const {
            form: { errors },
          } = formApi.getAllErrors();
          if (errors.length === 0) {
            void formApi.handleSubmit();
          }
        });
      },
    },
    onSubmit: ({ value }) => {
      handleUpdate(value);
    },
  });

  if (!value) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-sm text-neutral-500">Template not found</p>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-1 flex-col">
      <div className="border-b border-neutral-200 px-6 py-4">
        <div className="flex items-center gap-2">
          <form.Field name="title">
            {(field) => (
              <Input
                value={field.state.value}
                onChange={(e) => field.handleChange(e.target.value)}
                placeholder="Enter template title"
                className="h-8 flex-1 border-0 px-0 text-lg font-semibold shadow-none focus-visible:ring-0"
              />
            )}
          </form.Field>
          <button
            type="button"
            onClick={setSelectedTemplateId}
            title={isDefault ? "Remove as default" : "Set as default"}
            className={cn([
              "shrink-0 rounded border px-2 py-0.5 text-xs transition-colors",
              isDefault
                ? "border-neutral-800 bg-neutral-800 text-white"
                : "border-neutral-300 text-neutral-500 hover:border-neutral-500 hover:text-neutral-700",
            ])}
          >
            {isDefault ? "Default" : "Set default"}
          </button>
        </div>
        <form.Field name="description">
          {(field) => (
            <Textarea
              value={field.state.value}
              onChange={(e) => field.handleChange(e.target.value)}
              placeholder="Describe the template purpose..."
              className="min-h-[40px] resize-none border-0 px-0 text-sm text-neutral-600 shadow-none focus-visible:ring-0"
              rows={2}
            />
          )}
        </form.Field>
        {value.targets && value.targets.length > 0 && (
          <div className="mt-2 flex flex-wrap items-center gap-2">
            {value.targets.map((target, index) => (
              <span
                key={index}
                className="rounded-xs bg-neutral-100 px-2 py-0.5 text-xs text-neutral-600"
              >
                {target}
              </span>
            ))}
          </div>
        )}
      </div>

      <div className="flex-1 overflow-y-auto">
        <div className="border-b border-neutral-200 p-6">
          <h3 className="mb-3 text-sm font-medium text-neutral-600">
            Sections
          </h3>
          <form.Field name="sections">
            {(field) => (
              <SectionsList
                disabled={false}
                items={field.state.value}
                onChange={(items) => field.handleChange(items)}
              />
            )}
          </form.Field>
        </div>

        <div className="border-b border-neutral-200 p-6">
          <h3 className="mb-4 text-sm font-medium text-neutral-600">
            Related Notes
          </h3>
          <RelatedSessions templateId={id} />
        </div>

        <div className="p-6">
          <DangerZone
            title="Delete this template"
            description="This action cannot be undone"
            buttonLabel="Delete Template"
            onAction={() => handleDeleteTemplate(id)}
          />
        </div>

        <div className="pb-96" />
      </div>
    </div>
  );
}
