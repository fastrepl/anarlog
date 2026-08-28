import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import {
  createFileRoute,
  Link,
  redirect,
  useNavigate,
} from "@tanstack/react-router";
import { useState } from "react";
import { z } from "zod";

import { AuthShell, authStyles } from "@/components/auth-shell";
import { doUpdatePassword, fetchUser } from "@/functions/auth";
import { flowSearchSchema } from "@/functions/desktop-flow";
import { toAuthFlowSearch } from "@/lib/auth-flow-context";
const styles = stylex.create({
  style1: {
    display: "flex",
    flexDirection: "column",
    gap: ".75rem",
  },
  style2: {
    textAlign: "center",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    color: "#b91c1c",
  },
  style3: {
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
});
const validateSearch = flowSearchSchema({
  redirect: z.string().optional(),
});
export const Route = createFileRoute("/update-password")({
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
  beforeLoad: async ({ search }) => {
    const user = await fetchUser();
    if (!user) {
      throw redirect({
        to: "/auth/",
        search: toAuthFlowSearch(search),
      });
    }
  },
});
function Component() {
  const context = Route.useSearch();
  const navigate = useNavigate();
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [errorMessage, setErrorMessage] = useState("");
  const updateMutation = useMutation({
    mutationFn: () =>
      doUpdatePassword({
        data: {
          password,
          flow: context.flow,
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
      if (result && "success" in result && result.success) {
        if (
          context.flow === "desktop" &&
          "access_token" in result &&
          "refresh_token" in result
        ) {
          navigate({
            to: "/callback/auth/",
            search: {
              flow: "desktop",
              scheme: context.scheme,
              access_token: result.access_token,
              refresh_token: result.refresh_token,
            },
          });
          return;
        }
        navigate({
          to: "/auth/",
          search: toAuthFlowSearch(context),
        });
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
      <form onSubmit={handleSubmit} {...stylex.props(styles.style1)}>
        <input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="New password"
          required
          {...stylex.props(authStyles.input)}
        />
        <input
          type="password"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
          placeholder="Confirm new password"
          required
          {...stylex.props(authStyles.input)}
        />
        {errorMessage && <p {...stylex.props(styles.style2)}>{errorMessage}</p>}
        <button
          type="submit"
          disabled={updateMutation.isPending || !password || !confirmPassword}
          {...stylex.props(authStyles.primaryButton)}
        >
          {updateMutation.isPending ? "Updating..." : "Update password"}
        </button>
      </form>

      <Link
        to="/auth/"
        search={toAuthFlowSearch(context)}
        {...stylex.props(styles.style3)}
      >
        Back to sign in
      </Link>
    </AuthShell>
  );
}
