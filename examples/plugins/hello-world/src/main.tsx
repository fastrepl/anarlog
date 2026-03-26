if (!window.__char_react || !window.__char_plugins) {
  throw new Error("Char plugin globals are unavailable");
}

const React = window.__char_react;

type LifecycleState = {
  status: string;
  sessionId: string | null;
  eventCount: number;
};

let lifecycleState: LifecycleState = {
  status: "inactive",
  sessionId: null,
  eventCount: 0,
};

type LifecycleSubscriber = (state: LifecycleState) => void;

const lifecycleSubscribers = new Set<LifecycleSubscriber>();

function emitLifecycleState() {
  for (const subscriber of lifecycleSubscribers) {
    subscriber(lifecycleState);
  }
}

function setLifecycleState(next: LifecycleState) {
  lifecycleState = next;
  emitLifecycleState();
}

function subscribeLifecycleState(subscriber: LifecycleSubscriber) {
  lifecycleSubscribers.add(subscriber);
  subscriber(lifecycleState);

  return () => {
    lifecycleSubscribers.delete(subscriber);
  };
}

function HelloWorldView() {
  const [count, setCount] = React.useState(0);
  const [lifecycle, setLifecycle] = React.useState(lifecycleState);

  React.useEffect(() => {
    return subscribeLifecycleState(setLifecycle);
  }, []);

  return (
    <div className="flex h-full items-center justify-center bg-neutral-50">
      <div className="w-full max-w-md rounded-xl border border-neutral-200 bg-white p-6 shadow-sm">
        <h1 className="text-lg font-semibold text-neutral-900">
          Hello from plugin
        </h1>
        <p className="mt-2 text-sm text-neutral-600">
          This tab is rendered from <code>examples/plugins/hello-world</code>.
        </p>
        <p className="mt-4 text-sm text-neutral-700">
          Listener lifecycle:{" "}
          <span className="font-medium">{lifecycle.status}</span>
        </p>
        <p className="mt-1 text-xs text-neutral-500">
          Session: {lifecycle.sessionId ?? "none"} / Events seen:{" "}
          {lifecycle.eventCount}
        </p>
        <div className="mt-4 flex items-center gap-3">
          <button
            className="rounded-md border border-neutral-300 px-3 py-1.5 text-sm text-neutral-700"
            onClick={() => setCount((value) => value + 1)}
            type="button"
          >
            Increment
          </button>
          <span className="text-sm text-neutral-500">Count: {count}</span>
        </div>
      </div>
    </div>
  );
}

window.__char_plugins.register({
  id: "hello-world",
  onload(ctx) {
    ctx.registerEvent(
      ctx.events.tauri.listener.sessionLifecycleEvent.listen(({ payload }) => {
        setLifecycleState({
          status: payload.type,
          sessionId: payload.session_id,
          eventCount: lifecycleState.eventCount + 1,
        });
      }),
    );

    ctx.registerView("hello-world", () => <HelloWorldView />);
    ctx.openTab("hello-world");
  },
});

export {};
