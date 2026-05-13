import { DownloadButtons } from "./download-buttons";

import { SettingsPageTitle } from "~/settings/page-title";

export function SettingsLab() {
  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title="General" />

      <div>
        <h2 className="mb-4 font-serif text-lg font-semibold">Preview</h2>
        <div className="flex flex-col gap-6">
          <DownloadButtons />
        </div>
      </div>
    </div>
  );
}
