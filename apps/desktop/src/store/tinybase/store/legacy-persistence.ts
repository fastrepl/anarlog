let disabled = false;

export function disableLegacyDataPersistence(): void {
  disabled = true;
}

export function enableLegacyDataPersistence(): void {
  disabled = false;
}

export function isLegacyDataPersistenceDisabled(): boolean {
  return disabled;
}

export type LegacyPersister = {
  startAutoPersisting(): Promise<unknown>;
  stopAutoPersisting(stopSaveFirst?: boolean): Promise<unknown>;
};

export async function cutoverLegacyDataPersistence(
  persisters: LegacyPersister[],
): Promise<void> {
  disableLegacyDataPersistence();
  try {
    await Promise.all(
      persisters.map((persister) => persister.stopAutoPersisting(true)),
    );
  } catch (error) {
    enableLegacyDataPersistence();
    await Promise.allSettled(
      persisters.map((persister) => persister.startAutoPersisting()),
    );
    throw error;
  }
}
