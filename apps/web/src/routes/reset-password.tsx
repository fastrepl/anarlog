import { ArrowLeft } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useState } from "react";
import { z } from "zod";

import { AuthShell, authStyles } from "@/components/auth-shell";
import { doPasswordResetRequest } from "@/functions/auth";
import { flowSearchSchema } from "@/functions/desktop-flow";
import { toAuthFlowSearch } from "@/lib/auth-flow-context";
const styles = stylex.create({
  style1: {
    fontWeight: 500,
    color: "#4f4940",
  },
  style2: {
    marginTop: ".25rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style3: {
    display: "flex",
    flexDirection: "column",
    gap: ".75rem",
  },
  style4: {
    textAlign: "center",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#b91c1c",
  },
  style5: {
    marginTop: "1.25rem",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    gap: ".25rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: {
      default: "#756b5d",
      ":hover": "#181613",
    },
    transitionProperty:
      "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(.4, 0, .2, 1)",
    transitionDuration: ".15s",
  },
  style6: {
    width: ".875rem",
    height: ".875rem",
  },
});
const validateSearch = flowSearchSchema({
  redirect: z.string().optional(),
});
export const Route = createFileRoute("/reset-password")({
  validateSearch,
  component: Component,
  head: () => ({
    meta: [
      {
        name: "robots",
        content: "noindex, nofollow",
      },
    ],
  }),
});
function Component() {
  const context = Route.useSearch();
  const [email, setEmail] = useState("");
  const [submitted, setSubmitted] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");
  const resetMutation = useMutation({
    mutationFn: () =>
      doPasswordResetRequest({
        data: {
          email,
          ...context,
        },
      }),
    onSuccess: (result) => {
      if (result && "error" in result && result.error) {
        setErrorMessage(
          (
            result as {
              error: boolean;
              message: string;
            }
          ).message,
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
        <div {...stylex.props(authStyles.notice)}>
          <p {...stylex.props(styles.style1)}>Check your email</p>
          <p {...stylex.props(styles.style2)}>
            We sent a password reset link to {email}
          </p>
        </div>
      ) : (
        <form onSubmit={handleSubmit} {...stylex.props(styles.style3)}>
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="Email"
            required
            {...stylex.props(authStyles.input)}
          />
          {errorMessage && (
            <p {...stylex.props(styles.style4)}>{errorMessage}</p>
          )}
          <button
            type="submit"
            disabled={resetMutation.isPending || !email}
            {...stylex.props(authStyles.primaryButton)}
          >
            {resetMutation.isPending ? "Sending..." : "Send reset link"}
          </button>
        </form>
      )}

      <Link
        to="/auth/"
        search={toAuthFlowSearch(context)}
        {...stylex.props(styles.style5)}
      >
        <ArrowLeft {...stylex.props(styles.style6)} />
        Back to sign in
      </Link>
    </AuthShell>
  );
}
