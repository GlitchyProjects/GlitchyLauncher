import { ChevronDown, Download, Search } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { ActionButton } from "@/components/ui/action-button";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Empty, EmptyDescription, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import {
  CATEGORY_ITEM_LABELS,
  CATEGORY_LABELS,
  CATEGORY_PROJECT_TYPES,
  useBackpackActions,
  useBackpackState,
} from "@/context/backpack-context";
import { useBackend, useBackendMutation } from "@/hooks/use-backend";
import type {
  BackpackCategory,
  ModrinthSearchHit,
  ModrinthVersion,
} from "@/invokes";

const PAGE_SIZE = 20;
const SEARCH_DEBOUNCE_MS = 400;

function formatDownloads(n: number | null): string {
  if (n === null) {
    return "0";
  }
  if (n >= 1_000_000) {
    return `${(n / 1_000_000).toFixed(1)}M`;
  }
  if (n >= 1000) {
    return `${(n / 1000).toFixed(1)}K`;
  }
  return String(n);
}

function formatSize(bytes: number | null): string | null {
  if (bytes === null) {
    return null;
  }
  if (bytes >= 1_000_000) {
    return `${(bytes / 1_000_000).toFixed(1)} MB`;
  }
  if (bytes >= 1000) {
    return `${(bytes / 1000).toFixed(0)} KB`;
  }
  return `${bytes} B`;
}

function formatDate(iso: string | null): string {
  if (!iso) {
    return "";
  }
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? "" : date.toLocaleDateString();
}

/**
 * Modrinth browser for the active Backpack category. Search results are
 * filtered by the category's project type ("mod" / "resourcepack" /
 * "shader") and, for mods, the selected instance's game version + mod
 * loader. Downloads land in that instance's own folder for the category
 * — content never mixes between versions.
 */
