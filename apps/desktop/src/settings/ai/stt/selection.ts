export function getPreferredProviderModel(
  savedModel: string | undefined,
  models: Array<{ id: string }>,
) {
  if (
    savedModel &&
    (models.length === 0 || models.some((model) => model.id === savedModel))
  ) {
    return savedModel;
  }

  return models[0]?.id ?? "";
}
