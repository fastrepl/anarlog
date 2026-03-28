import { useForm } from "@tanstack/react-form";
import { HeartIcon, MoreHorizontalIcon } from "lucide-react";
import { useState } from "react";

import type { Template, TemplateSection, TemplateStorage } from "@hypr/store";
import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";
import { Input } from "@hypr/ui/components/ui/input";
import { Textarea } from "@hypr/ui/components/ui/textarea";
import { cn } from "@hypr/utils";

import { useToggleTemplateFavorite } from "../shared";
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
    pinned: Boolean(record.pinned),
    pin_order:
      typeof record.pin_order === "number" ? record.pin_order : undefined,
    category: typeof record.category === "string" ? record.category : undefined,
    sections,
    targets,
  };
}

export function TemplateForm({
  id,
  handleDeleteTemplate,
  handleDuplicateTemplate,
}: {
  id: string;
  handleDeleteTemplate: (id: string) => void;
  handleDuplicateTemplate: (id: string) => void;
}) {
  const row = main.UI.useRow("templates", id, main.STORE_ID);
  const value = row ? normalizeTemplatePayload(row) : undefined;
  const toggleTemplateFavorite = useToggleTemplateFavorite();
  const [actionsOpen, setActionsOpen] = useState(false);

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
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0 flex-1">
            {value.category ? (
              <div className="mb-1">
                <span className="font-mono text-xs text-stone-400">
                  {value.category}
                </span>
              </div>
            ) : null}
            <form.Field name="title">
              {(field) => (
                <Input
                  value={field.state.value}
                  onChange={(e) => field.handleChange(e.target.value)}
                  placeholder="Enter template title"
                  className="h-auto border-0 px-0 py-0 text-lg font-semibold shadow-none focus-visible:ring-0 md:text-lg"
                />
              )}
            </form.Field>
            <form.Field name="description">
              {(field) => (
                <Textarea
                  value={field.state.value}
                  onChange={(e) => field.handleChange(e.target.value)}
                  placeholder="Describe the template purpose..."
                  className="mt-1 min-h-[24px] resize-none border-0 px-0 py-0 text-sm text-neutral-500 shadow-none focus-visible:ring-0"
                  rows={1}
                />
              )}
            </form.Field>
          </div>
          <div className="ml-4 flex shrink-0 items-center gap-1">
            <Button
              type="button"
              size="icon"
              variant="ghost"
              onClick={() => toggleTemplateFavorite(id)}
              className={cn([
                "text-neutral-500 hover:text-neutral-800",
                value.pinned && "text-rose-500 hover:text-rose-600",
              ])}
              title={value.pinned ? "Unfavorite template" : "Favorite template"}
              aria-label={
                value.pinned ? "Unfavorite template" : "Favorite template"
              }
            >
              <HeartIcon
                className="size-4"
                fill={value.pinned ? "currentColor" : "none"}
              />
            </Button>
            <Button
              type="button"
              size="sm"
              variant="default"
              onClick={setSelectedTemplateId}
              title={isDefault ? "Remove as default" : "Set as default"}
              className="shrink-0"
            >
              {isDefault ? "Default" : "Set default"}
            </Button>
            <DropdownMenu open={actionsOpen} onOpenChange={setActionsOpen}>
              <DropdownMenuTrigger asChild>
                <Button
                  type="button"
                  size="icon"
                  variant="ghost"
                  className={cn([
                    "text-neutral-500 hover:text-neutral-800",
                    actionsOpen &&
                      "bg-neutral-100 text-neutral-800 hover:bg-neutral-100",
                  ])}
                  aria-label="Template actions"
                >
                  <MoreHorizontalIcon className="size-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent variant="app" align="end">
                <AppFloatingPanel className="overflow-hidden p-1">
                  <DropdownMenuItem
                    onClick={() => handleDuplicateTemplate(id)}
                    className="cursor-pointer"
                  >
                    Duplicate
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    onClick={() => handleDeleteTemplate(id)}
                    className="cursor-pointer text-red-600 focus:text-red-600"
                  >
                    Delete
                  </DropdownMenuItem>
                </AppFloatingPanel>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>
        {value.targets && value.targets.length > 0 ? (
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
        ) : null}
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
