import type { DownloadSessionState } from "@/invokes";

/** Human-readable label for each download session phase. */
export const downloadPhaseLabels: Record<string, string> = {
  assets: "Downloading Assets",
  client: "Downloading Client",
  done: "Finishing up",
  java: "Downloading Java",
  libraries: "Downloading Libraries",
  loader: "Loader installer",
  modpack: "Installing Modpack",
  prepare: "Preparing",
};

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB"] as const;
  const unitIndex = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1
  );
  const value = bytes / 1024 ** unitIndex;
  return `${value >= 100 || unitIndex === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unitIndex]}`;
}

/** Tailwind classes for each session state badge / bar color. */
export const sessionStateColor: Record<
  DownloadSessionState,
  { badge: string; bar: string }
> = {
  cancelled: {
    badge: "bg-muted text-muted-foreground",
    bar: "bg-muted-foreground",
  },
  completed: {
    badge: "bg-emerald-500/10 text-emerald-500",
    bar: "bg-emerald-500",
  },
  failed: {
    badge: "bg-destructive/10 text-destructive",
    bar: "bg-destructive",
  },
  paused: { badge: "bg-amber-500/10 text-amber-500", bar: "bg-amber-500" },
  running: { badge: "bg-primary/10 text-primary", bar: "bg-primary" },
};
