import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink, Gamepad2, ShieldCheck } from "lucide-react";
import { Button } from "@/components/ui/button";

const PVP_CLIENTS = [
  {
    description:
      "Popular all-in-one PvP client and launcher with performance and competitive features.",
    name: "Lunar Client",
    note: "Official Windows installer",
    url: "https://www.lunarclient.com/download",
  },
  {
    description:
      "Badlion is now launched through Lunar Client; this opens Badlion's official download route.",
    name: "Badlion Client",
    note: "Delivered through Lunar",
    url: "https://www.badlion.net/en/download",
  },
  {
    description:
      "A customizable Minecraft client and launcher focused on performance, mods, and PvP.",
    name: "Feather Client",
    note: "Official download page",
    url: "https://feathermc.com/a/",
  },
  {
    description:
      "A long-running Minecraft client platform with its own launcher and a cross-launcher installer.",
    name: "LabyMod",
    note: "Official launcher / installer",
    url: "https://www.labymod.net/download",
  },
] as const;

export function PvpClientDownloads() {
  return (
    <div className="scrollbar-thin min-h-0 flex-1 space-y-3 overflow-y-auto pe-1">
      <div className="rounded-xl border border-amber-500/20 bg-amber-500/5 p-4">
        <div className="flex items-center gap-2 font-bold text-sm">
          <ShieldCheck className="size-4 text-amber-500" /> Official installers
          only
        </div>
        <p className="mt-1 text-muted-foreground text-xs leading-relaxed">
          These proprietary clients maintain separate launchers and cannot be
          safely converted into Glitchy Minecraft profiles. The buttons open
          each publisher's official installer route—no mirrors or repackaged
          binaries.
        </p>
      </div>

      <div className="grid gap-3 md:grid-cols-2">
        {PVP_CLIENTS.map((client) => (
          <article
            className="flex min-h-40 flex-col rounded-xl border border-border/50 bg-background/55 p-4"
            key={client.name}
          >
            <div className="flex items-center gap-2">
              <Gamepad2 className="size-5 text-primary" />
              <h2 className="font-bold text-sm">{client.name}</h2>
            </div>
            <p className="mt-2 flex-1 text-muted-foreground text-xs leading-relaxed">
              {client.description}
            </p>
            <div className="mt-4 flex items-center justify-between gap-2">
              <span className="text-[10px] text-muted-foreground">
                {client.note}
              </span>
              <Button
                className="gap-1.5"
                onClick={() => openUrl(client.url)}
                size="sm"
              >
                <ExternalLink className="size-3.5" /> Download
              </Button>
            </div>
          </article>
        ))}
      </div>
    </div>
  );
}
