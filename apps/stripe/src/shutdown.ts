export async function drainServer(
  server: Pick<Bun.Server<unknown>, "stop">,
  stopWorker: () => Promise<void>,
) {
  const results = await Promise.allSettled([
    Promise.resolve().then(() => server.stop()),
    Promise.resolve().then(stopWorker),
  ]);
  const errors = results.flatMap((result) =>
    result.status === "rejected" ? [result.reason] : [],
  );
  if (errors.length > 0) {
    throw new AggregateError(errors, "Billing shutdown failed");
  }
}