export function ModrinthBrowser() {
  const { activeCategory, isBrowserOpen, selectedVersion } = useBackpackState();
  const { closeBrowser } = useBackpackActions();

  const [searchInput, setSearchInput] = useState("");
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);

  const categoryLabel = CATEGORY_LABELS[activeCategory];
  const projectType = CATEGORY_PROJECT_TYPES[activeCategory];

  // Reset paging and the search box whenever the user switches tab so a
  // stale shader query doesn't follow them into the resource packs tab.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the reset is keyed on the tab change itself
  useEffect(() => {
    setSearchInput("");
    setQuery("");
    setOffset(0);
  }, [activeCategory]);

  // Debounce raw input into the actual search query and reset paging.
  useEffect(() => {
    const timer = setTimeout(() => {
      setQuery(searchInput);
      setOffset(0);
    }, SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [searchInput]);

  const { data: instanceInfo } = useBackend({
    args: { instanceId: selectedVersion },
    enabled: isBrowserOpen && selectedVersion !== "",
    name: "get_instance_info",
    queryKey: ["instance_info", selectedVersion],
  });

  const gameVersion = instanceInfo?.gameVersion ?? "";
  // Only mods are loader-specific on Modrinth — resource and shader
  // packs are not tagged "fabric"/"forge", so the loader facet must stay
  // empty for them or the search would return nothing.
  const loader = activeCategory === "mods" ? (instanceInfo?.loader ?? "") : "";

  const {
    data: results,
    error,
    isLoading,
  } = useBackend({
    args: { gameVersion, limit: PAGE_SIZE, loader, offset, projectType, query },
    enabled: isBrowserOpen,
    name: "modrinth_search",
    queryKey: [
      "modrinth_search",
      query,
      gameVersion,
      loader,
      projectType,
      offset,
    ],
  });

  const hits = results?.hits ?? [];
  const totalHits = results?.totalHits ?? 0;
  const canGoNext = offset + PAGE_SIZE < totalHits;
  const compatibilityLabel =
    gameVersion === ""
      ? "Loading instance info…"
      : `Compatible with ${gameVersion}${loader ? ` · ${loader}` : ""} — ${categoryLabel.toLowerCase()} are installed to this instance only.`;

  const renderResults = () => {
    if (isLoading) {
      return (
        <div className="flex h-full items-center justify-center">
          <Spinner className="h-6 w-6" />
        </div>
      );
    }
    if (error) {
      return (
        <div className="flex h-full items-center justify-center">
          <Empty>
            <EmptyTitle>Search failed</EmptyTitle>
            <EmptyDescription>
              Could not reach Modrinth. Check your connection and try again.
            </EmptyDescription>
          </Empty>
        </div>
      );
    }
    if (hits.length === 0) {
      return (
        <div className="flex h-full items-center justify-center">
          <Empty>
            <EmptyTitle>No {categoryLabel.toLowerCase()} found</EmptyTitle>
            <EmptyDescription>
              {query
                ? `No results for "${query}". Try a different search.`
                : `Type something in the search box to find ${categoryLabel.toLowerCase()}.`}
            </EmptyDescription>
          </Empty>
        </div>
      );
    }
    return hits.map((hit) => (
      <ModrinthHit
        activeCategory={activeCategory}
        gameVersion={gameVersion}
        hit={hit}
        instanceId={selectedVersion}
        key={hit.projectId}
        loader={loader}
      />
    ));
  };

  return (
    <Dialog
      onOpenChange={(open) => {
        if (!open) {
          closeBrowser();
        }
      }}
      open={isBrowserOpen}
    >
      <DialogContent className="flex h-[80vh] max-w-2xl flex-col gap-4 sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Browse {categoryLabel} on Modrinth</DialogTitle>
          <DialogDescription>{compatibilityLabel}</DialogDescription>
        </DialogHeader>

        <div className="relative">
          <Search className="pointer-events-none absolute start-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            className="ps-9"
            onChange={(e) => setSearchInput(e.target.value)}
            placeholder={`Search ${categoryLabel.toLowerCase()} by name…`}
            value={searchInput}
          />
        </div>

        <div className="scrollbar-thin scrollbar-thumb-muted-foreground/20 flex-1 space-y-2.5 overflow-y-auto pe-1">
          {renderResults()}
        </div>

        <div className="flex items-center justify-between border-border/40 border-t pt-3">
          <Button
            disabled={offset === 0}
            onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
            size="sm"
            variant="outline"
          >
            Previous
          </Button>
          <span className="font-medium text-muted-foreground text-xs">
            {totalHits} {categoryLabel.toLowerCase()} found
          </span>
          <Button
            disabled={!canGoNext}
            onClick={() => setOffset(offset + PAGE_SIZE)}
            size="sm"
            variant="outline"
          >
            Next
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function ModrinthHit({
  activeCategory,
  gameVersion,
  hit,
  instanceId,
  loader,
}: {
  activeCategory: BackpackCategory;
  gameVersion: string;
  hit: ModrinthSearchHit;
  instanceId: string;
  loader: string;
}) {
  const { invalidateCategory } = useBackpackActions();
  const [showVersions, setShowVersions] = useState(false);
  const [downloadedVersionIds, setDownloadedVersionIds] = useState<string[]>(
    []
  );

  const itemLabel = CATEGORY_ITEM_LABELS[activeCategory];

  // Versions are only fetched when the user expands this hit — avoids
  // one API call per search result.
  const { data: versions, isLoading: isLoadingVersions } = useBackend({
    args: { gameVersion, loader, projectId: hit.projectId },
    enabled: showVersions,
    name: "modrinth_get_project_versions",
    queryKey: ["modrinth_versions", hit.projectId, gameVersion, loader],
  });

  const { mutateAsync: downloadItem, isPending: isDownloading } =
    useBackendMutation({
      name: "modrinth_download_item",
      onSuccess: (fileName) => {
        toast.success(`${itemLabel} downloaded`, {
          description: `${fileName} was added to this instance's ${CATEGORY_LABELS[activeCategory].toLowerCase()} folder.`,
        });
      },
    });

  const handleDownload = async (version: ModrinthVersion) => {
    await downloadItem({
      category: activeCategory,
      instanceId,
      versionId: version.id,
    });
    setDownloadedVersionIds((prev) => [...prev, version.id]);
    invalidateCategory(activeCategory);
  };

  const renderVersions = () => {
    if (isLoadingVersions) {
      return (
        <div className="flex justify-center py-2">
          <Spinner className="h-5 w-5" />
        </div>
      );
    }
    if ((versions ?? []).length === 0) {
      return (
        <p className="py-1 text-center text-muted-foreground text-xs">
          No compatible versions for {gameVersion}
          {loader ? ` (${loader})` : ""}.
        </p>
      );
    }
    return (versions ?? []).slice(0, 10).map((version) => {
      const isDownloaded = downloadedVersionIds.includes(version.id);
      const size = formatSize(version.primaryFile.size);
      return (
        <div
          className="flex items-center justify-between rounded-lg bg-secondary/40 px-3 py-2"
          key={version.id}
        >
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <span className="truncate font-mono text-xs">
                {version.versionNumber ?? version.id}
              </span>
              <span className="shrink-0 text-[10px] text-muted-foreground">
                {formatDate(version.datePublished)}
              </span>
            </div>
            <span className="text-[10px] text-muted-foreground">
              {version.primaryFile.filename}
              {size ? ` · ${size}` : ""}
            </span>
          </div>

          <ActionButton
            action={async () => {
              await handleDownload(version);
            }}
            className="h-8 gap-1.5 px-3 text-xs"
            disabled={isDownloading}
            size="sm"
          >
            {isDownloading ? (
              <Spinner className="h-3.5 w-3.5" />
            ) : (
              <Download className="h-3.5 w-3.5" />
            )}
            {isDownloaded ? "Again" : "Download"}
          </ActionButton>
        </div>
      );
    });
  };

  return (
    <div className="rounded-xl border border-border/50 bg-background/60 p-3">
      <div className="flex items-center gap-3.5">
        <div className="flex h-11 w-11 shrink-0 items-center justify-center overflow-hidden rounded-lg border border-border/40 bg-secondary">
          {hit.iconUrl ? (
            <img
              alt={hit.title ?? hit.projectId}
              className="h-full w-full object-cover"
              height={44}
              loading="lazy"
              src={hit.iconUrl}
              width={44}
            />
          ) : (
            <span className="font-bold text-muted-foreground text-xs uppercase">
              {(hit.title ?? hit.projectId).slice(0, 2)}
            </span>
          )}
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <h4 className="truncate font-semibold text-foreground text-sm">
              {hit.title ?? hit.slug ?? hit.projectId}
            </h4>
            <span className="shrink-0 rounded bg-secondary px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
              ↓ {formatDownloads(hit.downloads)}
            </span>
          </div>
          <p className="mt-0.5 truncate text-muted-foreground text-xs">
            {hit.description || `by ${hit.author ?? "unknown"}`}
          </p>
        </div>

        <Button
          onClick={() => setShowVersions((v) => !v)}
          size="sm"
          variant="outline"
        >
          Versions
          <ChevronDown
            className={`h-4 w-4 transition-transform ${showVersions ? "rotate-180" : ""}`}
          />
        </Button>
      </div>

      {showVersions && (
        <div className="mt-3 space-y-1.5 border-border/40 border-t pt-3">
          {renderVersions()}
        </div>
      )}
    </div>
  );
}
