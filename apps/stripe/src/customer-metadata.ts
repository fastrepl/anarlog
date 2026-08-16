const OWNER_METADATA_KEYS = ["userId", "user_id", "userID"] as const;
const WORKSPACE_METADATA_KEYS = ["workspaceId", "workspace_id"] as const;

export function getCustomerUserId(metadata: Record<string, string> | null) {
  return getConsistentMetadataValue(metadata, OWNER_METADATA_KEYS);
}

export function getCustomerWorkspaceId(
  metadata: Record<string, string> | null,
) {
  return getConsistentMetadataValue(metadata, WORKSPACE_METADATA_KEYS);
}

export function getCustomerOwner(metadata: Record<string, string> | null) {
  const hasUserOwner = OWNER_METADATA_KEYS.some((key) =>
    Boolean(metadata?.[key]),
  );
  const hasWorkspaceOwner = WORKSPACE_METADATA_KEYS.some((key) =>
    Boolean(metadata?.[key]),
  );

  if (hasUserOwner === hasWorkspaceOwner) {
    return null;
  }

  const userId = getCustomerUserId(metadata);
  const workspaceId = getCustomerWorkspaceId(metadata);

  if (userId && !workspaceId) {
    return { kind: "user" as const, id: userId };
  }

  if (workspaceId && !userId) {
    return { kind: "workspace" as const, id: workspaceId };
  }

  return null;
}

function getConsistentMetadataValue(
  metadata: Record<string, string> | null,
  keys: readonly string[],
) {
  const values = keys
    .map((key) => metadata?.[key])
    .filter((value): value is string => Boolean(value));

  if (values.length === 0 || values.some((value) => value !== values[0])) {
    return null;
  }

  return values[0];
}

export function getCustomerIdentityMetadata(
  metadata: Record<string, string> | null,
  userId: string,
) {
  if (
    metadata?.["userId"] === userId &&
    metadata["posthog_person_distinct_id"] === userId
  ) {
    return null;
  }

  return {
    userId,
    posthog_person_distinct_id: userId,
  };
}
