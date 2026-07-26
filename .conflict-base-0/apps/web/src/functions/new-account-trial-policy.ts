import type { User } from "@supabase/supabase-js";

export type NewAccountAuthMethod =
  | "password-signup"
  | "oauth"
  | "email"
  | "recovery"
  | "magiclink"
  | "signup"
  | "invite"
  | "email_change";

const INITIAL_SIGN_IN_TOLERANCE_MS = 10_000;

type ConfirmedAccountUser = Pick<
  User,
  | "created_at"
  | "confirmed_at"
  | "email_confirmed_at"
  | "phone_confirmed_at"
  | "last_sign_in_at"
>;

export function shouldOfferNewAccountTrialCheckout({
  flow,
  method,
  user,
}: {
  flow: "desktop" | "web";
  method: NewAccountAuthMethod;
  user: ConfirmedAccountUser;
}) {
  return flow === "web" && isConfirmedNewAccount(user, method);
}

export function isConfirmedNewAccount(
  user: ConfirmedAccountUser,
  method: NewAccountAuthMethod,
) {
  if (
    method === "password-signup" ||
    method === "signup" ||
    method === "invite"
  ) {
    return true;
  }

  if (method !== "oauth" && method !== "email" && method !== "magiclink") {
    return false;
  }

  const lastSignInAt = parseTimestamp(user.last_sign_in_at);
  if (lastSignInAt === null) {
    return false;
  }

  return [
    user.confirmed_at,
    user.email_confirmed_at,
    user.phone_confirmed_at,
    user.created_at,
  ].some((timestamp) => {
    const initialAccountAt = parseTimestamp(timestamp);
    return (
      initialAccountAt !== null &&
      Math.abs(lastSignInAt - initialAccountAt) <= INITIAL_SIGN_IN_TOLERANCE_MS
    );
  });
}

function parseTimestamp(timestamp: string | undefined) {
  if (!timestamp) {
    return null;
  }

  const value = Date.parse(timestamp);
  return Number.isFinite(value) ? value : null;
}
