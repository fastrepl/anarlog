import { useMutation } from "@tanstack/react-query";
import { createFileRoute, Link, redirect } from "@tanstack/react-router";
import { useRef, useState, type ReactNode } from "react";
import { z } from "zod";

import { ArrowLeft, Buildings, Envelope } from "@anlg/ui/components/icons";
import { cn } from "@anlg/utils";

import {
  AuthShell,
  authInputClassName,
  authNoticeClassName,
  authPrimaryButtonClassName,
  authSecondaryButtonClassName,
} from "@/components/auth-shell";
import {
  createDesktopSession,
  doAuth,
  doMagicLinkAuth,
  doPasswordSignIn,
  doPasswordSignUp,
  doSsoAuth,
  fetchUser,
} from "@/functions/auth";
import { fetchLastSignInMethod } from "@/functions/auth-last-used";
import {
  DEFAULT_DESKTOP_SCHEME,
  type DesktopScheme,
  flowSearchSchema,
} from "@/functions/desktop-flow";
import { useMountEffect } from "@/hooks/useMountEffect";
import {
  shouldReuseBrowserSession,
  toAuthFlowSearch,
} from "@/lib/auth-flow-context";
import type { AuthSignInMethod } from "@/lib/auth-last-sign-in-method";
import {
  buildPostAuthDestination,
  sanitizeInternalReturnPath,
} from "@/lib/auth-redirect";
import {
  buildDesktopAuthCallbackPath,
  resolveDesktopAuthCallbackMethod,
} from "@/lib/desktop-auth-handoff";
import {
  capturePrivateRouteEvent,
  identifyPrivateRouteUser,
} from "@/lib/private-route-analytics";

const commonSearch = {
  redirect: z.string().optional(),
  provider: z.enum(["apple", "azure", "github", "google"]).optional(),
  view: z.enum(["email", "sso"]).optional(),
};

const validateSearch = flowSearchSchema(commonSearch);

export const Route = createFileRoute("/auth")({
  validateSearch,
  component: Component,
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
  beforeLoad: async ({ search }) => {
    const [user, lastSignInMethod] = await Promise.all([
      shouldReuseBrowserSession(search) ? fetchUser() : null,
      fetchLastSignInMethod(),
    ]);

    if (user) {
      if (search.flow === "web") {
        throw redirect({
          href: sanitizeInternalReturnPath(search.redirect),
        } as any);
      }

      if (search.flow === "desktop") {
        const result = await createDesktopSession();

        if (result) {
          throw redirect({
            to: "/callback/auth/",
            search: {
              flow: "desktop",
              scheme: search.scheme ?? DEFAULT_DESKTOP_SCHEME,
              access_token: result.access_token,
              refresh_token: result.refresh_token,
              method: search.provider ?? search.view,
            },
          });
        }
      }
    }

    return { existingUser: user, lastSignInMethod };
  },
});

type AuthView = "main" | "email" | "sso";
type OAuthProvider = "apple" | "azure" | "github" | "google";

const oauthProviderIcons: Record<OAuthProvider, string> = {
  apple: "/icons/auth/apple.svg",
  azure: "/icons/auth/microsoft.svg",
  github: "/icons/auth/github.svg",
  google: "/icons/auth/google.svg",
};

function getOAuthProviderName(provider: OAuthProvider) {
  return provider === "azure"
    ? "Microsoft"
    : provider.charAt(0).toUpperCase() + provider.slice(1);
}

