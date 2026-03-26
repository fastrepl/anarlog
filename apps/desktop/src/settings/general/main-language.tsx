import { useMemo } from "react";

import { getLanguageOptions, normalizeLanguageCode } from "./language";
import {
  SearchableSelect,
  type SearchableSelectOption,
} from "./searchable-select";

export function MainLanguageView({
  value,
  onChange,
  supportedLanguages,
}: {
  value: string;
  onChange: (value: string) => void;
  supportedLanguages: readonly string[];
}) {
  const normalizedValue = useMemo(() => {
    return normalizeLanguageCode(value);
  }, [value]);

  const options: SearchableSelectOption[] = useMemo(
    () =>
      getLanguageOptions(supportedLanguages).map((option) => ({
        value: option.value,
        label: option.label,
      })),
    [supportedLanguages],
  );

  return (
    <div className="flex flex-row items-center justify-between">
      <div>
        <h3 className="mb-1 text-sm font-medium">Main language</h3>
        <p className="text-xs text-neutral-600">
          Language for summaries, chats, and AI-generated responses
        </p>
      </div>
      <SearchableSelect
        value={normalizedValue}
        onChange={onChange}
        options={options}
        placeholder="Select language"
        searchPlaceholder="Search language..."
        className="w-40"
      />
    </div>
  );
}
