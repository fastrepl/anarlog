import { useMutation } from "@tanstack/react-query";
import { useState } from "react";

import { updateUserEmail } from "@/functions/auth";

export function ProfileInfoSection({ email }: { email?: string }) {
  const [isEditing, setIsEditing] = useState(false);
  const [newEmail, setNewEmail] = useState("");
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

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
    <div className="rounded-xs border border-neutral-100">
      <div className="p-4">
        <h3 className="mb-2 font-serif text-lg font-semibold">Profile</h3>
        <p className="text-sm text-neutral-600">Your personal information</p>
      </div>

      <div className="flex flex-col gap-4 border-t border-neutral-100 p-4">
        <div>
          <div className="mb-1 text-sm text-neutral-500">Email</div>
          {isEditing ? (
            <form onSubmit={handleSubmit} className="flex flex-col gap-3">
              <div className="flex gap-2">
                <input
                  type="email"
                  value={newEmail}
                  onChange={(e) => setNewEmail(e.target.value)}
                  placeholder={email || "Enter new email"}
                  className="flex-1 rounded-md border border-neutral-200 px-3 py-2 text-sm focus:border-transparent focus:ring-2 focus:ring-stone-500 focus:outline-none"
                  autoFocus
                />
              </div>
              {updateEmailMutation.isError && (
                <p className="text-sm text-red-600">
                  {updateEmailMutation.error?.message ||
                    "Failed to update email"}
                </p>
              )}
              <div className="flex gap-2">
                <button
                  type="submit"
                  disabled={
                    updateEmailMutation.isPending ||
                    !newEmail ||
                    newEmail === email
                  }
                  className="flex h-8 items-center rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-4 text-sm text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%] disabled:opacity-50 disabled:hover:scale-100"
                >
                  {updateEmailMutation.isPending ? "Saving..." : "Save"}
                </button>
                <button
                  type="button"
                  onClick={handleCancel}
                  disabled={updateEmailMutation.isPending}
                  className="flex h-8 items-center rounded-full border border-neutral-300 bg-linear-to-b from-white to-stone-50 px-4 text-sm text-neutral-700 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%] disabled:opacity-50"
                >
                  Cancel
                </button>
              </div>
            </form>
          ) : (
            <div className="flex items-center gap-3">
              <div className="text-base">{email || "Not available"}</div>
              <button
                onClick={() => {
                  setIsEditing(true);
                  setSuccessMessage(null);
                }}
                className="flex h-7 items-center rounded-full border border-neutral-300 bg-linear-to-b from-white to-stone-50 px-3 text-xs text-neutral-700 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]"
              >
                Change
              </button>
            </div>
          )}
        </div>

        {successMessage && (
          <div className="rounded-md border border-green-200 bg-green-50 p-3">
            <p className="text-sm text-green-800">{successMessage}</p>
          </div>
        )}
      </div>
    </div>
  );
}