function Component() {
  const {
    flow,
    scheme,
    redirect,
    provider,
    view: initialView,
  } = Route.useSearch();
  const { existingUser, lastSignInMethod } = Route.useRouteContext();
  const [view, setView] = useState<AuthView>(initialView ?? "main");
  const autoStartOAuth = flow === "desktop" && provider !== undefined;

  if (existingUser && flow === "desktop") {
    return (
      <AuthShell
        title="Welcome back"
        description="Finishing your secure handoff to the desktop app."
      >
        <DesktopReauthView
          email={existingUser.email}
          scheme={scheme ?? DEFAULT_DESKTOP_SCHEME}
          callbackMethod={resolveDesktopAuthCallbackMethod(
            provider ?? initialView,
            lastSignInMethod,
          )}
          lastSignInMethod={lastSignInMethod}
        />
      </AuthShell>
    );
  }

  const showApple = !provider || provider === "apple";
  const showGoogle = !provider || provider === "google";
  const showMicrosoft = !provider || provider === "azure";
  const showGithub = !provider || provider === "github";
  const showEmail = !provider;

  return (
    <AuthShell title="Welcome to Anarlog" showEyebrow={false}>
      {view === "main" && (
        <>
          <div className="flex flex-col gap-3">
            {showApple && (
              <OAuthButton
                flow={flow}
                scheme={scheme}
                redirect={redirect}
                provider="apple"
                autoStart={autoStartOAuth}
                isLastUsed={lastSignInMethod === "apple"}
              />
            )}
            {showGoogle && (
              <OAuthButton
                flow={flow}
                scheme={scheme}
                redirect={redirect}
                provider="google"
                autoStart={autoStartOAuth}
                isLastUsed={lastSignInMethod === "google"}
              />
            )}
            {showMicrosoft && (
              <OAuthButton
                flow={flow}
                scheme={scheme}
                redirect={redirect}
                provider="azure"
                autoStart={autoStartOAuth}
                isLastUsed={lastSignInMethod === "azure"}
              />
            )}
            {showGithub && (
              <OAuthButton
                flow={flow}
                scheme={scheme}
                redirect={redirect}
                provider="github"
                autoStart={autoStartOAuth}
                isLastUsed={lastSignInMethod === "github"}
              />
            )}
            {showEmail && (
              <AuthMethodButton isLastUsed={lastSignInMethod === "email"}>
                <button
                  onClick={() => setView("email")}
                  className={authSecondaryButtonClassName}
                >
                  <AuthProviderContent
                    icon={
                      <Envelope className="size-[18px]" aria-hidden="true" />
                    }
                  >
                    Sign in with Email
                  </AuthProviderContent>
                </button>
              </AuthMethodButton>
            )}
            {showEmail && (
              <AuthMethodButton isLastUsed={lastSignInMethod === "sso"}>
                <button
                  onClick={() => setView("sso")}
                  className={authSecondaryButtonClassName}
                >
                  <AuthProviderContent
                    icon={
                      <Buildings className="size-[18px]" aria-hidden="true" />
                    }
                  >
                    Sign in with SSO
                  </AuthProviderContent>
                </button>
              </AuthMethodButton>
            )}
          </div>
          <LegalText />
        </>
      )}
      {view === "email" && (
        <EmailAuthView
          flow={flow}
          scheme={scheme}
          redirect={redirect}
          onBack={() => setView("main")}
        />
      )}
      {view === "sso" && (
        <SsoAuthView
          flow={flow}
          scheme={scheme}
          redirect={redirect}
          onBack={() => setView("main")}
        />
      )}
    </AuthShell>
  );
}

