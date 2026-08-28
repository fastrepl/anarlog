import { Trans, useLingui } from "@lingui/react/macro";
import {
  CircleNotch,
  FileText,
  Plus,
  Trash,
  UploadSimple,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import { useRef, useState } from "react";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";
import { Textarea } from "@anlg/ui/components/ui/textarea";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import {
  inferSummaryFormat,
  MAX_FORMAT_EXAMPLE_LENGTH,
  MAX_FORMAT_EXAMPLES,
} from "./auto-format-inference";

import { useLanguageModel } from "~/ai/hooks";

export function AutoFormatExamplesDialog({
  onClose,
  onGenerated,
}: {
  onClose: () => void;
  onGenerated: (format: string) => void;
}) {
  const { t } = useLingui();
  const model = useLanguageModel("enhance");
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [examples, setExamples] = useState([""]);
  const populatedExamples = examples
    .map((example) => example.trim())
    .filter(Boolean);

  const inferenceMutation = useMutation({
    mutationFn: async () => {
      if (!model) {
        throw new Error(t`Choose an AI model before generating a format.`);
      }
      if (populatedExamples.length === 0) {
        throw new Error(t`Add at least one example summary.`);
      }

      return inferSummaryFormat({ model, examples: populatedExamples });
    },
    onSuccess: (format) => {
      onGenerated(format);
      onClose();
    },
    onError: (error) => sonnerToast.error(error.message),
  });

  const updateExample = (index: number, value: string) => {
    setExamples((current) =>
      current.map((example, currentIndex) =>
        currentIndex === index ? value : example,
      ),
    );
  };

  const removeExample = (index: number) => {
    setExamples((current) => {
      const remaining = current.filter(
        (_, currentIndex) => currentIndex !== index,
      );
      return remaining.length > 0 ? remaining : [""];
    });
  };

  const uploadExamples = async (files: FileList | null) => {
    const currentCount = populatedExamples.length;
    const availableSlots = MAX_FORMAT_EXAMPLES - currentCount;
    const selectedFiles = Array.from(files ?? []);

    if (selectedFiles.length > availableSlots) {
      sonnerToast.error(t`You can use up to three example summaries.`);
    }

    const loadedExamples: string[] = [];
    for (const file of selectedFiles.slice(0, availableSlots)) {
      if (!isTextExample(file)) {
        sonnerToast.error(t`Examples must be Markdown or plain text files.`);
        continue;
      }

      const content = (await file.text()).replace(/\r\n/g, "\n").trim();
      if (content.length > MAX_FORMAT_EXAMPLE_LENGTH) {
        sonnerToast.error(t`Each example must be 12,000 characters or fewer.`);
        continue;
      }
      if (content) loadedExamples.push(content);
    }

    if (loadedExamples.length > 0) {
      setExamples((current) => {
        const populated = current.filter((example) => example.trim());
        return [...populated, ...loadedExamples].slice(0, MAX_FORMAT_EXAMPLES);
      });
    }
  };

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !inferenceMutation.isPending) onClose();
      }}
    >
      <DialogContent sx={styles.content}>
        <DialogHeader sx={styles.header}>
          <DialogTitle>
            <Trans>Improve summary format</Trans>
          </DialogTitle>
          <DialogDescription>
            <Trans>
              Attach up to three past summaries you like. Anarlog will learn how
              you prefer meeting notes to be structured and written.
            </Trans>
          </DialogDescription>
        </DialogHeader>

        <div {...stylex.props(styles.body)}>
          <div {...stylex.props(styles.notice)}>
            <FileText {...stylex.props(styles.noticeIcon)} />
            <Trans>
              Examples are used only to improve this format. They are not saved
              or reused for future meetings.
            </Trans>
          </div>

          {examples.map((example, index) => (
            <div key={index} {...stylex.props(styles.example)}>
              <div {...stylex.props(styles.exampleHeader)}>
                <label
                  htmlFor={`auto-format-example-${index}`}
                  {...stylex.props(styles.label)}
                >
                  <Trans>Example summary</Trans> {index + 1}
                </label>
                <Button
                  type="button"
                  size="icon"
                  variant="ghost"
                  sx={styles.removeButton}
                  aria-label={t`Remove example ${index + 1}`}
                  onClick={() => removeExample(index)}
                  disabled={inferenceMutation.isPending}
                >
                  <Trash {...stylex.props(styles.icon)} />
                </Button>
              </div>
              <Textarea
                id={`auto-format-example-${index}`}
                value={example}
                maxLength={MAX_FORMAT_EXAMPLE_LENGTH}
                onChange={(event) => updateExample(index, event.target.value)}
                placeholder={t`Paste a past summary you like...`}
                sx={styles.textarea}
                disabled={inferenceMutation.isPending}
              />
            </div>
          ))}

          <div {...stylex.props(styles.actions)}>
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => setExamples((current) => [...current, ""])}
              disabled={
                examples.length >= MAX_FORMAT_EXAMPLES ||
                inferenceMutation.isPending
              }
            >
              <Plus {...stylex.props(styles.icon)} />
              <Trans>Add example</Trans>
            </Button>
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => fileInputRef.current?.click()}
              disabled={
                populatedExamples.length >= MAX_FORMAT_EXAMPLES ||
                inferenceMutation.isPending
              }
            >
              <UploadSimple {...stylex.props(styles.icon)} />
              <Trans>Attach Markdown or text</Trans>
            </Button>
            <span {...stylex.props(styles.count)}>
              {populatedExamples.length} / {MAX_FORMAT_EXAMPLES}
            </span>
            <input
              ref={fileInputRef}
              type="file"
              accept=".md,.markdown,.txt,text/markdown,text/plain"
              multiple
              {...stylex.props(styles.hidden)}
              onChange={(event) => {
                void uploadExamples(event.currentTarget.files).catch((error) =>
                  sonnerToast.error(error.message),
                );
                event.currentTarget.value = "";
              }}
            />
          </div>
        </div>

        <DialogFooter sx={styles.footer}>
          <Button
            type="button"
            variant="ghost"
            onClick={onClose}
            disabled={inferenceMutation.isPending}
          >
            <Trans>Cancel</Trans>
          </Button>
          <Button
            type="button"
            onClick={() => inferenceMutation.mutate()}
            disabled={
              populatedExamples.length === 0 || inferenceMutation.isPending
            }
          >
            {inferenceMutation.isPending ? (
              <CircleNotch {...stylex.props(styles.spinner)} />
            ) : null}
            <Trans>Improve format</Trans>
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  actions: {
    alignItems: "center",
    display: "flex",
    flexWrap: "wrap",
    gap: "0.5rem",
  },
  body: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
    maxHeight: "60vh",
    overflowY: "auto",
    paddingBlock: "1.25rem",
    paddingInline: "1.5rem",
  },
  content: {
    gap: 0,
    maxHeight: "85vh",
    maxWidth: "42rem",
    overflow: "hidden",
    padding: 0,
  },
  count: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
    marginLeft: "auto",
  },
  example: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  exampleHeader: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    justifyContent: "space-between",
  },
  footer: {
    borderTopColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    paddingBlock: "1rem",
    paddingInline: "1.5rem",
  },
  header: {
    borderBottomColor: colors.border,
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    paddingBlock: "1.25rem",
    paddingLeft: "1.5rem",
    paddingRight: "3rem",
  },
  hidden: {
    display: "none",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  label: {
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
  },
  notice: {
    alignItems: "flex-start",
    backgroundColor: `color-mix(in srgb, ${colors.muted} 50%, transparent)`,
    borderRadius: radii.lg,
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.5rem",
    lineHeight: "1rem",
    paddingBlock: "0.625rem",
    paddingInline: "0.75rem",
  },
  noticeIcon: {
    flexShrink: 0,
    height: "1rem",
    marginTop: "0.125rem",
    width: "1rem",
  },
  removeButton: {
    color: colors.mutedForeground,
    height: "1.75rem",
    width: "1.75rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "1rem",
    width: "1rem",
  },
  textarea: {
    fontFamily: fonts.mono,
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    minHeight: "9rem",
    resize: "vertical",
  },
});

function isTextExample(file: File): boolean {
  const extension = file.name.split(".").pop()?.toLowerCase();
  return (
    file.type === "text/plain" ||
    file.type === "text/markdown" ||
    extension === "txt" ||
    extension === "md" ||
    extension === "markdown"
  );
}
