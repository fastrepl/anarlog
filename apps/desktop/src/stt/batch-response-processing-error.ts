export const BATCH_RESPONSE_PROCESSING_ERROR_MESSAGE =
  "Batch transcription completed, but Anarlog could not process the response.";

export class BatchResponseProcessingError extends Error {
  readonly cause: unknown;

  constructor(cause: unknown) {
    super(BATCH_RESPONSE_PROCESSING_ERROR_MESSAGE);
    this.name = "BatchResponseProcessingError";
    this.cause = cause;
  }
}
