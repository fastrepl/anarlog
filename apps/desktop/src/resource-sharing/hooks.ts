import { useQuery } from "@tanstack/react-query";

import {
  listSharedResources,
  requireResourceSharingContext,
  type SharedResourceType,
} from "./client";

import { useOptionalAuth } from "~/auth";

export function sharedResourcesQueryKey(
  userId: string | null | undefined,
  resourceType: SharedResourceType,
) {
  return ["shared-resources", userId ?? "", resourceType] as const;
}

export function useSharedResources(resourceType: SharedResourceType) {
  const auth = useOptionalAuth();
  const userId = auth?.session?.user.id;
  const enabled = Boolean(
    auth?.supabase && auth.session && !auth.session.user.is_anonymous,
  );
  return useQuery({
    queryKey: sharedResourcesQueryKey(userId, resourceType),
    enabled,
    retry: false,
    queryFn: () =>
      listSharedResources(
        requireResourceSharingContext(auth ?? {}),
        resourceType,
      ),
  });
}
