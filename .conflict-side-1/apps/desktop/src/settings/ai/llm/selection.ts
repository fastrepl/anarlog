type PreferredProviderModelOptions = {
  allowSavedModelWithoutChoices?: boolean;
};

export function getPreferredProviderModel(
  savedModel: string | undefined,
  models: string[],
  options?: PreferredProviderModelOptions,
) {
  if (savedModel && models.includes(savedModel)) {
    return savedModel;
  }

  if (models.length > 0) {
    return models[0];
  }

  if (options?.allowSavedModelWithoutChoices) {
    return savedModel ?? "";
  }

  return "";
}

export async function getDefaultLlmSelection(
  providerIds: readonly string[],
  currentProvider: string | undefined,
  currentModel: string | undefined,
  loadModels: (provider: string) => Promise<string[]>,
) {
  for (const provider of providerIds) {
    try {
      const models = await loadModels(provider);
      const model = getPreferredProviderModel(
        provider === currentProvider ? currentModel : undefined,
        models,
        { allowSavedModelWithoutChoices: provider === "custom" },
      );

      if (model) {
        return { provider, model };
      }
    } catch {
      continue;
    }
  }

  return null;
}
