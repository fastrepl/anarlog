import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";

import { cn } from "@anlg/utils";

import { deleteAccount } from "@/functions/billing";
import { captureOperationalError } from "@/lib/error-reporting";

import { accountStyles } from "./-account-ui";
const styles = stylex.create({
  style1: {
    fontSize: "1rem",
    lineHeight: "1.5rem",
    "--tw-font-weight": "500",
    fontWeight: "500",
    color: "#7f1d1d",
  },
  style2: {
    marginTop: ".75rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    "--tw-leading": "1.5rem",
    color: "#7f1d1d",
  },
  style3: {
    marginTop: "1rem",
  },
  style4: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#991b1b",
  },
  style5: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#dc2626",
  },
  style6: {
    display: "flex",
    flexWrap: "wrap",
    gap: ".5rem",
  },
  style7: {
    display: "flex",
    height: "2.25rem",
    cursor: {
      default: "pointer",
      ":disabled": "not-allowed",
    },
    alignItems: "center",
    justifyContent: "center",
    borderRadius: "3.40282e38px",
    backgroundColor: {
      default: "#b91c1c",
      ":hover": "#991b1b",
    },
    paddingInline: "1rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    "--tw-font-weight": "500",
    fontWeight: "500",
    color: "#fff",
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, --tw-gradient-from, --tw-gradient-via, --tw-gradient-to",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
    opacity: {
      default: null,
      ":disabled": ".5",
    },
  },
});
export function DangerAreaSection() {
  const navigate = useNavigate();
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const deleteAccountMutation = useMutation({
    mutationFn: () => deleteAccount(),
    onSuccess: () => {
      navigate({
        to: "/",
      });
    },
    onError: (error) => {
      captureOperationalError(error, {
        operation: "account_delete",
      });
    },
  });
  return (
    <div
      {...stylex.props([
        "overflow-hidden rounded-[24px] border border-red-200 bg-red-50",
        "shadow-[0_18px_50px_rgba(24,22,19,0.08)]",
        "p-6 sm:p-8",
      ])}
    >
      <p {...stylex.props(styles.style1)}>Delete account</p>
      <p {...stylex.props(styles.style2)}>
        Anarlog is a local-first app. Your notes, transcripts, and meeting data
        stay on your device. Deleting your account only removes cloud-stored
        data.
      </p>

      {showDeleteConfirm ? (
        <div {...stylex.props(styles.style3)}>
          <p {...stylex.props(styles.style4)}>
            This permanently deletes your account and cloud data.
          </p>

          {deleteAccountMutation.isError && (
            <p {...stylex.props(styles.style5)}>
              {deleteAccountMutation.error?.message ||
                "Failed to delete account"}
            </p>
          )}

          <div {...stylex.props(styles.style6)}>
            <button
              onClick={() => deleteAccountMutation.mutate()}
              disabled={deleteAccountMutation.isPending}
              {...stylex.props(styles.style7)}
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
              {...stylex.props([accountStyles.pill, accountStyles.pillDanger])}
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <button
          onClick={() => setShowDeleteConfirm(true)}
          {...stylex.props([
            [accountStyles.pill, accountStyles.pillDanger],
            "mt-4",
          ])}
        >
          Continue
        </button>
      )}
    </div>
  );
}