function DesktopReauthView({
  email,
  scheme,
  callbackMethod,
  lastSignInMethod,
}: {
  email: string;
  scheme: DesktopScheme;
  callbackMethod: AuthSignInMethod | undefined;
  lastSignInMethod: AuthSignInMethod | null;
}) {
  const retryMutation = useMutation({
    mutationFn: () => {
      capturePrivateRouteEvent("auth_started", {
        method: "desktop_reauth",
        flow: "desktop",
      });
      return createDesktopSession();
    },
    onSuccess: (result) => {
      if (result) {
        window.location.href = buildDesktopAuthCallbackPath(
          result.access_token,
          result.refresh_token,
          scheme,
          callbackMethod,
        );
      }
    },
    onError: () => {
      capturePrivateRouteEvent("auth_failed", {
        method: "desktop_reauth",
        flow: "desktop",
        failure_stage: "session_handoff",
      });
    },
  });

  useMountEffect(() => {
    retryMutation.mutate();
  });

  const hasRetryFailed =
    retryMutation.isError || (retryMutation.isSuccess && !retryMutation.data);

  return (
    <div className="flex flex-col gap-4">
      {!hasRetryFailed && (
        <div className={authNoticeClassName}>
          <p className="text-sm font-medium text-[#4f4940]">
            Signing in as {email}...
          </p>
        </div>
      )}
      {hasRetryFailed && (
        <>
          <div className="text-center">
            <p className="mb-1 text-sm font-medium text-[#4f4940]">
              Signed in as {email}
            </p>
            <p className="text-sm text-[#8b8174]">
              Sign in with your provider to continue to the app
            </p>
          </div>
          <div className="flex flex-col gap-3">
            <OAuthButton
              flow="desktop"
              scheme={scheme}
              provider="apple"
              isLastUsed={lastSignInMethod === "apple"}
            />
            <OAuthButton
              flow="desktop"
              scheme={scheme}
              provider="google"
              isLastUsed={lastSignInMethod === "google"}
            />
            <OAuthButton
              flow="desktop"
              scheme={scheme}
              provider="azure"
              isLastUsed={lastSignInMethod === "azure"}
            />
            <OAuthButton
              flow="desktop"
              scheme={scheme}
              provider="github"
              isLastUsed={lastSignInMethod === "github"}
            />
            <SsoAuthView flow="desktop" scheme={scheme} />
          </div>
        </>
      )}
    </div>
  );
}

function LegalText() {
  return (
    <p className="mt-6 text-center text-xs leading-5 text-[#8b8174]">
      By signing up, you agree to our{" "}
      <a
        href="https://anarlog.so/terms"
        className="underline decoration-[#b9ae9f] underline-offset-2 hover:text-[#181613]"
      >
        Terms of Service
      </a>{" "}
      and{" "}
      <a
        href="https://anarlog.so/privacy"
        className="underline decoration-[#b9ae9f] underline-offset-2 hover:text-[#181613]"
      >
        Privacy Policy
      </a>
      .
    </p>
  );
}

type EmailMode = "password" | "magic-link";

function EmailAuthView({
  flow,
  scheme,
  redirect,
  onBack,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  redirect?: string;
  onBack: () => void;
}) {
  const [mode, setMode] = useState<EmailMode>("password");

  return (
    <div className="flex flex-col gap-5">
      <button
        onClick={onBack}
        className="flex cursor-pointer items-center gap-1 self-start text-sm text-[#756b5d] transition-colors hover:text-[#181613]"
      >
        <ArrowLeft className="size-3.5" />
        Back
      </button>

      <div className="flex gap-1 rounded-full bg-[#f4efe6] p-1">
        <button
          onClick={() => setMode("password")}
          className={cn([
            "flex-1 cursor-pointer rounded-full py-2 text-sm font-medium transition-colors",
            mode === "password"
              ? "bg-white text-[#181613] shadow-sm"
              : "text-[#756b5d] hover:text-[#181613]",
          ])}
        >
          Password
        </button>
        <button
          onClick={() => setMode("magic-link")}
          className={cn([
            "flex-1 cursor-pointer rounded-full py-2 text-sm font-medium transition-colors",
            mode === "magic-link"
              ? "bg-white text-[#181613] shadow-sm"
              : "text-[#756b5d] hover:text-[#181613]",
          ])}
        >
          Magic link
        </button>
      </div>

      {mode === "password" && (
        <PasswordForm flow={flow} scheme={scheme} redirect={redirect} />
      )}
      {mode === "magic-link" && (
        <MagicLinkForm flow={flow} scheme={scheme} redirect={redirect} />
      )}

      <LegalText />
    </div>
  );
}

