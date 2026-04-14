import { useListener } from "~/stt/contexts";

export function MeetingOverCTA() {
  const stop = useListener((state) => state.stop);
  const setMicStoppedPending = useListener(
    (state) => state.setMicStoppedPending,
  );

  return (
    <div className="flex items-center gap-2 rounded-full border-2 border-stone-600 bg-stone-800 px-4 py-2 text-sm text-white shadow-[0_4px_14px_rgba(87,83,78,0.4)]">
      <span>Is your meeting over?</span>
      <button
        type="button"
        onClick={() => {
          stop();
          setMicStoppedPending(false);
        }}
        className="rounded-full bg-white px-3 py-1 text-xs font-medium text-stone-800 transition-colors hover:bg-stone-200"
      >
        Yes
      </button>
      <button
        type="button"
        onClick={() => setMicStoppedPending(false)}
        className="rounded-full border border-stone-500 px-3 py-1 text-xs font-medium text-stone-300 transition-colors hover:bg-stone-700"
      >
        No
      </button>
    </div>
  );
}
