import * as stylex from "@stylexjs/stylex";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { authStyles } from "@/components/auth-shell";
import { updateUserEmail } from "@/functions/auth";
import { getSupabaseBrowserClient } from "@/functions/supabase";

import { accountSessionQueryKey, useAccountSession } from "./-account-session";
import { accountStyles } from "./-account-ui";
const styles = stylex.create({
  style1: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  style2: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#756b5d",
  },
  style3: {
    display: "flex",
    width: "100%",
    flexDirection: "column",
    gap: ".75rem",
    maxWidth: {
      default: null,
      "@media (width >= 48rem)": "420px",
    },
  },
  style4: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#dc2626",
  },
  style5: {
    display: "flex",
    justifyContent: {
      default: "flex-start",
      "@media (width >= 48rem)": "flex-end",
    },
    gap: ".5rem",
  },
  style6: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: ".75rem",
  },
  style7: {
    fontSize: "1rem",
    lineHeight: "1.5rem",
    color: "#181613",
  },
  style8: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#4f4940",
  },
  style9: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    borderColor: "#ede7dc",
    paddingTop: "1rem",
  },
  style10: {
    display: "flex",
    flexDirection: {
      default: "column",
      "@media (width >= 48rem)": "row",
    },
    gap: ".75rem",
    alignItems: {
      default: null,
      "@media (width >= 48rem)": "center",
    },
    justifyContent: {
      default: null,
      "@media (width >= 48rem)": "space-between",
    },
  },
  style11: {
    fontSize: "1rem",
    lineHeight: "1.5rem",
    color: "#181613",
    textDecorationLine: "underline",
    textDecorationColor: "#d9cdb8",
    textUnderlineOffset: "4px",
  },
  style12: {
    fontSize: "1rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style13: {
    display: "flex",
    flexDirection: {
      default: "column",
      "@media (width >= 48rem)": "row",
    },
    gap: ".75rem",
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    borderColor: "#ede7dc",
    paddingTop: "1rem",
    alignItems: {
      default: null,
      "@media (width >= 48rem)": "center",
    },
    justifyContent: {
      default: null,
      "@media (width >= 48rem)": "space-between",
    },
  },
  style14: {
    display: "flex",
    flexDirection: {
      default: "column",
      "@media (width >= 48rem)": "row",
    },
    gap: ".5rem",
    alignItems: {
      default: null,
      "@media (width >= 48rem)": "center",
    },
    justifyContent: {
      default: null,
      "@media (width >= 48rem)": "space-between",
    },
  },
  cardPadding: {
    padding: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2rem",
    },
  },
  emailRow: {
    display: "flex",
    flexDirection: {
      default: "column",
      "@media (width >= 48rem)": "row",
    },
    gap: ".75rem",
    justifyContent: {
      default: null,
      "@media (width >= 48rem)": "space-between",
    },
  },
  emailRowEditing: {
    alignItems: {
      default: null,
      "@media (width >= 48rem)": "flex-start",
    },
  },
  emailRowIdle: {
    alignItems: {
      default: null,
      "@media (width >= 48rem)": "center",
    },
  },
  detailsInput: {
    maxWidth: {
      default: null,
      "@media (width >= 48rem)": "420px",
    },
  },
});
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
      void queryClient.invalidateQueries({
        queryKey: accountSessionQueryKey,
      });
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
      const res = await updateUserEmail({
        data: {
          email,
        },
      });
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
    <div {...stylex.props(accountStyles.card, styles.cardPadding)}>
      <div {...stylex.props(styles.style1)}>
        <div
          {...stylex.props([
            styles.emailRow,
            isEditing ? styles.emailRowEditing : styles.emailRowIdle,
          ])}
        >
          <span {...stylex.props(styles.style2)}>Email</span>
          {isEditing ? (
            <form onSubmit={handleSubmit} {...stylex.props(styles.style3)}>
              <input
                type="email"
                value={newEmail}
                onChange={(e) => setNewEmail(e.target.value)}
                placeholder={email || "Enter new email"}
                {...stylex.props(authStyles.input)}
                autoFocus
              />
              {updateEmailMutation.isError && (
                <p {...stylex.props(styles.style4)}>
                  {updateEmailMutation.error?.message ||
                    "Failed to update email"}
                </p>
              )}
              <div {...stylex.props(styles.style5)}>
                <button
                  type="submit"
                  disabled={
                    updateEmailMutation.isPending ||
                    !newEmail ||
                    newEmail === email
                  }
                  {...stylex.props([
                    accountStyles.pill,
                    accountStyles.pillPrimary,
                  ])}
                >
                  {updateEmailMutation.isPending ? "Saving..." : "Save"}
                </button>
                <button
                  type="button"
                  onClick={handleCancel}
                  disabled={updateEmailMutation.isPending}
                  {...stylex.props([
                    accountStyles.pill,
                    accountStyles.pillSecondary,
                  ])}
                >
                  Cancel
                </button>
              </div>
            </form>
          ) : (
            <div {...stylex.props(styles.style6)}>
              <span {...stylex.props(styles.style7)}>
                {email || "Not available"}
              </span>
              <button
                onClick={() => {
                  setIsEditing(true);
                  setSuccessMessage(null);
                }}
                {...stylex.props([
                  accountStyles.pill,
                  accountStyles.pillSecondary,
                ])}
              >
                Change
              </button>
            </div>
          )}
        </div>

        {successMessage && (
          <div {...stylex.props(authStyles.notice)}>
            <p {...stylex.props(styles.style8)}>{successMessage}</p>
          </div>
        )}

        <div {...stylex.props(styles.style9)}>
          {isEditingDetails ? (
            <form
              onSubmit={handleDetailsSubmit}
              {...stylex.props(styles.style1)}
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
                <p {...stylex.props(styles.style4)}>
                  {updateDetailsMutation.error?.message ||
                    "Failed to update profile"}
                </p>
              )}
              <div {...stylex.props(styles.style5)}>
                <button
                  type="submit"
                  disabled={updateDetailsMutation.isPending}
                  {...stylex.props([
                    accountStyles.pill,
                    accountStyles.pillPrimary,
                  ])}
                >
                  {updateDetailsMutation.isPending ? "Saving..." : "Save"}
                </button>
                <button
                  type="button"
                  onClick={() => setIsEditingDetails(false)}
                  disabled={updateDetailsMutation.isPending}
                  {...stylex.props([
                    accountStyles.pill,
                    accountStyles.pillSecondary,
                  ])}
                >
                  Cancel
                </button>
              </div>
            </form>
          ) : (
            <>
              <div {...stylex.props(styles.style10)}>
                <span {...stylex.props(styles.style2)}>Full name</span>
                <div {...stylex.props(styles.style6)}>
                  <span {...stylex.props(styles.style7)}>
                    {profile?.fullName || "Not set"}
                  </span>
                  <button
                    onClick={startEditingDetails}
                    {...stylex.props([
                      accountStyles.pill,
                      accountStyles.pillSecondary,
                    ])}
                  >
                    Edit
                  </button>
                </div>
              </div>
              <div {...stylex.props(styles.style10)}>
                <span {...stylex.props(styles.style2)}>LinkedIn</span>
                {profile?.linkedinUrl ? (
                  <a
                    href={profile.linkedinUrl}
                    target="_blank"
                    rel="noreferrer"
                    {...stylex.props(styles.style11)}
                  >
                    {profile.linkedinUrl.replace(/^https?:\/\/(www\.)?/, "")}
                  </a>
                ) : (
                  <span {...stylex.props(styles.style12)}>Not set</span>
                )}
              </div>
              <div {...stylex.props(styles.style10)}>
                <span {...stylex.props(styles.style2)}>X</span>
                {profile?.xHandle ? (
                  <a
                    href={`https://x.com/${profile.xHandle}`}
                    target="_blank"
                    rel="noreferrer"
                    {...stylex.props(styles.style11)}
                  >
                    @{profile.xHandle}
                  </a>
                ) : (
                  <span {...stylex.props(styles.style12)}>Not set</span>
                )}
              </div>
            </>
          )}
        </div>

        {accountSession?.createdAt && (
          <div {...stylex.props(styles.style13)}>
            <span {...stylex.props(styles.style2)}>Member since</span>
            <span {...stylex.props(styles.style7)}>
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
    <label {...stylex.props(styles.style14)}>
      <span {...stylex.props(styles.style2)}>{label}</span>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        {...stylex.props(authStyles.input, styles.detailsInput)}
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
