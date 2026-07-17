export function getVisibleModelSelection(
  provider: string | undefined,
  model: string | undefined,
  providerConfigured: boolean,
) {
  if (!provider || !providerConfigured) {
    return { provider: "", model: "" };
  }

  return { provider, model: model ?? "" };
}

export function getConfiguredProviders<T extends { id: string }>(
  providers: readonly T[],
  statuses: Record<string, { configured: boolean } | undefined>,
) {
  return providers.filter(({ id }) => statuses[id]?.configured === true);
}

export function getConfiguredProviderIds<T extends { id: string }>(
  providers: readonly T[],
  statuses: Record<string, { configured: boolean } | undefined>,
  preferredProvider?: string,
) {
  const providerIds = getConfiguredProviders(providers, statuses).map(
    ({ id }) => id,
  );

  if (!preferredProvider || !providerIds.includes(preferredProvider)) {
    return providerIds;
  }

  return [
    preferredProvider,
    ...providerIds.filter((providerId) => providerId !== preferredProvider),
  ];
}
