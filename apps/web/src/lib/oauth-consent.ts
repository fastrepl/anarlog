const scopeDescriptions: Record<string, string> = {
  openid: "Confirm your Anarlog account identity",
  email: "Share your Anarlog account email with the connector",
};

export function describeOAuthScopes(scope: string) {
  return scope
    .split(/\s+/)
    .filter(Boolean)
    .map(
      (value) => scopeDescriptions[value] ?? `Grant the ${value} permission`,
    );
}