function SsoAuthView({
  flow,
  scheme,
  redirect,
  onBack,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  redirect?: string;
  onBack?: () => void;
}) {
  const [domain, setDomain] = useState("");
  const ssoMutation = useMutation({
    mutationFn: () => {
      capturePrivateRouteEvent("auth_started", {
        method: "sso",
        flow,
      });
      return doSsoAuth({
        data: {
          domain,
          flow,
          scheme,
          redirect,
        },
      });
    },
    onSuccess: (result) => {
      if (result?.url) {
        window.location.href = result.url;
        return;
      }
      capturePrivateRouteEvent("auth_failed", {
        method: "sso",
        flow,
        failure_stage: "provider",
      });
    },
    onError: () => {
      capturePrivateRouteEvent("auth_failed", {
        method: "sso",
        flow,
        failure_stage: "request",
      });
    },
  });

  return (
    <div className="flex flex-col gap-5">
      {onBack ? (
        <button
          onClick={onBack}
          className="flex cursor-pointer items-center gap-1 self-start text-sm text-[#756b5d] transition-colors hover:text-[#181613]"
        >
          <ArrowLeft className="size-3.5" />
          Back
        </button>
      ) : null}
      <form
        className="flex flex-col gap-3"
        onSubmit={(event) => {
          event.preventDefault();
          if (domain.trim()) ssoMutation.mutate();
        }}
      >
        <input
          type="text"
          autoComplete="organization"
          placeholder="you@company.com"
          value={domain}
          onChange={(event) => setDomain(event.target.value)}
          className={authInputClassName}
        />
        <button
          type="submit"
          disabled={!domain.trim() || ssoMutation.isPending}
          className={authPrimaryButtonClassName}
        >
          Continue with SSO
        </button>
        {ssoMutation.data &&
        "error" in ssoMutation.data &&
        ssoMutation.data.error ? (
          <p className="text-sm text-red-700">{ssoMutation.data.message}</p>
        ) : null}
      </form>
      {onBack ? <LegalText /> : null}
    </div>
  );
}

