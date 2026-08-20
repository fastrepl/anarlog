export function connectionNeedsReconnect(connection: {
  status?: string | null;
  last_error_type?: string | null;
}) {
  return (
    connection.status === "reconnect_required" ||
    Boolean(connection.last_error_type)
  );
}

export function connectionIdentityLabel(connection: {
  account_identity?: string | null;
  status?: string | null;
  last_error_type?: string | null;
}) {
  const identity = connection.account_identity?.trim();
  if (identity) {
    return identity;
  }
  if (connectionNeedsReconnect(connection)) {
    return "Needs reconnect.";
  }
  return "Connected.";
}

export function connectionReconnectError(connection: {
  last_error_description?: string | null;
}) {
  const description = connection.last_error_description?.trim();
  return description || "Connection needs attention.";
}
