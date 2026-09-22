import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { toast } from "sonner";
import type { DownloadSessionInfo } from "@/invokes";
import { queryClient } from "@/lib/query-client";
import { backend } from "@/lib/utils";
import { useDownloads } from "@/stores/downloads";

/**
 * Keeps the global downloads store in sync with the backend.
 *
 * Mounted ONCE in the layout (outside the routed pages) so download
 * progress keeps updating while the user browses other pages. On mount
 * it also pulls the current session list via `get_active_downloads` so
 * a page reload / navigation restores every running download.
 */
export function DownloadsListener() {
  const upsert = useDownloads((s) => s.upsert);
  const upsertMany = useDownloads((s) => s.upsertMany);

  useEffect(() => {
    let disposed = false;

    // Re-sync with the backend registry in case events were missed
    // (window reload, listener race on startup).
    backend("get_active_downloads")
      .then((infos) => {
        if (!disposed) {
          upsertMany(infos);
        }
      })
      .catch(() => {});

    const unlisten = listen<DownloadSessionInfo>(
      "download-session",
      (event) => {
        const info = event.payload;
        upsert(info);
        if (info.state === "completed") {
          queryClient.invalidateQueries({
            queryKey: ["get", "installed", "versions"],
          });
          queryClient.invalidateQueries({
            queryKey: ["get", "non", "installed", "versions"],
          });
          toast.success(`${info.label} installed`, {
            description: "You can now select and play it.",
          });
        } else if (info.state === "failed") {
          toast.error(`Download failed: ${info.label}`, {
            description: info.error ?? undefined,
          });
        } else if (info.state === "cancelled") {
          toast.info(`Download cancelled: ${info.label}`);
        }
      }
    );

    return () => {
      disposed = true;
      unlisten.then((fn) => fn());
    };
  }, [upsert, upsertMany]);

  return null;
}
