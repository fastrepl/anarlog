import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";

import {
  getCustomProfileImageUrl,
  getProviderProfileImageUrl,
} from "@anlg/supabase/profile";
import { saveProfileAvatar } from "@anlg/supabase/profile-avatar";
import { Button } from "@anlg/ui/components/ui/button";

import { AvatarUploadButton, ContactImage } from "./contact-avatar";
import { updateContactAvatar, usePersonalContact } from "./queries";
import { ContactFacehash } from "./shared";

import { useAuth } from "~/auth";

export function useSharedProfilePhoto(userId: string) {
  const auth = useAuth();
  const local = usePersonalContact(userId);
  const enabled = !!auth.supabase && auth.session?.user.id === userId;
  // The environment-scoped auth client is not serializable cache data.
  // eslint-disable-next-line @tanstack/query/exhaustive-deps
  const profile = useQuery({
    queryKey: ["profile-photo", userId],
    enabled,
    queryFn: async () => {
      const { data, error } = await auth.supabase!.auth.getUser();
      if (error) throw error;
      if (data.user?.id !== userId)
        throw new Error("Account changed during photo lookup");
      return data.user;
    },
    refetchInterval: 30_000,
  });
  const mirror = useMutation({
    mutationFn: (photo: string | null) =>
      updateContactAvatar("human", userId, photo),
    retry: 2,
  });
  const custom = getCustomProfileImageUrl(profile.data);
  const localExists = !!local.data;
  const localPhoto = local.data?.avatarDataUrl;
  useEffect(() => {
    if (
      enabled &&
      localExists &&
      custom !== undefined &&
      custom !== localPhoto
    ) {
      mirror.mutate(custom);
    }
  }, [
    enabled,
    custom,
    localExists,
    localPhoto,
    profile.dataUpdatedAt,
    mirror.mutate,
  ]);
  return profile;
}

export function ProfilePhoto({
  userId,
  name,
  localPhoto,
  onSave,
}: {
  userId: string;
  name: string;
  localPhoto: string | null;
  onSave: (url: string | null) => Promise<void>;
}) {
  const { t } = useLingui();
  const auth = useAuth();
  const queryClient = useQueryClient();
  const profile = useSharedProfilePhoto(userId);
  const cloud = !!auth.supabase && auth.session?.user.id === userId;
  const save = useMutation({
    scope: { id: `profile-photo:${userId}` },
    mutationFn: async (photo: string | null) => {
      if (!cloud) {
        await onSave(photo);
        return photo;
      }
      const user = await saveProfileAvatar(auth.supabase!, userId, photo);
      const url = getCustomProfileImageUrl(user) ?? null;
      queryClient.setQueryData(["profile-photo", userId], user);
      await onSave(url);
      await queryClient.invalidateQueries({ queryKey: ["team-members"] });
      return url;
    },
  });
  const migrated = useRef(false);
  // Publish legacy local uploads once, only after confirming no cloud override exists.
  useEffect(() => {
    if (
      !migrated.current &&
      cloud &&
      profile.data &&
      getCustomProfileImageUrl(profile.data) === undefined &&
      localPhoto?.startsWith("data:image/")
    ) {
      migrated.current = true;
      save.mutate(localPhoto);
    }
  }, [cloud, localPhoto, profile.data, save.mutate]);
  const custom = cloud ? getCustomProfileImageUrl(profile.data) : undefined;
  const photo =
    save.isPending || save.isError
      ? save.variables
      : custom !== undefined
        ? custom
        : ((!cloud && save.isSuccess ? save.data : localPhoto) ??
          (cloud ? getProviderProfileImageUrl(profile.data) : null));

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-4">
        <AvatarUploadButton
          label={t`Change photo`}
          onUpload={(value) => {
            migrated.current = true;
            save.mutate(value);
          }}
        >
          {photo ? (
            <ContactImage src={photo} size={64} />
          ) : (
            <ContactFacehash name={name} size={64} />
          )}
        </AvatarUploadButton>
        {photo && (
          <Button
            variant="outline"
            type="button"
            onClick={() => {
              migrated.current = true;
              save.mutate(null);
            }}
          >
            <Trans>Remove photo</Trans>
          </Button>
        )}
      </div>
      {save.isPending && (
        <p role="status" className="text-muted-foreground text-sm">
          <Trans>Saving...</Trans>
        </p>
      )}
      {(save.isError || profile.isError) && (
        <div className="flex items-center gap-3">
          <p role="alert" className="text-destructive text-sm">
            <Trans>Couldn't save your profile. Try again.</Trans>
          </p>
          <Button
            type="button"
            variant="outline"
            onClick={() =>
              save.isError
                ? save.mutate(save.variables ?? null)
                : void profile.refetch()
            }
          >
            <Trans>Retry</Trans>
          </Button>
        </div>
      )}
    </div>
  );
}
