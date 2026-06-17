type ModelEntry = {
  id: string;
  isDownloaded?: boolean;
};

type PreferredProviderModelOptions = {
  allowSavedModelWithoutChoices?: boolean;
};

export function normalizeStoredSttModel(
  provider: string | undefined,
  model: string | undefined,
) {
  if (provider === "assemblyai" && model === "universal") {
    return "universal-3-pro";
  }

  if (provider === "soniox") {
    const alias = model?.match(/^stt-(?:async-|rt-)?v([3-5])$/);
    if (alias) {
      const version = alias[1] === "3" ? "4" : alias[1];
      return `stt-rt-v${version}`;
    }
  }

  return model;
}

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

  const sonioxRealtimeAlias = savedModel?.match(
    /^stt-(?:async-|rt-)?v([3-5])$/,
  );
  if (sonioxRealtimeAlias) {
    const version =
      sonioxRealtimeAlias[1] === "3" ? "4" : sonioxRealtimeAlias[1];
    const realtimeModel = `stt-rt-v${version}`;
    if (models.some((model) => model.id === realtimeModel)) {
      return realtimeModel;
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
