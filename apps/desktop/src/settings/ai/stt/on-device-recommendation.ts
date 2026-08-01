export type OnDeviceModelCandidate = {
  id: string;
  recommendedMemoryBytes: number;
};

export function recommendOnDeviceModel(
  models: readonly OnDeviceModelCandidate[],
  totalMemoryBytes?: number | null,
) {
  if (models.length === 0) {
    return null;
  }

  const memoryBytes =
    totalMemoryBytes && Number.isFinite(totalMemoryBytes)
      ? totalMemoryBytes
      : 0;
  const fittingModels = models.filter(
    (model) => model.recommendedMemoryBytes <= memoryBytes,
  );
  const candidates = fittingModels.length > 0 ? fittingModels : models;

  return candidates.reduce((recommended, model) => {
    if (fittingModels.length > 0) {
      return model.recommendedMemoryBytes > recommended.recommendedMemoryBytes
        ? model
        : recommended;
    }

    return model.recommendedMemoryBytes < recommended.recommendedMemoryBytes
      ? model
      : recommended;
  }).id;
}
