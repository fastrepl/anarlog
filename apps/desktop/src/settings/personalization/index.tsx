import { useForm } from "@tanstack/react-form";
import { useCallback } from "react";

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
  const value = useConfigValues(PERSONALIZATION_KEYS);

  const setPartialValues = settings.UI.useSetPartialValuesCallback(
    (
      row: Partial<
        Record<(typeof PERSONALIZATION_KEYS)[number], string | undefined>
      >,
    ) => row,
    [],
    settings.STORE_ID,
  );

  const form = useForm({
    defaultValues: {
      chat_style_tone: value.chat_style_tone,
      chat_warmth: value.chat_warmth,
      chat_enthusiasm: value.chat_enthusiasm,
      chat_headers_lists: value.chat_headers_lists,
      chat_emoji: value.chat_emoji,
      chat_custom_instructions: value.chat_custom_instructions,
    },
    listeners: {
      onChange: ({ formApi }) => {
        const {
          form: { errors },
        } = formApi.getAllErrors();
        if (errors.length > 0) {
          console.log(errors);
        }
        void formApi.handleSubmit();
      },
    },
    onSubmit: ({ value }) => {
      setPartialValues(value);
    },
  });

  return { form };
}

export function SettingsPersonalization() {
  const { form } = usePersonalizationForm();
  const openNew = useTabs((state) => state.openNew);
  const selectTab = useTabs((state) => state.select);
  const updatePromptsTabState = useTabs((state) => state.updatePromptsTabState);
  const tabs = useTabs((state) => state.tabs);

  const handleOpenMeetingNotes = useCallback(() => {
    const promptsTab = tabs.find(
      (tab): tab is Extract<Tab, { type: "prompts" }> => tab.type === "prompts",
    );

    if (promptsTab) {
      updatePromptsTabState(promptsTab, { selectedTask: "enhance" });
      selectTab(promptsTab);
      return;
    }

    openNew({
      type: "prompts",
      state: {
        selectedTask: "enhance",
      },
    });
  }, [openNew, selectTab, tabs, updatePromptsTabState]);

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title="Personalization" />

      <div className="flex flex-col gap-8">
        <div>
          <h2 className="mb-4 font-serif text-lg font-semibold">General</h2>
          <p className="mb-6 text-sm text-neutral-600">
            Set the default voice and response style Char uses in chat.
          </p>

          <div className="flex flex-col gap-4">
            <h3 className="text-sm font-medium">Chat defaults</h3>

            <form.Field name="chat_style_tone">
              {(field) => (
                <SelectSettingRow
                  title="Base style & tone"
                  description="The main voice Char uses in chat conversations."
                  value={field.state.value}
                  onChange={field.handleChange}
                  options={STYLE_TONE_OPTIONS}
                />
              )}
            </form.Field>
          </div>
        </div>

        <div>
          <h2 className="mb-4 font-serif text-lg font-semibold">
            Meeting Notes
          </h2>
          <p className="mb-6 text-sm text-neutral-600">
            Customize how Char generates summaries and meeting notes.
          </p>

          <div className="flex flex-col gap-4">
            <h3 className="text-sm font-medium">Prompt template</h3>

            <MeetingNotesRow onOpen={handleOpenMeetingNotes} />
          </div>
        </div>

        <div>
          <h2 className="mb-4 font-serif text-lg font-semibold">Chat</h2>
          <p className="mb-6 text-sm text-neutral-600">
            Fine-tune detailed chat behavior, formatting, and instructions.
          </p>

          <div className="flex flex-col gap-8">
            <div className="flex flex-col gap-4">
              <h3 className="text-sm font-medium">Characteristics</h3>

              <div className="flex flex-col gap-6">
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
                      title="Headers & Lists"
                      description="Adjust how often chat uses headings and bullet lists."
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
                      description="Control whether chat uses emojis in responses."
                      value={field.state.value}
                      onChange={field.handleChange}
                      options={EMOJI_OPTIONS}
                    />
                  )}
                </form.Field>
              </div>
            </div>

            <div className="flex flex-col gap-4">
              <div>
                <h3 className="mb-1 text-sm font-medium">
                  Custom instructions
                </h3>
                <p className="text-xs text-neutral-600">
                  Tell Char how you want chat responses to behave, format
                  answers, or prioritize information.
                </p>
              </div>

              <form.Field name="chat_custom_instructions">
                {(field) => (
                  <InstructionsTextarea
                    value={field.state.value}
                    onChange={field.handleChange}
                    placeholder="Be very direct. Keep answers short and practical. Call out risks first."
                  />
                )}
              </form.Field>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function MeetingNotesRow({ onOpen }: { onOpen: () => void }) {
  return (
    <div className="flex items-center justify-between gap-6">
      <div className="flex-1">
        <h3 className="mb-1 text-sm font-medium">Advanced editor</h3>
        <p className="text-xs text-neutral-600">
          Open the dedicated editor for the meeting notes prompt template.
        </p>
      </div>

      <Button variant="outline" size="sm" onClick={onOpen}>
        Open editor
      </Button>
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
        <SelectTrigger className="w-40 bg-white shadow-none focus:ring-0">
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

function InstructionsTextarea({
  value,
  onChange,
  placeholder,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
}) {
  return (
    <Textarea
      className="min-h-28 resize-none rounded-lg border-neutral-200 bg-white px-4 py-3 text-sm leading-6 text-neutral-900 shadow-none placeholder:text-neutral-400 focus-visible:ring-0"
      value={value}
      onChange={(event) => onChange(event.target.value)}
      placeholder={placeholder}
    />
  );
}
