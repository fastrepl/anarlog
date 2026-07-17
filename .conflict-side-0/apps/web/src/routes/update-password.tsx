import { useMutation } from "@tanstack/react-query";
import {
  createFileRoute,
  Link,
  redirect,
  useNavigate,
} from "@tanstack/react-router";
import { useState } from "react";

import {
  AuthShell,
  authInputClassName,
  authPrimaryButtonClassName,
} from "@/components/auth-shell";
import { doUpdatePassword, fetchUser } from "@/functions/auth";

export const Route = createFileRoute("/update-password")({
  component: Component,
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
  beforeLoad: async () => {
    const user = await fetchUser();
    if (!user) {
      throw redirect({ to: "/auth/", search: { flow: "web" } });
    }
  },
});

function Component() {
  const navigate = useNavigate();
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [errorMessage, setErrorMessage] = useState("");

  const updateMutation = useMutation({
    mutationFn: () => doUpdatePassword({ data: { password } }),
    onSuccess: (result) => {
      if (result && "error" in result && result.error) {
        setErrorMessage(
          (result as { error: boolean; message: string }).message,
        );
        return;
      }
      if (result && "success" in result && result.success) {
        navigate({ to: "/auth/", search: { flow: "web" } });
      }
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMessage("");

    if (password !== confirmPassword) {
      setErrorMessage("Passwords do not match");
      return;
    }
    if (password.length < 6) {
      setErrorMessage("Password must be at least 6 characters");
      return;
    }

    updateMutation.mutate();
  };

  return (
    <AuthShell
      title="Choose a new password"
      description="Use at least six characters, then you’ll be ready to sign in."
    >
      <form onSubmit={handleSubmit} className="flex flex-col gap-3">
        <input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="New password"
          required
          className={authInputClassName}
        />
        <input
          type="password"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
          placeholder="Confirm new password"
          required
          className={authInputClassName}
        />
        {errorMessage && (
          <p className="text-center text-sm text-red-700">{errorMessage}</p>
        )}
        <button
          type="submit"
          disabled={updateMutation.isPending || !password || !confirmPassword}
          className={authPrimaryButtonClassName}
        >
          {updateMutation.isPending ? "Updating..." : "Update password"}
        </button>
      </form>

      <Link
        to="/auth/"
        search={{ flow: "web" }}
        className="mt-5 flex items-center justify-center gap-1 text-sm text-[#756b5d] transition-colors hover:text-[#181613]"
      >
        Back to sign in
      </Link>
    </AuthShell>
  );
}
