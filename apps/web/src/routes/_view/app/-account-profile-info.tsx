import { useMutation } from "@tanstack/react-query";
import { useState } from "react";

import { cn } from "@anlg/utils";

import {
  authInputClassName,
  authNoticeClassName,
} from "@/components/auth-shell";
import { updateUserEmail } from "@/functions/auth";

import { useAccountSession } from "./-account-session";
import {
  accountCardClassName,
  accountPillPrimaryClassName,
  accountPillSecondaryClassName,
} from "./-account-ui";

export function ProfileInfoSection({ email }: { email?: string }) {
  const [isEditing, setIsEditing] = useState(false);
  const [newEmail, setNewEmail] = useState("");
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const { data: accountSession } = useAccountSession();

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
