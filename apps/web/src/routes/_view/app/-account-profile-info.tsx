import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { cn } from "@anlg/utils";

import {
  authInputClassName,
  authNoticeClassName,
} from "@/components/auth-shell";
import { updateUserEmail } from "@/functions/auth";
import { getSupabaseBrowserClient } from "@/functions/supabase";

import { accountSessionQueryKey, useAccountSession } from "./-account-session";
import {
  accountCardClassName,
  accountPillPrimaryClassName,
  accountPillSecondaryClassName,
} from "./-account-ui";

export function ProfileInfoSection({ email }: { email?: string }) {
  const [isEditing, setIsEditing] = useState(false);
  const [newEmail, setNewEmail] = useState("");
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [isEditingDetails, setIsEditingDetails] = useState(false);
  const [draftName, setDraftName] = useState("");
  const [draftLinkedin, setDraftLinkedin] = useState("");
  const [draftX, setDraftX] = useState("");
  const { data: accountSession } = useAccountSession();
  const queryClient = useQueryClient();
  const profile = accountSession?.profile;

  const updateDetailsMutation = useMutation({
    mutationFn: async (details: {
      fullName: string | null;
      linkedinUrl: string | null;
      xHandle: string | null;
    }) => {
      const supabase = getSupabaseBrowserClient();
      const { error } = await supabase.auth.updateUser({
        data: {
          full_name: details.fullName,
          linkedin_url: details.linkedinUrl,
          x_handle: details.xHandle,
        },
      });
      if (error) {
        throw new Error(error.message);
      }
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: accountSessionQueryKey });
      setIsEditingDetails(false);
    },
  });

  const startEditingDetails = () => {
    setDraftName(profile?.fullName ?? "");
    setDraftLinkedin(profile?.linkedinUrl ?? "");
    setDraftX(profile?.xHandle ?? "");
    updateDetailsMutation.reset();
    setIsEditingDetails(true);
  };

  const handleDetailsSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    updateDetailsMutation.mutate({
      fullName: draftName.trim() || null,
      linkedinUrl: normalizeLinkedinUrl(draftLinkedin),
      xHandle: normalizeXHandle(draftX),
    });
  };

  const updateEmailMutation = useMutation({
    mutationFn: async (email: string) => {
      const res = await updateUserEmail({ data: { email } });
      if ("error" in res && res.error) {
        throw new Error(res.error);
      }
      return res;
    },
    onSuccess: (data) => {
      if ("message" in data && data.message) {
        setSuccessMessage(data.message);
      }
      setIsEditing(false);
      setNewEmail("");
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (newEmail && newEmail !== email) {
      updateEmailMutation.mutate(newEmail);
    }
  };

  const handleCancel = () => {
    setIsEditing(false);
    setNewEmail("");
    updateEmailMutation.reset();
  };

  return (
    <div className={cn([accountCardClassName, "p-6 sm:p-8"])}>
      <div className="flex flex-col gap-4">
        <div
          className={cn([
            "flex flex-col gap-3 md:flex-row md:justify-between",
            isEditing ? "md:items-start" : "md:items-center",
          ])}
        >
          <span className="text-sm font-medium text-[#756b5d]">Email</span>
          {isEditing ? (
            <form
              onSubmit={handleSubmit}
              className="flex w-full flex-col gap-3 md:max-w-[420px]"
            >
              <input
                type="email"
                value={newEmail}
                onChange={(e) => setNewEmail(e.target.value)}
                placeholder={email || "Enter new email"}
                className={authInputClassName}
                autoFocus
              />
              {updateEmailMutation.isError && (
                <p className="text-sm text-red-600">
                  {updateEmailMutation.error?.message ||
                    "Failed to update email"}
                </p>
              )}
              <div className="flex justify-start gap-2 md:justify-end">
                <button
                  type="submit"
                  disabled={
                    updateEmailMutation.isPending ||
                    !newEmail ||
                    newEmail === email
                  }
                  className={accountPillPrimaryClassName}
                >
                  {updateEmailMutation.isPending ? "Saving..." : "Save"}
                </button>
                <button
                  type="button"
                  onClick={handleCancel}
                  disabled={updateEmailMutation.isPending}
                  className={accountPillSecondaryClassName}
                >
                  Cancel
                </button>
              </div>
            </form>
          ) : (
            <div className="flex flex-wrap items-center gap-3">
              <span className="text-base text-[#181613]">
                {email || "Not available"}
              </span>
              <button
                onClick={() => {
                  setIsEditing(true);
                  setSuccessMessage(null);
                }}
                className={accountPillSecondaryClassName}
              >
                Change
              </button>
            </div>
          )}
        </div>

        {successMessage && (
          <div className={authNoticeClassName}>
            <p className="text-sm font-medium text-[#4f4940]">
              {successMessage}
            </p>
          </div>
        )}

        <div className="flex flex-col gap-4 border-t border-[#ede7dc] pt-4">
          {isEditingDetails ? (
            <form
              onSubmit={handleDetailsSubmit}
              className="flex flex-col gap-4"
            >
              <DetailsField
                label="Full name"
                value={draftName}
                onChange={setDraftName}
                placeholder="Ada Lovelace"
              />
              <DetailsField
                label="LinkedIn"
                value={draftLinkedin}
                onChange={setDraftLinkedin}
                placeholder="linkedin.com/in/your-name"
              />
              <DetailsField
                label="X"
                value={draftX}
                onChange={setDraftX}
                placeholder="@yourhandle"
              />
              {updateDetailsMutation.isError && (
                <p className="text-sm text-red-600">
                  {updateDetailsMutation.error?.message ||
                    "Failed to update profile"}
                </p>
              )}
              <div className="flex justify-start gap-2 md:justify-end">
                <button
                  type="submit"
                  disabled={updateDetailsMutation.isPending}
                  className={accountPillPrimaryClassName}
                >
                  {updateDetailsMutation.isPending ? "Saving..." : "Save"}
                </button>
                <button
                  type="button"
                  onClick={() => setIsEditingDetails(false)}
                  disabled={updateDetailsMutation.isPending}
                  className={accountPillSecondaryClassName}
                >
                  Cancel
                </button>
              </div>
            </form>
          ) : (
            <>
              <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                <span className="text-sm font-medium text-[#756b5d]">
                  Full name
                </span>
                <div className="flex flex-wrap items-center gap-3">
                  <span className="text-base text-[#181613]">
                    {profile?.fullName || "Not set"}
                  </span>
                  <button
                    onClick={startEditingDetails}
                    className={accountPillSecondaryClassName}
                  >
                    Edit
                  </button>
                </div>
              </div>
              <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                <span className="text-sm font-medium text-[#756b5d]">
                  LinkedIn
                </span>
                {profile?.linkedinUrl ? (
                  <a
                    href={profile.linkedinUrl}
                    target="_blank"
                    rel="noreferrer"
                    className="text-base text-[#181613] underline decoration-[#d9cdb8] underline-offset-4"
                  >
                    {profile.linkedinUrl.replace(/^https?:\/\/(www\.)?/, "")}
                  </a>
                ) : (
                  <span className="text-base text-[#756b5d]">Not set</span>
                )}
              </div>
              <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                <span className="text-sm font-medium text-[#756b5d]">X</span>
                {profile?.xHandle ? (
                  <a
                    href={`https://x.com/${profile.xHandle}`}
                    target="_blank"
                    rel="noreferrer"
                    className="text-base text-[#181613] underline decoration-[#d9cdb8] underline-offset-4"
                  >
                    @{profile.xHandle}
                  </a>
                ) : (
                  <span className="text-base text-[#756b5d]">Not set</span>
                )}
              </div>
            </>
          )}
        </div>

        {accountSession?.createdAt && (
          <div className="flex flex-col gap-3 border-t border-[#ede7dc] pt-4 md:flex-row md:items-center md:justify-between">
            <span className="text-sm font-medium text-[#756b5d]">
              Member since
            </span>
            <span className="text-base text-[#181613]">
              {new Date(accountSession.createdAt).toLocaleDateString("en-US", {
                month: "long",
                year: "numeric",
              })}
            </span>
          </div>
        )}
      </div>
    </div>
  );
}

function DetailsField({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
}) {
  return (
    <label className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
      <span className="text-sm font-medium text-[#756b5d]">{label}</span>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className={cn([authInputClassName, "md:max-w-[420px]"])}
      />
    </label>
  );
}

function normalizeLinkedinUrl(input: string): string | null {
  const trimmed = input.trim();
  if (!trimmed) {
    return null;
  }
  const bare = trimmed
    .replace(/^https?:\/\//i, "")
    .replace(/^www\./i, "")
    .replace(/\/+$/, "");
  if (/^linkedin\.com\//i.test(bare)) {
    return `https://www.${bare}`;
  }
  const handle = bare.replace(/^@/, "");
  return `https://www.linkedin.com/in/${handle}`;
}

function normalizeXHandle(input: string): string | null {
  const trimmed = input.trim();
  if (!trimmed) {
    return null;
  }
  const handle = trimmed
    .replace(/^(https?:\/\/)?(www\.)?(x\.com|twitter\.com)\//i, "")
    .replace(/^@/, "")
    .replace(/\/+$/, "");
  return handle || null;
}
