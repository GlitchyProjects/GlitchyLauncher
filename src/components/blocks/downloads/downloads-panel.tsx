import {
  AlertCircleIcon,
  ArrowReloadHorizontalIcon,
  Cancel01Icon,
  CheckmarkCircle01Icon,
  Delete02Icon,
  Loading02Icon,
  PauseIcon,
  PlayIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useMemo } from "react";
import { ActionButton } from "@/components/ui/action-button";
import { useBackendMutation } from "@/hooks/use-backend";
import type { DownloadSessionInfo } from "@/invokes";
import {
  downloadPhaseLabels,
  formatBytes,
  sessionStateColor,
} from "@/lib/downloads";
import { useDownloads } from "@/stores/downloads";

function StateBadge({ state }: { state: DownloadSessionInfo["state"] }) {
  const labels: Record<DownloadSessionInfo["state"], string> = {
    cancelled: "Cancelled",
    completed: "Completed",
    failed: "Failed",
    paused: "Paused",
    running: "Downloading",
  };
  return (
    <span
      className={`rounded-full px-2.5 py-0.5 font-bold text-[11px] uppercase tracking-wider ${sessionStateColor[state].badge}`}
    >
      {labels[state]}
    </span>
  );
}

function SessionControls({ session }: { session: DownloadSessionInfo }) {
  const { mutateAsync: pause } = useBackendMutation({
    name: "pause_download",
  });
  const { mutateAsync: resume } = useBackendMutation({
    name: "resume_download",
  });
  const { mutateAsync: cancel } = useBackendMutation({
    name: "cancel_download",
  });
  const dismiss = useDownloads((s) => s.dismiss);
  const dismissFinished = useDownloads((s) => s.dismissFinished);
  const clearFinished = useBackendMutation({
    name: "clear_finished_downloads",
  });

  const active = session.state === "running" || session.state === "paused";

  return (
    <div className="flex shrink-0 items-center gap-1.5">
      {session.state === "running" && (
        <ActionButton
          action={async () => {
            await pause({ sessionId: session.id });
          }}
          className="h-8 gap-1.5 px-3 text-xs"
          title="Pause download"
          variant="secondary"
        >
          <HugeiconsIcon icon={PauseIcon} size={14} />
          Pause
        </ActionButton>
      )}
      {session.state === "paused" && (
        <ActionButton
          action={async () => {
            await resume({ sessionId: session.id });
          }}
          className="h-8 gap-1.5 px-3 text-xs"
          title="Continue download"
          variant="default"
        >
          <HugeiconsIcon icon={PlayIcon} size={14} />
          Continue
        </ActionButton>
      )}
      {active && (
        <ActionButton
          action={async () => {
            await cancel({ sessionId: session.id });
          }}
          className="h-8 gap-1.5 px-3 text-xs"
          title="Cancel download"
          variant="destructive"
        >
          <HugeiconsIcon icon={Cancel01Icon} size={14} />
          Cancel
        </ActionButton>
      )}
      {!active && (
        <>
          <ActionButton
            action={async () => {
              await clearFinished.mutateAsync();
              dismissFinished();
            }}
            className="h-8 gap-1.5 px-3 text-xs"
            title="Clear finished downloads"
            variant="ghost"
          >
            <HugeiconsIcon icon={ArrowReloadHorizontalIcon} size={14} />
            Clear finished
          </ActionButton>
          <ActionButton
            action={async () => {
              dismiss(session.id);
            }}
            className="h-8 px-2 text-xs"
            title="Hide this entry"
            variant="ghost"
          >
            <HugeiconsIcon icon={Delete02Icon} size={14} />
          </ActionButton>
        </>
      )}
    </div>
  );
}

function SessionRow({ session }: { session: DownloadSessionInfo }) {
  const active = session.state === "running" || session.state === "paused";
  const phaseText = downloadPhaseLabels[session.phase] ?? session.phase;

  return (
    <div className="rounded-xl border border-border/50 bg-background/60 p-3.5">
      <div className="mb-2 flex items-center gap-3">
        <div
          className={`rounded-lg border border-border/40 bg-secondary/40 p-2 ${
            session.state === "completed"
              ? "text-emerald-500"
              : active
                ? "text-primary"
                : "text-muted-foreground"
          }`}
        >
          {session.state === "completed" ? (
            <HugeiconsIcon icon={CheckmarkCircle01Icon} size={18} />
          ) : session.state === "failed" ? (
            <HugeiconsIcon icon={AlertCircleIcon} size={18} />
          ) : (
            <HugeiconsIcon
              className={
                active && session.state === "running" ? "animate-spin" : ""
              }
              icon={Loading02Icon}
              size={18}
            />
          )}
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate font-semibold text-foreground text-sm">
              {session.label}
            </span>
            <StateBadge state={session.state} />
          </div>
          <div className="truncate font-mono text-muted-foreground text-xs">
            {active
              ? `${phaseText}${session.currentFile ? ` — ${session.currentFile}` : ""}`
              : (session.error ?? session.versionId)}
          </div>
        </div>

        <span className="shrink-0 font-bold text-muted-foreground text-sm tabular-nums">
          {session.percent}%
        </span>
      </div>

      <div className="mb-2.5 h-2 w-full overflow-hidden rounded-full border border-border/60 bg-secondary/40 p-0.5">
        <div
          className={`h-full rounded-full transition-all duration-300 ease-out ${sessionStateColor[session.state].bar}`}
          style={{ width: `${Math.min(Math.max(session.percent, 0), 100)}%` }}
        />
      </div>

      <div className="flex items-center justify-between gap-2">
        <div className="min-w-0 text-[11px] text-muted-foreground">
          <span className="uppercase tracking-wider">
            {phaseText}
            {session.state === "paused" ? " · paused" : ""}
          </span>
          {session.totalBytes > 0 && (
            <div className="mt-1 flex flex-wrap gap-x-3 font-mono tabular-nums">
              <span>{formatBytes(session.downloadedBytes)} downloaded</span>
              <span>{formatBytes(session.totalBytes)} total</span>
              <span>{formatBytes(session.bytesPerSecond)}/s</span>
            </div>
          )}
        </div>
        <SessionControls session={session} />
      </div>
    </div>
  );
}

/**
 * Live list of background downloads (running, paused and finished).
 * Reads from the global downloads store, so it fully survives page
 * navigation and app reloads.
 *
 * NOTE: select `order` and `sessions` separately and derive the list
 * with useMemo. A selector that builds a fresh array on every call
 * makes useSyncExternalStore loop forever (React error #185).
 */
export function DownloadsPanel() {
  const order = useDownloads((s) => s.order);
  const sessions = useDownloads((s) => s.sessions);
  const sessionList = useMemo(
    () => order.map((id) => sessions[id]).filter(Boolean),
    [order, sessions]
  );

  if (sessionList.length === 0) {
    return null;
  }

  return (
    <div className="mb-4 w-full">
      <h3 className="mb-2 px-1 font-bold text-muted-foreground text-xs uppercase tracking-wider">
        Downloads ({sessionList.length})
      </h3>
      <div className="scrollbar-thin max-h-[45vh] space-y-2 overflow-y-auto pr-1">
        {sessionList.map((session) => (
          <SessionRow key={session.id} session={session} />
        ))}
      </div>
    </div>
  );
}
