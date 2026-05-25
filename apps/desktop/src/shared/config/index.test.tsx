import { renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { createMergeableStore } from "tinybase/with-schemas";
import { describe, expect, test } from "vitest";

import { useConfigValue } from ".";

import * as settings from "~/store/tinybase/store/settings";
import type { SettingsValueKey } from "~/store/tinybase/store/settings";

function createWrapper(
  values: Partial<Record<SettingsValueKey, unknown>>,
): ({ children }: { children: ReactNode }) => ReactNode {
  return function ConfigTestWrapper({ children }) {
    const store = settings.UI.useCreateMergeableStore(() => {
      const store = createMergeableStore()
        .setTablesSchema(settings.SCHEMA.table)
        .setValuesSchema(settings.SCHEMA.value) as settings.Store;

      store.setValues(values as Parameters<settings.Store["setValues"]>[0]);

      return store;
    });

    return (
      <settings.UI.Provider storesById={{ [settings.STORE_ID]: store }}>
        {children}
      </settings.UI.Provider>
    );
  };
}

describe("useConfigValue", () => {
  test("uses legacy don't-save when audio retention is missing", () => {
    const { result } = renderHook(() => useConfigValue("audio_retention"), {
      wrapper: createWrapper({ save_recordings: false }),
    });

    expect(result.current).toBe("none");
  });

  test("keeps explicit default audio retention over legacy save_recordings", () => {
    const { result } = renderHook(() => useConfigValue("audio_retention"), {
      wrapper: createWrapper({
        save_recordings: false,
        audio_retention: "oneMonth",
      }),
    });

    expect(result.current).toBe("oneMonth");
  });
});
