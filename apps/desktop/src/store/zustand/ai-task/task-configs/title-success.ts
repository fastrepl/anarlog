import type { TaskConfig } from ".";

const onSuccess: NonNullable<TaskConfig<"title">["onSuccess"]> = ({
  text,
  args,
  store,
}) => {
  if (!text) {
    return;
  }

  const trimmed = text.trim();
  if (!trimmed || trimmed === "<EMPTY>") {
    return;
  }

  store.setPartialRow("sessions", args.sessionId, { title: trimmed });
};

export const titleSuccess: Pick<TaskConfig<"title">, "onSuccess"> = {
  onSuccess,
};
