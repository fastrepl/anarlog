import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation } from "@tanstack/react-query";
import { useRef, useState } from "react";

import {
  CircleNotch,
  FileText,
  Plus,
  Trash,
  UploadSimple,
} from "@anlg/ui/components/icons";
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
      <DialogContent className="max-h-[85vh] max-w-2xl gap-0 overflow-hidden p-0">
        <DialogHeader className="border-border border-b px-6 py-5 pr-12">
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

        <div className="flex max-h-[60vh] flex-col gap-4 overflow-y-auto px-6 py-5">
          <div className="bg-muted/50 text-muted-foreground flex items-start gap-2 rounded-lg px-3 py-2.5 text-xs">
            <FileText className="mt-0.5 size-4 shrink-0" />
            <Trans>
              Examples are used only to improve this format. They are not saved
              or reused for future meetings.
            </Trans>
          </div>

          {examples.map((example, index) => (
            <div key={index} className="flex flex-col gap-2">
              <div className="flex items-center justify-between gap-2">
                <label
                  htmlFor={`auto-format-example-${index}`}
                  className="text-sm font-medium"
                >
                  <Trans>Example summary</Trans> {index + 1}
                </label>
                <Button
                  type="button"
                  size="icon"
                  variant="ghost"
                  className="text-muted-foreground size-7"
                  aria-label={t`Remove example ${index + 1}`}
                  onClick={() => removeExample(index)}
                  disabled={inferenceMutation.isPending}
                >
                  <Trash className="size-4" />
                </Button>
              </div>
              <Textarea
                id={`auto-format-example-${index}`}
                value={example}
                maxLength={MAX_FORMAT_EXAMPLE_LENGTH}
                onChange={(event) => updateExample(index, event.target.value)}
                placeholder={t`Paste a past summary you like...`}
                className="min-h-36 resize-y font-mono text-sm leading-5"
                disabled={inferenceMutation.isPending}
              />
            </div>
          ))}

          <div className="flex flex-wrap items-center gap-2">
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
              <Plus className="size-4" />
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
              <UploadSimple className="size-4" />
              <Trans>Attach Markdown or text</Trans>
            </Button>
            <span className="text-muted-foreground ml-auto text-xs">
              {populatedExamples.length} / {MAX_FORMAT_EXAMPLES}
            </span>
            <input
              ref={fileInputRef}
              type="file"
              accept=".md,.markdown,.txt,text/markdown,text/plain"
              multiple
              className="hidden"
              onChange={(event) => {
                void uploadExamples(event.currentTarget.files).catch((error) =>
                  sonnerToast.error(error.message),
                );
                event.currentTarget.value = "";
              }}
            />
          </div>
        </div>

        <DialogFooter className="border-border border-t px-6 py-4">
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
              <CircleNotch className="size-4 animate-spin" />
            ) : null}
            <Trans>Improve format</Trans>
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

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
