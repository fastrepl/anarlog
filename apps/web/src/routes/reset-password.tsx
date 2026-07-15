import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { ArrowLeftIcon } from "lucide-react";
import { useState } from "react";

import {
  AuthShell,
  authInputClassName,
  authNoticeClassName,
  authPrimaryButtonClassName,
} from "@/components/auth-shell";
import { doPasswordResetRequest } from "@/functions/auth";

export const Route = createFileRoute("/reset-password")({
  component: Component,
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
});

function Component() {
  const [email, setEmail] = useState("");
  const [submitted, setSubmitted] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");

  const resetMutation = useMutation({
    mutationFn: () => doPasswordResetRequest({ data: { email } }),
    onSuccess: (result) => {
      if (result && "error" in result && result.error) {
        setErrorMessage(
          (result as { error: boolean; message: string }).message,
        );
        return;
      }
      setSubmitted(true);
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMessage("");
    resetMutation.mutate();
  };

  return (
    <AuthShell
      title="Reset your password"
      description="We’ll send a reset link to the email on your account."
    >
      {submitted ? (
        <div className={authNoticeClassName}>
          <p className="font-medium text-[#4f4940]">Check your email</p>
          <p className="mt-1 text-sm leading-6 text-[#756b5d]">
            We sent a password reset link to {email}
          </p>
        </div>
      ) : (
        <form onSubmit={handleSubmit} className="flex flex-col gap-3">
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="Email"
            required
            className={authInputClassName}
          />
          {errorMessage && (
            <p className="text-center text-sm text-red-700">{errorMessage}</p>
          )}
          <button
            type="submit"
            disabled={resetMutation.isPending || !email}
            className={authPrimaryButtonClassName}
          >
            {resetMutation.isPending ? "Sending..." : "Send reset link"}
          </button>
        </form>
      )}

      <Link
        to="/auth/"
        search={{ flow: "web" }}
        className="mt-5 flex items-center justify-center gap-1 text-sm text-[#756b5d] transition-colors hover:text-[#181613]"
      >
        <ArrowLeftIcon className="size-3.5" />
        Back to sign in
      </Link>
    </AuthShell>
  );
}
