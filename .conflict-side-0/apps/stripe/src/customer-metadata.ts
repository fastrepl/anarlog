const OWNER_METADATA_KEYS = ["userId", "user_id", "userID"] as const;

export function getCustomerUserId(metadata: Record<string, string> | null) {
  const ownerIds = OWNER_METADATA_KEYS.map((key) => metadata?.[key]).filter(
    (ownerId): ownerId is string => Boolean(ownerId),
  );

  if (
    ownerIds.length === 0 ||
    ownerIds.some((ownerId) => ownerId !== ownerIds[0])
  ) {
    return null;
  }

  return ownerIds[0];
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
