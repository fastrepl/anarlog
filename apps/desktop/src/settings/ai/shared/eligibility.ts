export type ConfigField = "base_url" | "api_key";

export type ProviderRequirement =
  | { kind: "requires_auth" }
  | { kind: "requires_entitlement"; entitlement: "pro" }
  | {
      kind: "requires_config";
      fields: ConfigField[];
      // Fields the form still shows, but that may stay empty, such as the API
      // key of a Custom endpoint that runs without authentication.
      optional?: ConfigField[];
    }
  | { kind: "requires_platform"; platform: "apple_silicon" };

export function requiresEntitlement(
  requirements: readonly ProviderRequirement[],
  entitlement: "pro",
): boolean {
  return requirements.some(
    (r) => r.kind === "requires_entitlement" && r.entitlement === entitlement,
  );
}

export function getRequiredConfigFields(
  requirements: readonly ProviderRequirement[],
): ConfigField[] {
  const req = requirements.find((r) => r.kind === "requires_config");
  return req?.kind === "requires_config" ? req.fields : [];
}

export function getMandatoryConfigFields(
  requirements: readonly ProviderRequirement[],
): ConfigField[] {
  const req = requirements.find((r) => r.kind === "requires_config");
  if (req?.kind !== "requires_config") return [];
  const optional = new Set(req.optional ?? []);
  return req.fields.filter((field) => !optional.has(field));
}

export type ProviderEligibilityContext = {
  isAuthenticated: boolean;
  isPaid: boolean;
  config?: { base_url?: string; api_key?: string };
};

export function getProviderSelectionBlockers(
  requirements: readonly ProviderRequirement[],
  context: ProviderEligibilityContext,
): EligibilityBlocker[] {
  const blockers: EligibilityBlocker[] = [];

  for (const req of requirements) {
    switch (req.kind) {
      case "requires_auth":
        if (!context.isAuthenticated) {
          blockers.push({ code: "requires_auth" });
        }
        break;
      case "requires_entitlement":
        if (req.entitlement === "pro" && !context.isPaid) {
          blockers.push({ code: "requires_entitlement", entitlement: "pro" });
        }
        break;
      case "requires_config": {
        const optional = new Set(req.optional ?? []);
        const missingFields = req.fields.filter((field) => {
          if (optional.has(field)) return false;
          const value = context.config?.[field];
          return !value || value.trim() === "";
        });
        if (missingFields.length > 0) {
          blockers.push({ code: "missing_config", fields: missingFields });
        }
        break;
      }
      case "requires_platform":
        break;
    }
  }

  return blockers;
}

export type EligibilityBlocker =
  | { code: "missing_provider" }
  | { code: "missing_model" }
  | { code: "provider_disabled" }
  | { code: "requires_auth" }
  | { code: "requires_entitlement"; entitlement: "pro" }
  | { code: "missing_config"; fields: Array<"base_url" | "api_key"> }
  | { code: "model_not_downloaded"; modelId: string }
  | { code: "unsupported_platform"; required: "apple_silicon" };