function PasswordForm({
  flow,
  scheme,
  redirect,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  redirect?: string;
}) {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [isSignUp, setIsSignUp] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");
  const [submitted, setSubmitted] = useState(false);

  const signInMutation = useMutation({
    mutationFn: () => {
      capturePrivateRouteEvent("auth_started", {
        method: "password",
        action: "sign_in",
        flow,
      });
      return doPasswordSignIn({
        data: { email, password, flow, scheme, redirect },
      });
    },
    onSuccess: (result) => {
      if (result && "error" in result && result.error) {
        capturePrivateRouteEvent("auth_failed", {
          method: "password",
          action: "sign_in",
          flow,
          failure_stage: "provider",
        });
        setErrorMessage(
          (result as { error: boolean; message: string }).message,
        );
        return;
      }
      if (
        result &&
        "success" in result &&
        result.success &&
        "access_token" in result
      ) {
        identifyPrivateRouteUser(
          "userId" in result
            ? (result.userId as string | undefined)
            : undefined,
          { method: "password", action: "sign_in", flow },
        );
        handlePasswordSuccess(
          result.access_token as string,
          result.refresh_token as string,
          flow,
          scheme,
          redirect,
          false,
        );
      }
    },
    onError: () => {
      capturePrivateRouteEvent("auth_failed", {
        method: "password",
        action: "sign_in",
        flow,
        failure_stage: "request",
      });
    },
  });

  const signUpMutation = useMutation({
    mutationFn: () => {
      capturePrivateRouteEvent("auth_started", {
        method: "password",
        action: "sign_up",
        flow,
      });
      return doPasswordSignUp({
        data: { name, email, password, flow, scheme, redirect },
      });
    },
    onSuccess: (result) => {
      if (result && "error" in result && result.error) {
        capturePrivateRouteEvent("auth_failed", {
          method: "password",
          action: "sign_up",
          flow,
          failure_stage: "provider",
        });
        setErrorMessage(
          (result as { error: boolean; message: string }).message,
        );
        return;
      }
      if (result && "success" in result && result.success) {
        identifyPrivateRouteUser(
          "userId" in result
            ? (result.userId as string | undefined)
            : undefined,
          { method: "password", action: "sign_up", flow },
        );
        if ("needsConfirmation" in result && result.needsConfirmation) {
          setSubmitted(true);
          return;
        }
        if ("access_token" in result) {
          handlePasswordSuccess(
            result.access_token as string,
            result.refresh_token as string,
            flow,
            scheme,
            redirect,
            "newAccount" in result && result.newAccount,
          );
        }
      }
    },
    onError: () => {
      capturePrivateRouteEvent("auth_failed", {
        method: "password",
        action: "sign_up",
        flow,
        failure_stage: "request",
      });
    },
  });

  const isPending = signInMutation.isPending || signUpMutation.isPending;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMessage("");

    if (isSignUp) {
      if (password !== confirmPassword) {
        capturePrivateRouteEvent("auth_failed", {
          method: "password",
          action: "sign_up",
          flow,
          failure_stage: "validation",
        });
        setErrorMessage("Passwords do not match");
        return;
      }
      if (password.length < 6) {
        capturePrivateRouteEvent("auth_failed", {
          method: "password",
          action: "sign_up",
          flow,
          failure_stage: "validation",
        });
        setErrorMessage("Password must be at least 6 characters");
        return;
      }
      signUpMutation.mutate();
    } else {
      signInMutation.mutate();
    }
  };

  if (submitted) {
    return (
      <div className={authNoticeClassName}>
        <p className="font-medium text-[#4f4940]">Check your email</p>
        <p className="mt-1 text-sm text-[#756b5d]">
          We sent a confirmation link to {email}
        </p>
      </div>
    );
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col gap-3">
      {isSignUp && (
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Name"
          autoComplete="name"
          required
          className={authInputClassName}
        />
      )}
      <input
        type="email"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        placeholder="Email"
        required
        className={authInputClassName}
      />
      <input
        type="password"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        placeholder="Password"
        required
        className={authInputClassName}
      />
      {isSignUp && (
        <input
          type="password"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
          placeholder="Confirm password"
          required
          className={authInputClassName}
        />
      )}
      {errorMessage && (
        <p className="text-center text-sm text-red-700">{errorMessage}</p>
      )}
      <button
        type="submit"
        disabled={
          isPending ||
          !email ||
          !password ||
          (isSignUp && (!name.trim() || !confirmPassword))
        }
        className={authPrimaryButtonClassName}
      >
        {isPending ? "Loading..." : isSignUp ? "Create account" : "Sign in"}
      </button>
      <div className="flex flex-col items-center gap-1">
        <button
          type="button"
          onClick={() => {
            setIsSignUp(!isSignUp);
            setErrorMessage("");
            setName("");
            setConfirmPassword("");
          }}
          className="cursor-pointer text-sm text-[#756b5d] transition-colors hover:text-[#181613] hover:underline"
        >
          {isSignUp
            ? "Already have an account? Sign in"
            : "Don't have an account? Sign up"}
        </button>
        {!isSignUp && (
          <Link
            to="/reset-password/"
            search={toAuthFlowSearch({ flow, scheme, redirect })}
            className="text-sm text-[#756b5d] transition-colors hover:text-[#181613] hover:underline"
          >
            Forgot password?
          </Link>
        )}
      </div>
    </form>
  );
}

function handlePasswordSuccess(
  accessToken: string,
  refreshToken: string,
  flow: "desktop" | "web",
  scheme?: DesktopScheme,
  redirectPath?: string,
  newAccount = false,
) {
  if (flow === "desktop") {
    window.location.href = buildDesktopAuthCallbackPath(
      accessToken,
      refreshToken,
      scheme,
      "email",
    );
  } else {
    window.location.href = buildPostAuthDestination({
      newAccount,
      returnTo: redirectPath,
    });
  }
}

