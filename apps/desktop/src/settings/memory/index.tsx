import { CustomVocabularyView } from "./custom-vocabulary";

import { SettingsPageTitle } from "~/settings/page-title";

export function SettingsMemory() {
  return (
    <div className="flex flex-col gap-6">
      <SettingsPageTitle title="Memory" />
      <CustomVocabularyView />
    </div>
  );
}
