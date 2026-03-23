import { ChevronDown, ChevronUp, RefreshCcw } from "lucide-react";
import { type ReactNode, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@hypr/ui/components/ui/dialog";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { cn } from "@hypr/utils";

interface ConnectedServiceCardProps {
  icon: ReactNode;
  title: string;
  subtitle?: string;
  children?: ReactNode;
  onSync?: () => Promise<void>;
  onReconnect?: () => Promise<void>;
  onDisconnect?: () => void;
  connectedAt?: string;
  showAdvanced?: boolean;
  disconnectDialogTitle?: string;
  disconnectDialogDescription?: ReactNode;
}

function formatConnectionDate(isoDate: string) {
  const date = new Date(isoDate);
  return date.toLocaleDateString("en-US", {
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

export function ConnectedServiceCard({
  icon,
  title,
  subtitle,
  children,
  onSync,
  onReconnect,
  onDisconnect,
  connectedAt,
  showAdvanced = true,
  disconnectDialogTitle = "Disconnect Service?",
  disconnectDialogDescription,
}: ConnectedServiceCardProps) {
  const [isSyncing, setIsSyncing] = useState(false);
  const [isAdvancedOpen, setIsAdvancedOpen] = useState(false);
  const [showDisconnectDialog, setShowDisconnectDialog] = useState(false);
  const [isReconnecting, setIsReconnecting] = useState(false);

  const handleSync = async () => {
    if (!onSync) {
      return;
    }
    setIsSyncing(true);
    try {
      await onSync();
    } finally {
      setIsSyncing(false);
    }
  };

  const handleReconnect = async () => {
    if (!onReconnect) {
      return;
    }
    setIsReconnecting(true);
    try {
      await onReconnect();
    } finally {
      setIsReconnecting(false);
    }
  };

  const handleDisconnect = () => {
    setShowDisconnectDialog(false);
    onDisconnect?.();
  };

  return (
    <div className="overflow-clip rounded-lg border border-neutral-200">
      <div className="flex items-center justify-between border-b py-2 pr-2 pl-4 text-sm font-medium">
        <div className="flex items-center gap-4">
          {icon}
          <div>
            <div>{title}</div>
            {subtitle && (
              <div className="text-xs font-normal text-neutral-600">
                {subtitle}
              </div>
            )}
          </div>
        </div>

        {onSync && (
          <Button
            size="icon"
            variant="ghost"
            onClick={handleSync}
            disabled={isSyncing}
          >
            {isSyncing ? <Spinner size={16} /> : <RefreshCcw size={16} />}
          </Button>
        )}
      </div>

      {children && <div className="p-4">{children}</div>}

      {showAdvanced && (onReconnect || onDisconnect || connectedAt) && (
        <div className="border-t border-neutral-200">
          <button
            onClick={() => setIsAdvancedOpen(!isAdvancedOpen)}
            className={cn([
              "flex w-full items-center justify-between px-4 py-2 text-sm text-neutral-600 transition-colors hover:bg-neutral-50",
              isAdvancedOpen && "bg-neutral-50",
            ])}
          >
            Advanced
            {isAdvancedOpen ? (
              <ChevronUp size={16} />
            ) : (
              <ChevronDown size={16} />
            )}
          </button>

          {isAdvancedOpen && (
            <div className="flex flex-col gap-3 bg-neutral-50 px-4 pb-4">
              {connectedAt && (
                <div className="text-xs text-neutral-600">
                  <span className="font-medium">Connected on:</span>{" "}
                  {formatConnectionDate(connectedAt)}
                </div>
              )}

              {(onReconnect || onDisconnect) && (
                <div className="flex gap-2">
                  {onReconnect && (
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={handleReconnect}
                      disabled={isReconnecting}
                      className="flex-1"
                    >
                      {isReconnecting ? (
                        <>
                          <Spinner size={14} className="mr-2" />
                          Reconnecting...
                        </>
                      ) : (
                        "Reconnect"
                      )}
                    </Button>
                  )}

                  {onDisconnect && (
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => setShowDisconnectDialog(true)}
                      className="flex-1 border-red-200 text-red-600 hover:border-red-300 hover:bg-red-50 hover:text-red-700"
                    >
                      Disconnect
                    </Button>
                  )}
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {onDisconnect && (
        <Dialog
          open={showDisconnectDialog}
          onOpenChange={setShowDisconnectDialog}
        >
          <DialogContent>
            <DialogHeader>
              <DialogTitle>{disconnectDialogTitle}</DialogTitle>
              <DialogDescription>
                {disconnectDialogDescription ||
                  "Are you sure you want to disconnect this service?"}
              </DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <Button
                variant="outline"
                onClick={() => setShowDisconnectDialog(false)}
              >
                Cancel
              </Button>
              <Button variant="destructive" onClick={handleDisconnect}>
                Disconnect
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      )}
    </div>
  );
}
