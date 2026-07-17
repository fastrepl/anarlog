import { createMiddleware } from "@tanstack/react-start";
import { setResponseHeader } from "@tanstack/react-start/server";

const CORS_HEADERS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, POST, PUT, DELETE, OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type, Authorization",
} as const;

export const corsMiddleware = createMiddleware().server(
  async ({ next, request }) => {
    Object.entries(CORS_HEADERS).forEach(([key, value]) => {
      setResponseHeader(key, value);
    });

    if (request.method === "OPTIONS") {
      throw new Response(null, {
        status: 204,
        headers: CORS_HEADERS,
      });
    }

    return next();
  },
);
