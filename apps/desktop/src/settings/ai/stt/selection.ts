type ModelEntry = {
  id: string;
};

export function getPreferredProviderModel(
  providerId: string,
  savedModels: Record<string, string>,
  models: ModelEntry[],
) {
  const savedModel = savedModels[providerId];

  if (
    savedModel &&
    (models.length === 0 || models.some((model) => model.id === savedModel))
  ) {
    return savedModel;
  }

  return models[0]?.id ?? "";
}
