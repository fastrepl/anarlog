export function getNangoAuthErrorType(error: unknown) {
  if (
    error &&
    typeof error === "object" &&
    "type" in error &&
    typeof error.type === "string"
  ) {
    return error.type;
  }
  return "unknown_error";
}

export function getConnectionErrorMessage(
  errorType: string,
  providerName: string,
  integrationId?: string,
) {
  if (errorType === "blocked_by_browser") {
    return `Your browser blocked the ${providerName} sign-in window. Allow pop-ups for Anarlog and try again.`;
  }
  if (errorType === "window_closed") {
    if (integrationId === "google-calendar") {
      return 'The Google sign-in window closed before Calendar connected. If Google showed "This app is blocked", that is Google\'s verification gate, not Anarlog. Email founders@anarlog.so if you need access.';
    }
    return `The ${providerName} sign-in window closed before the connection finished. Please try again.`;
  }
  if (errorType === "resource_capped") {
    return "This integration has reached its connection limit. Contact support to connect another account.";
  }
  if (integrationId === "google-calendar") {
    return 'Google rejected the Calendar connection. If you saw "This app is blocked", Google is still verifying Anarlog. Email founders@anarlog.so if you need access.';
  }
  return `${providerName} rejected the connection. Please try again or contact support if it keeps happening.`;
}
