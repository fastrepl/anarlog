import { Loader2Icon } from "lucide-react";
import type { ReactNode } from "react";

import { useStoredSettingValuesQuery } from "~/settings/queries";

export function SettingsHydrationBoundary({
  children,
}: {
  children: ReactNode;
}) {
  const { data, isLoading, error } = useStoredSettingValuesQuery();

  if (error) {
    throw error;
  }
  if (isLoading || !data) {
    return (
      <div className="flex min-h-48 items-center justify-center">
        <Loader2Icon
          aria-label="Loading settings"
          className="text-muted-foreground size-5 animate-spin"
        />
      </div>
    );
  }

  return children;
}
