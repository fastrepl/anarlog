import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";

import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@anlg/ui/components/ui/accordion";
import { cn } from "@anlg/utils";

import { deleteAccount } from "@/functions/billing";
import { captureOperationalError } from "@/lib/error-reporting";

import {
  accountCardClassName,
  accountPillDangerClassName,
} from "./-account-ui";

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
    <div className={accountCardClassName}>
      <div className="px-6 py-2 sm:px-8">
        <Accordion
          type="single"
          collapsible
          onValueChange={(value) => {
            if (!value) {
              setShowDeleteConfirm(false);
              deleteAccountMutation.reset();
            }
          }}
        >
          <AccordionItem value="delete-account" className="border-none">
            <AccordionTrigger className="py-4 font-sans text-base font-medium text-red-700 hover:text-red-800 hover:no-underline">
              Delete account
            </AccordionTrigger>
            <AccordionContent className="pb-6">
              <div className="rounded-xl border border-red-200 bg-red-50 p-4">
                <p className="text-sm leading-6 text-red-900">
                  Anarlog is a local-first app. Your notes, transcripts, and
                  meeting data stay on your device. Deleting your account only
                  removes cloud-stored data.
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
            </AccordionContent>
          </AccordionItem>
        </Accordion>
      </div>
    </div>
  );
}
