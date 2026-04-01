"use client";

import { useEffect, useState } from "react";

import { cn } from "@hypr/utils";

interface OpenAPISpec {
  openapi: string;
  info: {
    title: string;
    version: string;
    description?: string;
  };
  tags?: Array<{
    name: string;
    description?: string;
  }>;
  paths: Record<
    string,
    Record<
      string,
      {
        tags?: string[];
        summary?: string;
        description?: string;
        security?: Array<Record<string, string[]>>;
        requestBody?: {
          content?: Record<
            string,
            {
              schema?: Record<string, unknown>;
            }
          >;
        };
        responses?: Record<
          string,
          {
            description?: string;
            content?: Record<
              string,
              {
                schema?: Record<string, unknown>;
              }
            >;
          }
        >;
      }
    >
  >;
}

const METHOD_COLORS: Record<string, string> = {
  get: "bg-green-100 text-green-800 border-green-200",
  post: "bg-blue-100 text-blue-800 border-blue-200",
  put: "bg-yellow-100 text-yellow-800 border-yellow-200",
  delete: "bg-red-100 text-red-800 border-red-200",
  patch: "bg-purple-100 text-purple-800 border-purple-200",
};

const TAG_COLORS: Record<string, string> = {
  internal: "bg-gray-100 text-gray-700 border-gray-200",
  app: "bg-indigo-100 text-indigo-700 border-indigo-200",
  webhook: "bg-orange-100 text-orange-700 border-orange-200",
};

export function OpenAPIDocs({ apiUrl }: { apiUrl: string }) {
  const [spec, setSpec] = useState<OpenAPISpec | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch(`${apiUrl}/openapi.gen.json`)
      .then((res) => {
        if (!res.ok) throw new Error(`Failed to fetch: ${res.status}`);
        return res.json();
      })
      .then((data) => {
        setSpec(data);
        setLoading(false);
      })
      .catch((err) => {
        setError(err.message);
        setLoading(false);
      });
  }, [apiUrl]);

  if (loading) {
    return (
      <div className="p-8 text-center text-neutral-500">
        Loading API documentation...
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-8 text-center text-red-500">
        Failed to load API documentation: {error}
      </div>
    );
  }

  if (!spec) {
    return null;
  }

  const groupedPaths: Record<
    string,
    Array<{
      path: string;
      method: string;
      operation: (typeof spec.paths)[string][string];
    }>
  > = {};

  for (const [path, methods] of Object.entries(spec.paths)) {
    for (const [method, operation] of Object.entries(methods)) {
      const tag = operation.tags?.[0] || "other";
      if (!groupedPaths[tag]) {
        groupedPaths[tag] = [];
      }
      groupedPaths[tag].push({ path, method, operation });
    }
  }

  return (
    <div className="flex flex-col gap-8">
      <div className="flex flex-col gap-2">
        <p className="text-sm text-neutral-500">
          OpenAPI {spec.openapi} | Version {spec.info.version}
        </p>
        <p className="text-sm text-neutral-600">
          <a
            href={`${apiUrl}/openapi.gen.json`}
            target="_blank"
            rel="noopener noreferrer"
            className="underline hover:text-stone-800"
          >
            View raw OpenAPI spec
          </a>
        </p>
      </div>

      {spec.tags && spec.tags.length > 0 && (
        <div className="flex flex-col gap-4">
          <h3 className="font-serif text-lg text-stone-700">API Categories</h3>
          <div className="grid gap-3">
            {spec.tags.map((tag) => (
              <div
                key={tag.name}
                className={cn(
                  "rounded-xs border p-3",
                  TAG_COLORS[tag.name] || "border-gray-200 bg-gray-50",
                )}
              >
                <span className="font-medium capitalize">{tag.name}</span>
                {tag.description && (
                  <p className="mt-1 text-sm opacity-80">{tag.description}</p>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {Object.entries(groupedPaths).map(([tag, endpoints]) => (
        <div key={tag} className="flex flex-col gap-4">
          <h3 className="font-serif text-lg text-stone-700 capitalize">
            {tag} Endpoints
          </h3>
          <div className="flex flex-col gap-3">
            {endpoints.map(({ path, method, operation }) => (
              <div
                key={`${method}-${path}`}
                className="overflow-hidden rounded-xs border border-neutral-200"
              >
                <div className="flex items-center gap-3 bg-neutral-50 p-3">
                  <span
                    className={cn(
                      "rounded border px-2 py-1 font-mono text-xs font-medium uppercase",
                      METHOD_COLORS[method] || "bg-gray-100 text-gray-800",
                    )}
                  >
                    {method}
                  </span>
                  <code className="font-mono text-sm text-stone-700">
                    {path}
                  </code>
                  {operation.security && operation.security.length > 0 && (
                    <span className="ml-auto flex items-center gap-1 text-xs text-neutral-500">
                      <svg
                        className="h-3 w-3"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={2}
                          d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"
                        />
                      </svg>
                      Auth required
                    </span>
                  )}
                </div>
                <div className="flex flex-col gap-2 p-3">
                  {operation.summary && (
                    <p className="font-medium text-stone-700">
                      {operation.summary}
                    </p>
                  )}
                  {operation.description && (
                    <p className="text-sm text-neutral-600">
                      {operation.description}
                    </p>
                  )}
                  {operation.responses && (
                    <div className="mt-3">
                      <p className="mb-2 text-xs font-medium text-neutral-500 uppercase">
                        Responses
                      </p>
                      <div className="flex flex-wrap gap-2">
                        {Object.entries(operation.responses).map(
                          ([code, response]) => (
                            <span
                              key={code}
                              className={cn(
                                "rounded border px-2 py-1 text-xs",
                                code.startsWith("2")
                                  ? "border-green-200 bg-green-50 text-green-700"
                                  : code.startsWith("4")
                                    ? "border-yellow-200 bg-yellow-50 text-yellow-700"
                                    : code.startsWith("5")
                                      ? "border-red-200 bg-red-50 text-red-700"
                                      : "border-gray-200 bg-gray-50 text-gray-700",
                              )}
                              title={response.description}
                            >
                              {code}
                            </span>
                          ),
                        )}
                      </div>
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
