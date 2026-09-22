import {
  AlertCircleIcon,
  Cancel01Icon,
  CheckmarkBadge01Icon,
  PauseIcon,
  PlayIcon,
  Settings01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { ActionButton } from "@/components/ui/action-button";
import { useBackendMutation } from "@/hooks/use-backend";
import { downloadPhaseLabels, formatBytes } from "@/lib/downloads";
import { useDownloads } from "@/stores/downloads";

/**
 * Wizard step 3 — live view of a background download session.
 *
 * Reads from the global downloads store (fed by backend events), so
 * the progress shown here is identical to the DownloadsPanel and keeps
 * updating even if this component remounts after navigation.
 */
export function StepInstalling({
  sessionId,
  instanceName,
  onReset,
}: {
  sessionId: string | null;
  instanceName: string;
  onReset: () => void;
}) {
  const session = useDownloads((s) =>
    sessionId ? s.sessions[sessionId] : undefined
  );
  const { mutateAsync: pause } = useBackendMutation({ name: "pause_download" });
  const { mutateAsync: resume } = useBackendMutation({
    name: "resume_download",
  });
  const { mutateAsync: cancel } = useBackendMutation({
    name: "cancel_download",
  });

  // Session not in the store (yet) — backend creates it right before
  // the first event arrives; show an indeterminate state meanwhile.
  if (!session) {
    return (
      <div className="zoom-in-95 flex h-full animate-in flex-col items-center justify-center duration-300">
        <HugeiconsIcon
          className="animate-spin text-primary"
          icon={Settings01Icon}
          size={48}
        />
        <p className="mt-6 text-muted-foreground text-sm">
          Starting download...
        </p>
      </div>
    );
  }

  const isDone = session.state === "completed";
  const isFailed = session.state === "failed" || session.state === "cancelled";
  const isActive = session.state === "running" || session.state === "paused";
  const phaseText = downloadPhaseLabels[session.phase] ?? session.phase;

  return (
    <div className="zoom-in-95 flex h-full animate-in flex-col items-center justify-center duration-300">
      <div className="mb-8 rounded-full border border-border/40 bg-secondary/40 p-4">
        {isDone ? (
          <HugeiconsIcon
            className="spin-in-12 animate-in text-emerald-500 duration-500"
            icon={CheckmarkBadge01Icon}
            size={48}
          />
        ) : isFailed ? (
          <HugeiconsIcon
            className="text-destructive"
            icon={AlertCircleIcon}
            size={48}
          />
        ) : (
          <HugeiconsIcon
            className={
              session.state === "running"
                ? "animate-spin text-primary"
                : "text-amber-500"
            }
            icon={Settings01Icon}
            size={48}
          />
        )}
      </div>

      <h2 className="mb-2 font-bold text-2xl text-foreground tracking-tight">
        {isDone
          ? "Installation Complete!"
          : session.state === "paused"
            ? "Paused"
            : isFailed
              ? session.state === "cancelled"
                ? "Download Cancelled"
                : "Download Failed"
              : "Installing..."}
      </h2>

      <p className="mb-8 max-w-[80%] truncate text-center text-muted-foreground text-sm">
        {isDone
          ? `Successfully installed ${instanceName || session.label}`
          : isFailed
            ? (session.error ??
              "You can start the download again from the wizard.")
            : `${phaseText}${session.currentFile ? ` — ${session.currentFile}` : ""}`}
      </p>

      <div className="mb-8 w-full max-w-md space-y-2">
        <div className="h-3.5 w-full overflow-hidden rounded-full border border-border/60 bg-background p-0.5 shadow-inner">
          <div
            className={`h-full rounded-full transition-all duration-300 ease-out ${
              isDone
                ? "bg-emerald-500"
                : session.state === "paused"
                  ? "bg-amber-500"
                  : isFailed
                    ? "bg-destructive"
                    : "bg-primary"
            }`}
            style={{
              width: `${Math.min(Math.max(session.percent, 0), 100)}%`,
            }}
          />
        </div>
        <div className="text-right font-bold text-muted-foreground text-xs">
          {String(session.percent)}%
        </div>
        {session.totalBytes > 0 && (
          <div className="grid grid-cols-3 gap-2 pt-2 text-center">
            <DownloadMetric
              label="Downloaded"
              value={formatBytes(session.downloadedBytes)}
            />
            <DownloadMetric
              label="Total"
              value={formatBytes(session.totalBytes)}
            />
            <DownloadMetric
              label="Speed"
              value={`${formatBytes(session.bytesPerSecond)}/s`}
            />
          </div>
        )}
      </div>

      {isActive && sessionId && (
        <div className="flex gap-2">
          {session.state === "running" && (
            <ActionButton
              action={async () => {
                await pause({ sessionId });
              }}
              className="gap-2 px-6"
              variant="secondary"
            >
              <HugeiconsIcon icon={PauseIcon} size={16} />
              Pause
            </ActionButton>
          )}
          {session.state === "paused" && (
            <ActionButton
              action={async () => {
                await resume({ sessionId });
              }}
              className="gap-2 px-6"
            >
              <HugeiconsIcon icon={PlayIcon} size={16} />
              Continue
            </ActionButton>
          )}
          <ActionButton
            action={async () => {
              await cancel({ sessionId });
            }}
            className="gap-2 px-6"
            variant="destructive"
          >
            <HugeiconsIcon icon={Cancel01Icon} size={16} />
            Cancel
          </ActionButton>
        </div>
      )}

      {!isActive && (
        <ActionButton
          action={async () => onReset()}
          className="px-8"
          variant="secondary"
        >
          {isDone ? "Finish & Go Back" : "Back to Wizard"}
        </ActionButton>
      )}
    </div>
  );
}

function DownloadMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-border/50 bg-background/60 px-2 py-2">
      <div className="font-bold text-foreground text-xs tabular-nums">
        {value}
      </div>
      <div className="mt-0.5 text-[10px] text-muted-foreground uppercase tracking-wider">
        {label}
      </div>
    </div>
  );
}
