import { useForm } from "@tanstack/react-form";

import { Button } from "@hypr/ui/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@hypr/ui/components/ui/select";
import { Textarea } from "@hypr/ui/components/ui/textarea";

import { SettingsPageTitle } from "~/settings/page-title";
import { useConfigValues } from "~/shared/config";
import * as settings from "~/store/tinybase/store/settings";
import { type Tab, useTabs } from "~/store/zustand/tabs";

const PERSONALIZATION_KEYS = [
  "chat_style_tone",
  "chat_warmth",
  "chat_enthusiasm",
  "chat_headers_lists",
  "chat_emoji",
  "chat_custom_instructions",
] as const;

const STYLE_TONE_OPTIONS = [
  { value: "professional", label: "Professional" },
  { value: "friendly", label: "Friendly" },
  { value: "concise", label: "Concise" },
  { value: "technical", label: "Technical" },
] as const;

const CHARACTERISTIC_OPTIONS = [
  { value: "default", label: "Default" },
  { value: "more", label: "More" },
  { value: "less", label: "Less" },
] as const;

const EMOJI_OPTIONS = [
  { value: "default", label: "Default" },
  { value: "less", label: "Less" },
  { value: "none", label: "None" },
] as const;

function usePersonalizationForm() {
  const values = useConfigValues(PERSONALIZATION_KEYS);

  const setPartialValues = settings.UI.useSetPartialValuesCallback(
    (row: Partial<Record<(typeof PERSONALIZATION_KEYS)[number], string>>) =>
      row,
    [],
    settings.STORE_ID,
  );

  return useForm({
    defaultValues: {
      chat_style_tone: values.chat_style_tone,
      chat_warmth: values.chat_warmth,
      chat_enthusiasm: values.chat_enthusiasm,
      chat_headers_lists: values.chat_headers_lists,
      chat_emoji: values.chat_emoji,
      chat_custom_instructions: values.chat_custom_instructions,
    },
    listeners: {
      onChange: ({ formApi }) => {
        void formApi.handleSubmit();
      },
    },
    onSubmit: ({ value }) => {
      setPartialValues(value);
    },
  });
}

export function SettingsPersonalization() {
  const form = usePersonalizationForm();
  const tabs = useTabs((state) => state.tabs);
  const openNew = useTabs((state) => state.openNew);
  const select = useTabs((state) => state.select);
  const updatePromptsTabState = useTabs((state) => state.updatePromptsTabState);

  const openMeetingNotesEditor = () => {
    const promptsTab = tabs.find(
      (tab): tab is Extract<Tab, { type: "prompts" }> => tab.type === "prompts",
    );

    if (!promptsTab) {
      openNew({
        type: "prompts",
        state: {
          selectedTask: "enhance",
        },
      });
      return;
    }

    updatePromptsTabState(promptsTab, {
      ...promptsTab.state,
      selectedTask: "enhance",
    });
    select(promptsTab);
  };

  return (
    <div className="flex flex-col gap-6">
      <SettingsPageTitle title="Personalization" />

      <section className="flex flex-col gap-4">
        <div>
          <h2 className="mb-1 font-serif text-lg font-semibold">Chat</h2>
          <p className="text-sm text-neutral-600">
            Adjust Charlie&apos;s default voice, formatting, and response rules.
          </p>
        </div>

        <div className="flex flex-col gap-6">
          <form.Field name="chat_style_tone">
            {(field) => (
              <SelectSettingRow
                title="Base style and tone"
                description="Set the default voice Charlie uses in chat."
                value={field.state.value}
                onChange={field.handleChange}
                options={STYLE_TONE_OPTIONS}
              />
            )}
          </form.Field>

          <form.Field name="chat_warmth">
            {(field) => (
              <SelectSettingRow
                title="Warmth"
                description="Make responses more personable or more neutral."
                value={field.state.value}
                onChange={field.handleChange}
                options={CHARACTERISTIC_OPTIONS}
              />
            )}
          </form.Field>

          <form.Field name="chat_enthusiasm">
            {(field) => (
              <SelectSettingRow
                title="Enthusiasm"
                description="Control how energetic or restrained the tone feels."
                value={field.state.value}
                onChange={field.handleChange}
                options={CHARACTERISTIC_OPTIONS}
              />
            )}
          </form.Field>

          <form.Field name="chat_headers_lists">
            {(field) => (
              <SelectSettingRow
                title="Headers and lists"
                description="Change how often Charlie uses headings and bullets."
                value={field.state.value}
                onChange={field.handleChange}
                options={CHARACTERISTIC_OPTIONS}
              />
            )}
          </form.Field>

          <form.Field name="chat_emoji">
            {(field) => (
              <SelectSettingRow
                title="Emoji"
                description="Control whether chat replies use emoji."
                value={field.state.value}
                onChange={field.handleChange}
                options={EMOJI_OPTIONS}
              />
            )}
          </form.Field>

          <form.Field name="chat_custom_instructions">
            {(field) => (
              <TextareaSettingRow
                title="Custom instructions"
                description="Add any standing guidance Charlie should follow in chat."
                value={field.state.value}
                onChange={field.handleChange}
                placeholder="Be direct. Keep answers short. Lead with risks."
              />
            )}
          </form.Field>
        </div>
      </section>

      <section className="flex flex-col gap-4">
        <div>
          <h2 className="mb-1 font-serif text-lg font-semibold">
            Meeting Notes
          </h2>
          <p className="text-sm text-neutral-600">
            Open the advanced editor for meeting-note and title-generation
            prompts.
          </p>
        </div>

        <ActionSettingRow
          title="Advanced editor"
          description="Fine-tune meeting-note templates in a split editor with a built-in prompt assistant."
          actionLabel="Open editor"
          onAction={openMeetingNotesEditor}
        />
      </section>
    </div>
  );
}

function SelectSettingRow({
  title,
  description,
  value,
  onChange,
  options,
}: {
  title: string;
  description: string;
  value: string;
  onChange: (value: string) => void;
  options: ReadonlyArray<{ value: string; label: string }>;
}) {
  return (
    <div className="flex items-center justify-between gap-6">
      <div className="flex-1">
        <h3 className="mb-1 text-sm font-medium">{title}</h3>
        <p className="text-xs text-neutral-600">{description}</p>
      </div>

      <Select value={value} onValueChange={onChange}>
        <SelectTrigger className="w-44 bg-white shadow-none focus:ring-0">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

function TextareaSettingRow({
  title,
  description,
  value,
  onChange,
  placeholder,
}: {
  title: string;
  description: string;
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
}) {
  return (
    <div className="flex flex-col gap-3">
      <div>
        <h3 className="mb-1 text-sm font-medium">{title}</h3>
        <p className="text-xs text-neutral-600">{description}</p>
      </div>

      <Textarea
        className="min-h-28 resize-none rounded-lg border-neutral-200 bg-white px-4 py-3 text-sm leading-6 text-neutral-900 shadow-none placeholder:text-neutral-400 focus-visible:ring-0"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
      />
    </div>
  );
}

function ActionSettingRow({
  title,
  description,
  actionLabel,
  onAction,
}: {
  title: string;
  description: string;
  actionLabel: string;
  onAction: () => void;
}) {
  return (
    <div className="flex items-center justify-between gap-6">
      <div className="flex-1">
        <h3 className="mb-1 text-sm font-medium">{title}</h3>
        <p className="text-xs text-neutral-600">{description}</p>
      </div>

      <Button variant="outline" size="sm" onClick={onAction}>
        {actionLabel}
      </Button>
    </div>
  );
}
