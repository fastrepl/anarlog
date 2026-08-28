import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";

import { cn } from "@anlg/utils";

import { deleteAccount } from "@/functions/billing";
import { captureOperationalError } from "@/lib/error-reporting";

import { accountPillDangerClassName } from "./-account-ui";

export function DangerAreaSection() {
  const navigate = useNavigate();
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  const deleteAccountMutation = useMutation({
    mutationFn: () => deleteAccount(),
    onSuccess: () => {
      navigate({ to: "/" });
    },
    onError: (error) => {
      captureOperationalError(error, {
        operation: "account_delete",
      });
    },
  });

  return (
    <div
      className={cn([
        "overflow-hidden rounded-[24px] border border-red-200 bg-red-50",
        "shadow-[0_18px_50px_rgba(24,22,19,0.08)]",
        "p-6 sm:p-8",
      ])}
    >
      <p className="text-base font-medium text-red-900">Delete account</p>
      <p className="mt-3 text-sm leading-6 text-red-900">
        Anarlog is a local-first app. Your notes, transcripts, and meeting data
        stay on your device. Deleting your account only removes cloud-stored
        data.
      </p>

      {showDeleteConfirm ? (
        <div className="mt-4 space-y-3">
          <p className="text-sm text-red-800">
            This permanently deletes your account and cloud data.
          </p>

          {deleteAccountMutation.isError && (
            <p className="text-sm text-red-600">
              {deleteAccountMutation.error?.message ||
                "Failed to delete account"}
            </p>
          )}

          <div className="flex flex-wrap gap-2">
            <button
              onClick={() => deleteAccountMutation.mutate()}
              disabled={deleteAccountMutation.isPending}
              className="flex h-9 cursor-pointer items-center justify-center rounded-full bg-red-700 px-4 text-sm font-medium text-white transition-colors hover:bg-red-800 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {deleteAccountMutation.isPending
                ? "Deleting..."
                : "Yes, delete my account"}
            </button>
            <button
              onClick={() => {
                setShowDeleteConfirm(false);
                deleteAccountMutation.reset();
              }}
              disabled={deleteAccountMutation.isPending}
              className={accountPillDangerClassName}
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <button
          onClick={() => setShowDeleteConfirm(true)}
          className={cn([accountPillDangerClassName, "mt-4"])}
        >
          Continue
        </button>
      )}
    </div>
  );
}
