export async function mintDesktopSessionForAuthenticatedUser({
  getUser,
  mintSession,
}: {
  getUser: () => Promise<{
    data: { user: { email?: string } | null };
    error: unknown;
  }>;
  mintSession: (
    email: string,
  ) => Promise<{ access_token: string; refresh_token: string } | null>;
}) {
  const { data, error } = await getUser();
  const email = data.user?.email;

  if (error || !email) {
    return null;
  }

  return mintSession(email);
}
