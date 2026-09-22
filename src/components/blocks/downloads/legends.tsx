import { ChevronDown, Crown, Download, PackageOpen } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { ActionButton } from "@/components/ui/action-button";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { useBackend, useBackendMutation } from "@/hooks/use-backend";
import { formatBytes } from "@/lib/downloads";

const LEGENDS = [
  {
    description:
      "A huge Forge vanilla-plus adventure pack with quests, dimensions, bosses, and hundreds of mods.",
    projectId: "better-mc-forge-bmc4",
    title: "Better MC [Forge] — BMC4",
  },
  {
    description:
      "A lore-rich Fabric RPG with quests, talent trees, custom gear, bosses, magic, and technology.",
    projectId: "prominence-2-fabric",
    title: "Prominence II: Hasturian Era",
  },
  {
    description:
      "The official Fabric Cobblemon experience with performance, visual, audio, and quality-of-life mods.",
    projectId: "cobblemon-fabric",
    title: "Cobblemon Official Modpack",
  },
  {
    description:
      "A polished Fabric performance and graphics pack with broad OptiFine-feature parity.",
    projectId: "fabulously-optimized",
    title: "Fabulously Optimized",
  },
  {
    description:
      "A Forge progression pack built around Create, 50+ add-ons, thousands of custom recipes, and exploration.",
    projectId: "all-the-create",
    title: "All the Create",
  },
] as const;

export function LegendsDownloads() {
  return (
    <div className="scrollbar-thin min-h-0 flex-1 space-y-3 overflow-y-auto pe-1">
      <div className="rounded-xl border border-primary/20 bg-primary/5 p-4">
        <div className="flex items-center gap-2 font-bold text-sm">
          <Crown className="size-4 text-amber-500" /> Legends
        </div>
        <p className="mt-1 text-muted-foreground text-xs leading-relaxed">
          Installing a Legend downloads its declared Minecraft version and
          Fabric or Forge loader first, then creates a separate named profile in
          your Play version list.
        </p>
      </div>
      {LEGENDS.map((legend) => (
        <LegendCard key={legend.projectId} legend={legend} />
      ))}
    </div>
  );
}

function LegendCard({ legend }: { legend: (typeof LEGENDS)[number] }) {
  const [expanded, setExpanded] = useState(false);
  const versions = useBackend({
    args: { projectId: legend.projectId },
    enabled: expanded,
    name: "get_modpack_versions",
    queryKey: ["legend-versions", legend.projectId],
  });
  const install = useBackendMutation({ name: "install_modpack" });

  return (
    <article className="rounded-xl border border-border/50 bg-background/55 p-4">
      <div className="flex items-center gap-4">
        <div className="flex size-12 shrink-0 items-center justify-center rounded-xl bg-secondary/60">
          <PackageOpen className="size-6 text-primary" />
        </div>
        <div className="min-w-0 flex-1">
          <h2 className="font-bold text-sm">{legend.title}</h2>
          <p className="mt-1 line-clamp-2 text-muted-foreground text-xs">
            {legend.description}
          </p>
        </div>
        <Button
          className="shrink-0 gap-1.5"
          onClick={() => setExpanded((value) => !value)}
          variant="secondary"
        >
          Choose version
          <ChevronDown
            className={`size-4 transition-transform${expanded ? "rotate-180" : ""}`}
          />
        </Button>
      </div>

      {expanded && (
        <div className="mt-4 max-h-64 space-y-2 overflow-y-auto border-border/40 border-t pt-3">
          {versions.isLoading && (
            <div className="flex justify-center py-4">
              <Spinner className="size-5" />
            </div>
          )}
          {versions.error && (
            <p className="py-3 text-center text-destructive text-xs">
              Could not load official Modrinth versions.
            </p>
          )}
          {versions.data?.map((version) => (
            <div
              className="flex items-center gap-3 rounded-lg bg-secondary/25 px-3 py-2.5"
              key={version.id}
            >
              <div className="min-w-0 flex-1">
                <div className="truncate font-semibold text-xs">
                  {version.name}
                </div>
                <div className="mt-0.5 truncate text-[10px] text-muted-foreground">
                  Minecraft {version.gameVersions.slice(0, 4).join(" · ")}
                  {version.size > 0 ? ` · ${formatBytes(version.size)}` : ""}
                </div>
              </div>
              <ActionButton
                action={async () => {
                  await install.mutateAsync({
                    name: legend.title,
                    versionId: version.id,
                  });
                  toast.success(`${legend.title} installation started`, {
                    description:
                      "Minecraft, the loader, and the modpack will be installed as one named profile.",
                  });
                }}
                className="h-8 gap-1.5 px-3 text-xs"
                disabled={install.isPending}
              >
                <Download className="size-3.5" /> Install all
              </ActionButton>
            </div>
          ))}
        </div>
      )}
    </article>
  );
}
