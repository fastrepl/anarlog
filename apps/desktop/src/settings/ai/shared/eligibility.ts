export type ConfigField = "base_url" | "api_key";

export type ProviderRequirement =
  | { kind: "requires_auth" }
  | { kind: "requires_entitlement"; entitlement: "pro" }
  | { kind: "requires_config"; fields: ConfigField[] }
  | { kind: "requires_platform"; platform: "apple_silicon" };

export function requiresEntitlement(
  requirements: readonly ProviderRequirement[],
  entitlement: "pro",
): boolean {
  return requirements.some(
    (r) => r.kind === "requires_entitlement" && r.entitlement === entitlement,
  );
}

export function requiresConfigField(
  requirements: readonly ProviderRequirement[],
  field: ConfigField,
): boolean {
  return requirements.some(
    (r) => r.kind === "requires_config" && r.fields.includes(field),
  );
}

export function getRequiredConfigFields(
  requirements: readonly ProviderRequirement[],
): ConfigField[] {
  const req = requirements.find((r) => r.kind === "requires_config");
  return req?.kind === "requires_config" ? req.fields : [];
}

export type ProviderEligibilityContext = {
  isAuthenticated: boolean;
  isPro: boolean;
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
        if (req.entitlement === "pro" && !context.isPro) {
          blockers.push({ code: "requires_entitlement", entitlement: "pro" });
        }
        break;
      case "requires_config": {
        const missingFields = req.fields.filter((field) => {
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

export type ModelRequirement =
  | { kind: "requires_download" }
  | { kind: "requires_entitlement"; entitlement: "pro" }
  | { kind: "requires_platform"; platform: "apple_silicon" };

export type EligibilityBlocker =
  | { code: "missing_provider" }
  | { code: "missing_model" }
  | { code: "provider_disabled" }
  | { code: "requires_auth" }
  | { code: "requires_entitlement"; entitlement: "pro" }
  | { code: "missing_config"; fields: Array<"base_url" | "api_key"> }
  | { code: "model_not_downloaded"; modelId: string }
  | { code: "unsupported_platform"; required: "apple_silicon" };

export type EligibilityAction =
  | { kind: "sign_in" }
  | { kind: "upgrade_to_pro" }
  | { kind: "open_provider_settings"; providerId: string }
  | { kind: "download_model"; modelId: string };

export function getActionForBlocker(
  blocker: EligibilityBlocker,
  providerId?: string,
): EligibilityAction | null {
  switch (blocker.code) {
    case "requires_auth":
      return { kind: "sign_in" };
    case "requires_entitlement":
      return { kind: "upgrade_to_pro" };
    case "missing_config":
      return providerId ? { kind: "open_provider_settings", providerId } : null;
    case "model_not_downloaded":
      return { kind: "download_model", modelId: blocker.modelId };
    default:
      return null;
  }
}
