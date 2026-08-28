import { Trans } from "@lingui/react/macro";
import { CircleNotch } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { MessageBubble, MessageContainer } from "./shared";

export function LoadingMessage() {
  return (
    <MessageContainer align="start">
      <MessageBubble variant="loading">
        <div {...stylex.props(styles.content)}>
          <CircleNotch {...stylex.props(styles.spinner)} />
          <span {...stylex.props(styles.text)}>
            <Trans>Thinking...</Trans>
          </span>
        </div>
      </MessageBubble>
    </MessageContainer>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  content: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "1rem",
    width: "1rem",
  },
  text: {
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
  },
});
