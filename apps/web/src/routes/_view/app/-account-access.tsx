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
    <div className="border-color-brand surface rounded-lg border">
      <div className="px-8 pt-8">
        <h3 className="text-color mb-2 font-sans text-lg font-semibold">
          Access
        </h3>
        <p className="text-color-secondary text-sm">
          Session controls and destructive account actions
        </p>
      </div>

      <div className="border-color-brand flex w-full flex-col border-b p-8">
        <div className="flex flex-col gap-4 md:flex-row md:justify-between">
          <div className="text-color-secondary text-base">Sign out</div>
          <div className="flex items-center gap-3">
            <p className="text-base">End your current session on this device</p>
            <button
              onClick={() => signOut.mutate()}
              disabled={signOut.isPending}
              className="border-color-brand text-color-secondary flex h-7 items-center rounded-full border bg-linear-to-b from-white to-stone-50 px-3 text-xs shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%] disabled:opacity-50 disabled:hover:scale-100"
            >
              {signOut.isPending ? "Signing out..." : "Sign out"}
            </button>
          </div>
        </div>
      </div>

      <div className="px-8 py-8">
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
            <AccordionTrigger className="py-4 font-sans text-base text-red-700 hover:text-red-800 hover:no-underline">
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
                        className="flex h-8 items-center rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-4 text-sm text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%] disabled:opacity-50 disabled:hover:scale-100"
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
