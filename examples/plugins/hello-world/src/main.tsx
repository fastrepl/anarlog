if (!window.__char_react || !window.__char_plugins) {
  throw new Error("Char plugin globals are unavailable");
}

const React = window.__char_react;

const viewStyles = {
  root: {
    alignItems: "center",
    backgroundColor: "#fafafa",
    display: "flex",
    height: "100%",
    justifyContent: "center",
  },
  card: {
    backgroundColor: "#ffffff",
    border: "1px solid #e5e5e5",
    borderRadius: "0.75rem",
    boxShadow: "0 1px 2px 0 rgb(0 0 0 / 0.05)",
    boxSizing: "border-box",
    maxWidth: "28rem",
    padding: "1.5rem",
    width: "100%",
  },
  title: {
    color: "#171717",
    fontSize: "1.125rem",
    fontWeight: 600,
    lineHeight: "1.75rem",
    margin: 0,
  },
  description: {
    color: "#525252",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    margin: "0.5rem 0 0",
  },
  lifecycle: {
    color: "#404040",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    margin: "1rem 0 0",
  },
  lifecycleStatus: {
    fontWeight: 500,
  },
  session: {
    color: "#737373",
    fontSize: "0.75rem",
    lineHeight: "1rem",
    margin: "0.25rem 0 0",
  },
  actions: {
    alignItems: "center",
    display: "flex",
    gap: "0.75rem",
    marginTop: "1rem",
  },
  button: {
    backgroundColor: "transparent",
    border: "1px solid #d4d4d4",
    borderRadius: "0.375rem",
    color: "#404040",
    cursor: "pointer",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    padding: "0.375rem 0.75rem",
  },
  count: {
    color: "#737373",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
} satisfies Record<string, React.CSSProperties>;

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
    <div style={viewStyles.root}>
      <div style={viewStyles.card}>
        <h1 style={viewStyles.title}>Hello from plugin</h1>
        <p style={viewStyles.description}>
          This tab is rendered from <code>examples/plugins/hello-world</code>.
        </p>
        <p style={viewStyles.lifecycle}>
          Listener lifecycle:{" "}
          <span style={viewStyles.lifecycleStatus}>{lifecycle.status}</span>
        </p>
        <p style={viewStyles.session}>
          Session: {lifecycle.sessionId ?? "none"} / Events seen:{" "}
          {lifecycle.eventCount}
        </p>
        <div style={viewStyles.actions}>
          <button
            style={viewStyles.button}
            onClick={() => setCount((value) => value + 1)}
            type="button"
          >
            Increment
          </button>
          <span style={viewStyles.count}>Count: {count}</span>
        </div>
      </div>
    </div>
  );
}

window.__char_plugins.register({
  id: "hello-world",
  onload(ctx) {
    ctx.registerEvent(
      ctx.events.tauri.listener.captureLifecycleEvent.listen(({ payload }) => {
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