function MagicLinkForm({
  flow,
  scheme,
  redirect,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  redirect?: string;
}) {
  const [email, setEmail] = useState("");
  const [submitted, setSubmitted] = useState(false);

  const magicLinkMutation = useMutation({
    mutationFn: (email: string) => {
      capturePrivateRouteEvent("auth_started", {
        method: "magic_link",
        flow,
      });
      return doMagicLinkAuth({
        data: {
          email,
          flow,
          scheme,
          redirect,
        },
      });
    },
    onSuccess: (result) => {
      if (result && !("error" in result)) {
        setSubmitted(true);
      } else {
        capturePrivateRouteEvent("auth_failed", {
          method: "magic_link",
          flow,
          failure_stage: "provider",
        });
      }
    },
    onError: () => {
      capturePrivateRouteEvent("auth_failed", {
        method: "magic_link",
        flow,
        failure_stage: "request",
      });
    },
  });

  if (submitted) {
    return (
      <div className={authNoticeClassName}>
        <p className="font-medium text-[#4f4940]">Check your email</p>
        <p className="mt-1 text-sm text-[#756b5d]">
          We sent a magic link to {email}
        </p>
      </div>
    );
  }

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (email) {
          magicLinkMutation.mutate(email);
        }
      }}
      className="flex flex-col gap-3"
    >
      <input
        type="email"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        placeholder="Enter your email"
        required
        className={authInputClassName}
      />
      <button
        type="submit"
        disabled={magicLinkMutation.isPending || !email}
        className={authPrimaryButtonClassName}
      >
        {magicLinkMutation.isPending ? "Sending..." : "Send magic link"}
      </button>
      {magicLinkMutation.isError && (
        <p className="text-center text-sm text-red-700">
          Failed to send magic link. Please try again.
        </p>
      )}
    </form>
  );
}

function AuthProviderContent({
  icon,
  children,
}: {
  icon: ReactNode;
  children: ReactNode;
}) {
  return (
    <span className="inline-flex items-center gap-3">
      <span className="flex size-[18px] items-center justify-center overflow-hidden">
        {icon}
      </span>
      <span>{children}</span>
    </span>
  );
}

function AuthMethodButton({
  isLastUsed,
  children,
}: {
  isLastUsed: boolean;
  children: ReactNode;
}) {
  return (
    <div className={cn(["relative", isLastUsed && "pt-1"])}>
      {children}
      {isLastUsed && (
        <span className="border-surface bg-fg text-surface pointer-events-none absolute top-0 right-4 rounded-full border-2 px-2 py-0.5 text-[10px] leading-3 font-medium">
          Last used
        </span>
      )}
    </div>
  );
}

function OAuthButton({
  flow,
  scheme,
  redirect,
  provider,
  autoStart = false,
  isLastUsed = false,
}: {
  flow: "desktop" | "web";
  scheme?: DesktopScheme;
  redirect?: string;
  provider: OAuthProvider;
  autoStart?: boolean;
  isLastUsed?: boolean;
}) {
  const oauthMutation = useMutation({
    mutationFn: (provider: OAuthProvider) => {
      capturePrivateRouteEvent("auth_started", {
        method: "oauth",
        provider,
        flow,
      });
      return doAuth({
        data: {
          provider,
          flow,
          scheme,
          redirect,
        },
      });
    },
    onSuccess: (result) => {
      if (result?.url) {
        window.location.href = result.url;
      } else {
        capturePrivateRouteEvent("auth_failed", {
          method: "oauth",
          provider,
          flow,
          failure_stage: "provider",
        });
      }
    },
    onError: () => {
      capturePrivateRouteEvent("auth_failed", {
        method: "oauth",
        provider,
        flow,
        failure_stage: "request",
      });
    },
  });
  const { mutate, isPending } = oauthMutation;
  const hasAutoStartedRef = useRef(false);

  useMountEffect(() => {
    if (autoStart && !hasAutoStartedRef.current) {
      hasAutoStartedRef.current = true;
      mutate(provider);
    }
  });

  return (
    <AuthMethodButton isLastUsed={isLastUsed}>
      <button
        onClick={() => mutate(provider)}
        disabled={isPending}
        className={authSecondaryButtonClassName}
      >
        <AuthProviderContent
          icon={
            <img
              src={oauthProviderIcons[provider]}
              className="size-[18px] object-contain"
              alt=""
            />
          }
        >
          Sign in with {getOAuthProviderName(provider)}
        </AuthProviderContent>
      </button>
    </AuthMethodButton>
  );
}
