import {
  completeCancelledAttachmentTransferDelete,
  completeUpload,
  completeWithoutTransfer,
  deferAttachmentTransferDeleteForPreservation,
  prepareAttachmentTransferDelete,
} from "./store/completion";
import {
  claimNextAttachmentTransferJob,
  failAttachmentTransferJob,
  markPhase,
  retryAttachmentTransferJob,
  setDownloadGrant,
  setUploadReservation,
  subscribeToNextAttachmentTransferAttempt,
} from "./store/jobs";
import {
  reconcileAttachmentTransferJobs,
  recoverInterruptedAttachmentTransfers,
  resetProcessLocalAttachmentTransferAttempts,
} from "./store/reconcile";

export type {
  AttachmentTransferDirection,
  AttachmentTransferJob,
  AttachmentTransferPhase,
} from "./store/types";
export {
  claimNextAttachmentTransferJob,
  completeCancelledAttachmentTransferDelete,
  completeUpload,
  deferAttachmentTransferDeleteForPreservation,
  failAttachmentTransferJob,
  markPhase,
  prepareAttachmentTransferDelete,
  reconcileAttachmentTransferJobs,
  recoverInterruptedAttachmentTransfers,
  resetProcessLocalAttachmentTransferAttempts,
  retryAttachmentTransferJob,
};

export const attachmentTransferStore = {
  reconcile: reconcileAttachmentTransferJobs,
  resetProcessLocalAttempts: resetProcessLocalAttachmentTransferAttempts,
  recoverInterrupted: recoverInterruptedAttachmentTransfers,
  claimNext: claimNextAttachmentTransferJob,
  subscribeToNextAttempt: subscribeToNextAttachmentTransferAttempt,
  setUploadReservation,
  setDownloadGrant,
  markPhase,
  prepareDelete: prepareAttachmentTransferDelete,
  completeCancelledDelete: completeCancelledAttachmentTransferDelete,
  deferDeleteForPreservation: deferAttachmentTransferDeleteForPreservation,
  completeUpload,
  completeWithoutTransfer,
  retry: retryAttachmentTransferJob,
  fail: failAttachmentTransferJob,
};
