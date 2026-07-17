const NANGO_CALLBACK_URL = "https://api.nango.dev/oauth/callback";

export default (request: Request): Response => {
  const query = new URL(request.url).search;

  return new Response(null, {
    status: 308,
    headers: {
      "cache-control": "no-store",
      location: `${NANGO_CALLBACK_URL}${query}`,
    },
  });
};

export const config = {
  path: "/oauth/callback",
  method: ["GET"],
};
