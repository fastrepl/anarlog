import type { Session, SupabaseClient } from "@supabase/supabase-js";
import { createContext, useContext } from "react";

type AuthState = {
  supabase: SupabaseClient | null;
  // undefined = initial load in progress, null = known unauthenticated
  session: Session | null | undefined;
  isRefreshingSession: boolean;
};

type AuthActions = {
  signIn: () => Promise<void>;
  signOut: () => Promise<void>;
  refreshSession: () => Promise<Session | null>;
};

type AuthTokenHandlers = {
  handleAuthCallback: (url: string) => Promise<void>;
  setSessionFromTokens: (
    accessToken: string,
    refreshToken: string,
  ) => Promise<void>;
};

type AuthUtils = {
  getHeaders: () => Record<string, string> | null;
  getAvatarUrl: () => Promise<string | null>;
};

export type AuthContextType = AuthState &
  AuthActions &
  AuthTokenHandlers &
  AuthUtils;

export const AuthContext = createContext<AuthContextType | null>(null);

export function useOptionalAuth() {
  return useContext(AuthContext);
}

export function useAuth() {
  const context = useOptionalAuth();

  if (!context) {
    throw new Error("'useAuth' must be used within an 'AuthProvider'");
  }

  return context;
}
