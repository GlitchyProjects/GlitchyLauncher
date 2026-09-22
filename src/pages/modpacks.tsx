import {
  ChevronDown,
  Download,
  PackageOpen,
  Search,
  Upload,
} from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { ActionButton } from "@/components/ui/action-button";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { useBackend, useBackendMutation } from "@/hooks/use-backend";
import type { ModpackSearchHit } from "@/invokes";
import { formatBytes } from "@/lib/downloads";

const PAGE_SIZE = 20;
const DEBOUNCE_MS = 350;

export default function Modpacks() {
  const [searchInput, setSearchInput] = useState("");
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);
  const importPack = useBackendMutation({ name: "import_modpack" });

  useEffect(() => {
    const timer = setTimeout(() => {
      setQuery(searchInput.trim());
      setOffset(0);
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [searchInput]);

  const results = useBackend({
    args: { limit: PAGE_SIZE, offset, query },
    name: "search_modpacks",
    queryKey: ["modpacks", query, offset],
  });
  const hits = results.data?.hits ?? [];
  const total = results.data?.totalHits ?? 0;

  return (
    <div className="flex h-full flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="font-black text-2xl tracking-tight">Modpacks</h1>
          <p className="mt-1 text-muted-foreground text-xs">
            Browse full packs from Modrinth or import a local .mrpack archive.
          </p>
        </div>
        <ActionButton
          action={async () => {
            const sessionId = await importPack.mutateAsync();
            if (sessionId) {
              toast.success("Modpack import started", {
                description: "Progress is available in Downloads.",
              });
            }
          }}
          className="gap-2"
          variant="secondary"
        >
          <Upload className="size-4" />
          Import .mrpack
        </ActionButton>
      </div>

      <div className="flex flex-wrap items-center gap-3">
        <div className="relative min-w-56 flex-1">
          <Search className="pointer-events-none absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            className="ps-9"
            onChange={(event) => setSearchInput(event.target.value)}
            placeholder="Search Modrinth modpacks…"
            value={searchInput}
          />
        </div>
      </div>

      <div className="scrollbar-thin min-h-0 flex-1 space-y-3 overflow-y-auto pe-1">
        {results.isLoading && (
          <div className="flex h-full items-center justify-center">
            <Spinner className="size-7" />
          </div>
        )}
        {results.error && (
          <Empty className="h-full">
            <EmptyTitle>Search failed</EmptyTitle>
            <EmptyDescription>
              Could not reach Modrinth. Check your connection and try again.
            </EmptyDescription>
          </Empty>
        )}
        {!(results.isLoading || results.error) && hits.length === 0 && (
          <Empty className="h-full">
            <PackageOpen className="size-8 text-muted-foreground" />
            <EmptyTitle>No modpacks found</EmptyTitle>
            <EmptyDescription>Try a different name.</EmptyDescription>
          </Empty>
        )}
        {hits.map((hit) => (
          <ModpackCard hit={hit} key={hit.projectId} />
        ))}
      </div>

      <div className="flex items-center justify-between border-border/50 border-t pt-3 text-muted-foreground text-xs">
        <span>{total.toLocaleString()} packs</span>
        <div className="flex gap-2">
          <Button
            disabled={offset === 0}
            onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
            variant="outline"
          >
            Previous
          </Button>
          <Button
            disabled={offset + PAGE_SIZE >= total}
            onClick={() => setOffset(offset + PAGE_SIZE)}
            variant="outline"
          >
            Next
          </Button>
        </div>
      </div>
    </div>
  );
}

export function ModpackCard({ hit }: { hit: ModpackSearchHit }) {
  const [expanded, setExpanded] = useState(false);
  const versions = useBackend({
    args: { projectId: hit.projectId },
    enabled: expanded,
    name: "get_modpack_versions",
    queryKey: ["modpack-versions", hit.projectId],
  });
  const install = useBackendMutation({ name: "install_modpack" });

  return (
    <div className="rounded-xl border border-border/50 bg-background/55 p-4">
      <div className="flex gap-4">
        <div className="flex size-16 shrink-0 items-center justify-center overflow-hidden rounded-xl bg-secondary/60">
          {hit.iconUrl ? (
            <img
              alt=""
              className="size-full object-cover"
              height={64}
              src={hit.iconUrl}
              width={64}
            />
          ) : (
            <PackageOpen className="size-7 text-muted-foreground" />
          )}
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-x-2">
            <h2 className="truncate font-bold text-sm">{hit.title}</h2>
            <span className="text-[11px] text-muted-foreground">
              by {hit.author || "Unknown"}
            </span>
          </div>
          <p className="mt-1 line-clamp-2 text-muted-foreground text-xs leading-relaxed">
            {hit.description}
          </p>
          <div className="mt-2 text-[11px] text-muted-foreground">
            {hit.downloads.toLocaleString()} downloads
          </div>
        </div>
        <Button
          className="shrink-0 gap-1.5"
          onClick={() => setExpanded((value) => !value)}
          variant="secondary"
        >
          Versions{" "}
          <ChevronDown
            className={`size-4 transition-transform ${expanded ? "rotate-180" : ""}`}
          />
        </Button>
      </div>

      {expanded && (
        <div className="mt-4 space-y-2 border-border/40 border-t pt-3">
          {versions.isLoading && (
            <div className="flex justify-center py-4">
              <Spinner className="size-5" />
            </div>
          )}
          {versions.error && (
            <p className="py-3 text-center text-destructive text-xs">
              Could not load versions.
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
                  {version.gameVersions.slice(0, 4).join(" · ")}
                  {version.size > 0 ? ` · ${formatBytes(version.size)}` : ""}
                </div>
              </div>
              <ActionButton
                action={async () => {
                  await install.mutateAsync({
                    name: hit.title,
                    versionId: version.id,
                  });
                  toast.success(`${hit.title} installation started`, {
                    description:
                      "You can follow its live progress in Downloads.",
                  });
                }}
                className="h-8 gap-1.5 px-3 text-xs"
                disabled={install.isPending}
              >
                <Download className="size-3.5" /> Install
              </ActionButton>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
