import { useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";

export function GlitchyUpdateListener() {
  const qc = useQueryClient();
  useEffect(() => {
    const unlisten = listen("glitchy-update", () => {
      qc.invalidateQueries({ queryKey: ["glitchy"] });
      qc.invalidateQueries({ queryKey: ["glitchy", "get"] });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [qc]);
  return null;
}
