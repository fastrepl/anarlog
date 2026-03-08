import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";

import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@hypr/ui/components/ui/accordion";

import { signOutFn } from "@/functions/auth";
import { deleteAccount } from "@/functions/billing";

export function AccountAccessSection() {
  const navigate = useNavigate();
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  const signOut = useMutation({
    mutationFn: async () => {
      const res = await signOutFn();
      if (res.success) {
        return true;
      }

      throw new Error(res.message);
    },
    onSuccess: () => {
      navigate({ to: "/" });
    },
    onError: (error) => {
      console.error(error);
      navigate({ to: "/" });
    },
  });

  const deleteAccountMutation = useMutation({
    mutationFn: () => deleteAccount(),
    onSuccess: () => {
      navigate({ to: "/" });
    },
  });

  return (
    <div className="rounded-xs border border-neutral-100">
      <div className="p-4">
        <h3 className="mb-2 font-serif text-lg font-semibold">Access</h3>
        <p className="text-sm text-neutral-600">
          Session controls and destructive account actions
        </p>
      </div>

      <div className="flex flex-col gap-4 border-t border-neutral-100 p-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <div className="text-sm font-medium text-neutral-900">Sign out</div>
          <p className="text-sm text-neutral-600">
            End your current session on this device
          </p>
        </div>

        <button
          onClick={() => signOut.mutate()}
          disabled={signOut.isPending}
          className="flex h-8 cursor-pointer items-center justify-center rounded-full border border-neutral-300 bg-linear-to-b from-white to-stone-50 px-4 text-sm text-neutral-700 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%] disabled:opacity-50 disabled:hover:scale-100"
        >
          {signOut.isPending ? "Signing out..." : "Sign out"}
        </button>
      </div>

      <div className="border-t border-neutral-100 px-4">
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
            <AccordionTrigger className="py-4 text-sm font-medium text-red-700 hover:text-red-800 hover:no-underline">
              Delete account
            </AccordionTrigger>
            <AccordionContent className="pb-4">
              <div className="rounded-md border border-red-200 bg-red-50 p-4">
                <p className="text-sm text-red-900">
                  Char is a local-first app. Your notes, transcripts, and
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
                        className="flex h-8 items-center rounded-full bg-red-600 px-4 text-sm text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%] disabled:opacity-50 disabled:hover:scale-100"
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
                        className="flex h-8 items-center rounded-full border border-red-200 bg-white px-4 text-sm text-red-700 transition-all hover:border-red-300 hover:text-red-800 disabled:opacity-50"
                      >
                        Cancel
                      </button>
                    </div>
                  </div>
                ) : (
                  <button
                    onClick={() => setShowDeleteConfirm(true)}
                    className="mt-4 flex h-8 cursor-pointer items-center rounded-full border border-red-200 bg-white px-4 text-sm text-red-700 transition-all hover:border-red-300 hover:text-red-800"
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
