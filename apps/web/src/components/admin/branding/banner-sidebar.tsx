import { Download } from "lucide-react";
import { type Dispatch, useCallback, useRef } from "react";

import { cn } from "@hypr/utils";

import { exportBanner } from "./export";
import { platforms } from "./platforms";
import type { BannerAction, BannerState } from "./reducer";
import { templates } from "./templates";

export function BannerSidebar({
  state,
  dispatch,
  canvasRef,
}: {
  state: BannerState;
  dispatch: Dispatch<BannerAction>;
  canvasRef: React.RefObject<HTMLDivElement | null>;
}) {
  const template = templates.find((t) => t.id === state.layoutId);
  const exporting = useRef(false);

  const handleExport = useCallback(async () => {
    if (exporting.current) return;
    exporting.current = true;
    try {
      const platform = platforms.find((p) => p.id === state.platformId);
      const name = `${state.layoutId}-${platform?.id || "banner"}.png`;
      await exportBanner(canvasRef.current, name);
    } finally {
      exporting.current = false;
    }
  }, [canvasRef, state.layoutId, state.platformId]);

  return (
    <div className="flex h-full flex-col overflow-y-auto">
      <div className="space-y-6 p-4">
        <section>
          <label className="mb-2 block text-xs font-medium tracking-wider text-neutral-500 uppercase">
            Layout
          </label>
          <div className="grid grid-cols-2 gap-2">
            {templates.map((t) => (
              <button
                key={t.id}
                onClick={() => dispatch({ type: "SET_LAYOUT", layoutId: t.id })}
                className={cn([
                  "rounded-lg border p-3 text-left text-sm transition-colors",
                  state.layoutId === t.id
                    ? "border-neutral-900 bg-neutral-50"
                    : "border-neutral-200 hover:border-neutral-400",
                ])}
              >
                <div className="font-medium">{t.label}</div>
                <div className="mt-0.5 text-xs text-neutral-500">
                  {t.description}
                </div>
              </button>
            ))}
          </div>
        </section>

        <section>
          <label className="mb-2 block text-xs font-medium tracking-wider text-neutral-500 uppercase">
            Platform
          </label>
          <select
            value={state.platformId}
            onChange={(e) =>
              dispatch({ type: "SET_PLATFORM", platformId: e.target.value })
            }
            className="w-full rounded-lg border border-neutral-200 px-3 py-2 text-sm focus:border-neutral-400 focus:outline-none"
          >
            {platforms.map((p) => (
              <option key={p.id} value={p.id}>
                {p.label} ({p.width}×{p.height})
              </option>
            ))}
          </select>
        </section>

        {template && (
          <section className="space-y-3">
            <label className="block text-xs font-medium tracking-wider text-neutral-500 uppercase">
              Content
            </label>
            {template.fields.map((field) => (
              <div key={field.key}>
                <label className="mb-1 block text-sm text-neutral-700">
                  {field.label}
                </label>
                {field.type === "textarea" ? (
                  <textarea
                    value={state.values[field.key] || ""}
                    onChange={(e) =>
                      dispatch({
                        type: "SET_FIELD",
                        key: field.key,
                        value: e.target.value,
                      })
                    }
                    rows={3}
                    className="w-full rounded-lg border border-neutral-200 px-3 py-2 text-sm focus:border-neutral-400 focus:outline-none"
                  />
                ) : field.type === "color" ? (
                  <div className="flex items-center gap-2">
                    <input
                      type="color"
                      value={state.values[field.key] || "#000000"}
                      onChange={(e) =>
                        dispatch({
                          type: "SET_FIELD",
                          key: field.key,
                          value: e.target.value,
                        })
                      }
                      className="h-8 w-8 cursor-pointer rounded border border-neutral-200"
                    />
                    <input
                      type="text"
                      value={state.values[field.key] || ""}
                      onChange={(e) =>
                        dispatch({
                          type: "SET_FIELD",
                          key: field.key,
                          value: e.target.value,
                        })
                      }
                      className="flex-1 rounded-lg border border-neutral-200 px-3 py-1.5 font-mono text-sm focus:border-neutral-400 focus:outline-none"
                    />
                  </div>
                ) : field.type === "image" ? (
                  <div className="space-y-2">
                    <input
                      type="text"
                      value={state.values[field.key] || ""}
                      onChange={(e) =>
                        dispatch({
                          type: "SET_FIELD",
                          key: field.key,
                          value: e.target.value,
                        })
                      }
                      placeholder="Paste image URL..."
                      className="w-full rounded-lg border border-neutral-200 px-3 py-2 text-sm focus:border-neutral-400 focus:outline-none"
                    />
                    <label className="flex cursor-pointer items-center justify-center rounded-lg border border-dashed border-neutral-300 px-3 py-2 text-sm text-neutral-500 transition-colors hover:border-neutral-400 hover:text-neutral-700">
                      Or upload file
                      <input
                        type="file"
                        accept="image/*"
                        className="hidden"
                        onChange={(e) => {
                          const file = e.target.files?.[0];
                          if (!file) return;
                          const reader = new FileReader();
                          reader.onload = () => {
                            dispatch({
                              type: "SET_FIELD",
                              key: field.key,
                              value: reader.result as string,
                            });
                          };
                          reader.readAsDataURL(file);
                        }}
                      />
                    </label>
                  </div>
                ) : field.type === "select" ? (
                  <select
                    value={state.values[field.key] || ""}
                    onChange={(e) =>
                      dispatch({
                        type: "SET_FIELD",
                        key: field.key,
                        value: e.target.value,
                      })
                    }
                    className="w-full rounded-lg border border-neutral-200 px-3 py-2 text-sm focus:border-neutral-400 focus:outline-none"
                  >
                    {field.options?.map((opt) => (
                      <option key={opt} value={opt}>
                        {opt}
                      </option>
                    ))}
                  </select>
                ) : field.type === "toggle" ? (
                  <button
                    onClick={() =>
                      dispatch({
                        type: "SET_FIELD",
                        key: field.key,
                        value:
                          state.values[field.key] === "true" ? "false" : "true",
                      })
                    }
                    className={cn([
                      "relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors",
                      state.values[field.key] === "true"
                        ? "bg-neutral-900"
                        : "bg-neutral-200",
                    ])}
                  >
                    <span
                      className={cn([
                        "pointer-events-none inline-block h-5 w-5 rounded-full bg-white shadow-sm transition-transform",
                        state.values[field.key] === "true"
                          ? "translate-x-5"
                          : "translate-x-0",
                      ])}
                    />
                  </button>
                ) : field.type === "toggle-group" ? (
                  <div className="flex gap-1 rounded-lg border border-neutral-200 p-1">
                    {field.options?.map((opt) => (
                      <button
                        key={opt}
                        onClick={() =>
                          dispatch({
                            type: "SET_FIELD",
                            key: field.key,
                            value: opt,
                          })
                        }
                        className={cn([
                          "flex-1 rounded-md px-3 py-1.5 text-sm font-medium capitalize transition-colors",
                          state.values[field.key] === opt
                            ? "bg-neutral-900 text-white"
                            : "text-neutral-600 hover:bg-neutral-100",
                        ])}
                      >
                        {opt}
                      </button>
                    ))}
                  </div>
                ) : field.type === "range" ? (
                  <div className="flex items-center gap-3">
                    <input
                      type="range"
                      min={field.min ?? 0}
                      max={field.max ?? 1}
                      step={field.step ?? 0.01}
                      value={state.values[field.key] || "0"}
                      onChange={(e) =>
                        dispatch({
                          type: "SET_FIELD",
                          key: field.key,
                          value: e.target.value,
                        })
                      }
                      className="h-1.5 flex-1 cursor-pointer appearance-none rounded-full bg-neutral-200 accent-neutral-900"
                    />
                    <span className="w-10 text-right font-mono text-xs text-neutral-500">
                      {Math.round(
                        parseFloat(state.values[field.key] || "0") * 100,
                      )}
                      %
                    </span>
                  </div>
                ) : (
                  <input
                    type="text"
                    value={state.values[field.key] || ""}
                    onChange={(e) =>
                      dispatch({
                        type: "SET_FIELD",
                        key: field.key,
                        value: e.target.value,
                      })
                    }
                    className="w-full rounded-lg border border-neutral-200 px-3 py-2 text-sm focus:border-neutral-400 focus:outline-none"
                  />
                )}
              </div>
            ))}
          </section>
        )}
      </div>

      <div className="mt-auto border-t border-neutral-200 p-4">
        <button
          onClick={handleExport}
          className="flex w-full items-center justify-center gap-2 rounded-lg bg-neutral-900 px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-neutral-800"
        >
          <Download size={16} />
          Export PNG
        </button>
      </div>
    </div>
  );
}
