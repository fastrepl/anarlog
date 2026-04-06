import { useForm } from "@tanstack/react-form";
import { HeartIcon, MoreHorizontalIcon, Plus, X } from "lucide-react";
import { useRef, useState } from "react";

import type { Template, TemplateSection, TemplateStorage } from "@hypr/store";
import { Badge } from "@hypr/ui/components/ui/badge";
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

import {
  getTemplateCreatorByline,
  useTemplateCreatorName,
  useToggleTemplateFavorite,
} from "../shared";
import { TemplateDetailScrollArea } from "./detail-scroll-area";
import { SectionsList } from "./sections-editor";

import { TemplateCategoryLabel } from "~/shared/ui/template-category-label";
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

function parseTargets(value: string) {
  return value
    .split(",")
    .map((target) => target.trim())
    .filter(Boolean);
}

function TemplateTargetsInput({
  value,
  onChange,
}: {
  value: string[];
  onChange: (value: string[]) => void;
}) {
  const [inputValue, setInputValue] = useState("");
  const [isAddingTag, setIsAddingTag] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const submitTargets = () => {
    const nextTargets = parseTargets(inputValue);
    if (nextTargets.length === 0) {
      setInputValue("");
      setIsAddingTag(false);
      return;
    }

    onChange([...value, ...nextTargets]);
    setInputValue("");
    setIsAddingTag(false);
  };

  return (
    <div
      className="mt-2 flex min-h-6 w-full cursor-text flex-wrap items-center gap-1.5"
      onClick={() => {
        if (!isAddingTag) {
          setIsAddingTag(true);
          return;
        }

        inputRef.current?.focus();
      }}
    >
      {value.map((target, index) => (
        <Badge
          key={`${target}-${index}`}
          variant="secondary"
          className="bg-muted flex h-6 items-center gap-1 rounded-md px-2 py-0.5 text-xs font-normal"
        >
          {target}
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="ml-0.5 h-3 w-3 p-0 hover:bg-transparent"
            onClick={(e) => {
              e.stopPropagation();
              onChange(
                value.filter((_, currentIndex) => currentIndex !== index),
              );
            }}
          >
            <X className="h-2.5 w-2.5" />
          </Button>
        </Badge>
      ))}

      {!isAddingTag ? (
        <button
          type="button"
          className="bg-muted text-muted-foreground hover:bg-muted/80 inline-flex h-6 items-center gap-1 rounded-md px-2 py-0.5 text-xs transition-colors"
          onClick={() => setIsAddingTag(true)}
        >
          <Plus className="h-3 w-3" />
          Add tag
        </button>
      ) : (
        <input
          ref={inputRef}
          type="text"
          autoFocus
          value={inputValue}
          className="min-w-[84px] flex-1 bg-transparent py-0 text-xs leading-none text-neutral-500 outline-hidden"
          onChange={(e) => setInputValue(e.target.value)}
          onBlur={submitTargets}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === "Tab" || e.key === ",") {
              if (!inputValue.trim()) {
                return;
              }

              e.preventDefault();
              submitTargets();
              return;
            }

            if (e.key === "Escape") {
              e.preventDefault();
              setInputValue("");
              setIsAddingTag(false);
              return;
            }

            if (e.key === "Backspace" && !inputValue && value.length > 0) {
              e.preventDefault();
              onChange(value.slice(0, -1));
            }
          }}
        />
      )}
    </div>
  );
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
  const creatorName = useTemplateCreatorName();
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
      targets: value?.targets ?? [],
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
      <div className="pt-1 pr-1 pb-4 pl-3">
        <div className="flex items-center justify-between gap-3">
          <div className="min-w-0">
            <TemplateCategoryLabel category={value.category} />
          </div>
          <div className="flex items-center gap-0">
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={setSelectedTemplateId}
              title={isDefault ? "Remove as default" : "Set as default"}
              className={cn([
                "shrink-0 text-neutral-600 hover:text-black",
                isDefault
                  ? "bg-neutral-100 text-black hover:bg-neutral-100"
                  : null,
              ])}
            >
              {isDefault ? "Current default" : "Set as default"}
            </Button>
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
        <div className="mt-3 min-w-0 pr-5 pl-3">
          <form.Field name="title">
            {(field) => (
              <div className="flex min-w-0 items-baseline gap-2">
                <div className="relative max-w-full min-w-0">
                  <span
                    aria-hidden="true"
                    className="invisible block px-0 py-0 text-lg font-semibold whitespace-pre md:text-lg"
                  >
                    {(field.state.value || " ") + " "}
                  </span>
                  <Input
                    value={field.state.value}
                    onChange={(e) => field.handleChange(e.target.value)}
                    placeholder="Enter template title"
                    className="absolute inset-0 h-auto w-full max-w-full min-w-0 border-0 px-0 py-0 text-lg font-semibold shadow-none focus-visible:ring-0 md:text-lg"
                  />
                </div>
                <span className="shrink-0 text-sm font-normal whitespace-nowrap text-neutral-400">
                  {getTemplateCreatorByline({
                    isUserTemplate: true,
                    creatorName,
                  })}
                </span>
              </div>
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
          <form.Field name="targets">
            {(field) => (
              <TemplateTargetsInput
                value={field.state.value}
                onChange={field.handleChange}
              />
            )}
          </form.Field>
        </div>
      </div>

      <TemplateDetailScrollArea>
        <form.Field name="sections">
          {(field) => (
            <SectionsList
              disabled={false}
              items={field.state.value}
              onChange={(items) => field.handleChange(items)}
            />
          )}
        </form.Field>
      </TemplateDetailScrollArea>
    </div>
  );
}
