import { ModalClient } from "modal";

import { env } from "../env";

let modalClient: ModalClient | null = null;

export function getModalClient(): ModalClient {
  if (!modalClient) {
    modalClient = new ModalClient({
      tokenId: env.MODAL_TOKEN_ID,
      tokenSecret: env.MODAL_TOKEN_SECRET,
    });
  }
  return modalClient;
}
