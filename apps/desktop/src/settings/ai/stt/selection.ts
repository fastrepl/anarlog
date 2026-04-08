type ModelEntry = {
  id: string;
  isDownloaded?: boolean;
};

type PreferredProviderModelOptions = {
  allowSavedModelWithoutChoices?: boolean;
};

const normalizeSavedModel = (
  savedModel: string | undefined,
  models: ModelEntry[],
) => {
  if (savedModel === "universal") {
    if (models.some((model) => model.id === "universal-3-pro")) {
      return "universal-3-pro";
    }

    if (models.some((model) => model.id === "u3-rt-pro")) {
      return "u3-rt-pro";
    }
  }

  return savedModel;
};

export function getPreferredProviderModel(
  savedModel: string | undefined,
  models: ModelEntry[],
  options?: PreferredProviderModelOptions,
) {
  const normalizedSavedModel = normalizeSavedModel(savedModel, models);
  const selectableModels = models.filter((model) => model.isDownloaded ?? true);

  if (
    normalizedSavedModel &&
    selectableModels.some((model) => model.id === normalizedSavedModel)
  ) {
    return normalizedSavedModel;
  }

  if (selectableModels.length > 0) {
    return selectableModels[0].id;
  }

  if (options?.allowSavedModelWithoutChoices) {
    return normalizedSavedModel ?? "";
  }

  return "";
}
