import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  cutoverLegacyDataPersistence,
  disableLegacyDataPersistence,
  enableLegacyDataPersistence,
  isLegacyDataPersistenceDisabled,
} from "./legacy-persistence";

describe("legacy data persistence cutover", () => {
  beforeEach(enableLegacyDataPersistence);

  it("can disable and restore the rollback-file persistence path", () => {
    expect(isLegacyDataPersistenceDisabled()).toBe(false);
    disableLegacyDataPersistence();
    expect(isLegacyDataPersistenceDisabled()).toBe(true);
    enableLegacyDataPersistence();
    expect(isLegacyDataPersistenceDisabled()).toBe(false);
  });

  it("stops legacy loading and saving before SQLite shadows mount", async () => {
    const persister = {
      startAutoPersisting: vi.fn(async () => {}),
      stopAutoPersisting: vi.fn(async () => {}),
    };

    await cutoverLegacyDataPersistence([persister]);

    expect(persister.stopAutoPersisting).toHaveBeenCalledWith(true);
    expect(persister.startAutoPersisting).not.toHaveBeenCalled();
    expect(isLegacyDataPersistenceDisabled()).toBe(true);
  });

  it("restores legacy persistence when cutover cannot stop cleanly", async () => {
    const error = new Error("stop failed");
    const persister = {
      startAutoPersisting: vi.fn(async () => {}),
      stopAutoPersisting: vi.fn(async () => {
        throw error;
      }),
    };

    await expect(cutoverLegacyDataPersistence([persister])).rejects.toBe(error);

    expect(persister.startAutoPersisting).toHaveBeenCalledOnce();
    expect(isLegacyDataPersistenceDisabled()).toBe(false);
  });
});
