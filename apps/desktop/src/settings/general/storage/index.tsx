import { Trans } from "@lingui/react/macro";

import { ExportLocationRow } from "./export-location";
import {
  LegacyMigrationCleanupRow,
  useLegacyMigrationCleanup,
} from "./legacy-cleanup";

export function StorageSettingsView() {
  const { visible } = useLegacyMigrationCleanup();

  return (
    <div>
      <h2 className="mb-4 font-sans text-lg font-semibold">
        <Trans>Storage</Trans>
      </h2>
      <div className="flex flex-col gap-3">
        <ExportLocationRow />
        {visible && <LegacyMigrationCleanupRow />}
      </div>
    </div>
  );
}
