import { Alert01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Download, FolderOpen, PackagePlus } from "lucide-react";
import { ActionButton } from "@/components/ui/action-button";
import { LoadingSwap } from "@/components/ui/animated/swapper";
import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
} from "@/components/ui/combobox";
import {
  CATEGORY_ITEM_LABELS,
  CATEGORY_LABELS,
  useBackpackActions,
  useBackpackState,
} from "@/context/backpack-context";
import { useBackend } from "@/hooks/use-backend";
import { errorText } from "@/messages";

/**
 * Instance selector + per-category actions. Every action targets the
 * currently active Backpack tab's category for the selected instance —
 * imports, folder opening and Modrinth downloads all stay inside that
 * instance's own folder.
 */
export function ModsHeader() {
  const {
    activeCategory,
    installedVersions,
    isImporting,
    isLoadingVersions,
    selectedVersion,
    versionsError,
  } = useBackpackState();
  const { importItem, openBrowser, openFolder, setSelectedVersion } =
    useBackpackActions();

  const { data: instances } = useBackend({
    initialData: [],
    initialDataUpdatedAt: 0,
    name: "list_instances",
    queryKey: ["instances"],
  });

  const currentInstance = instances?.find((i) => i.id === selectedVersion);
  const categoryLabel = CATEGORY_LABELS[activeCategory];
  const itemLabel = CATEGORY_ITEM_LABELS[activeCategory];

  return (
    <div className="flex items-center justify-between gap-4 rounded-2xl border border-border/40 bg-secondary/30 p-3 shadow-sm backdrop-blur-md">
      <div className="w-64">
        <LoadingSwap isLoading={isLoadingVersions}>
          {versionsError ? (
            <div className="flex h-10 items-center gap-2 rounded-xl border border-destructive/20 bg-destructive/5 px-3 text-destructive">
              <HugeiconsIcon
                className="shrink-0"
                icon={Alert01Icon}
                size={16}
              />
              <span className="truncate font-medium text-xs">
                {errorText(versionsError.code).title}
              </span>
            </div>
          ) : (
            <Combobox
              autoHighlight
              items={installedVersions}
              onValueChange={(val) => setSelectedVersion(val ?? "")}
              value={selectedVersion}
            >
              <ComboboxInput
                placeholder="Select Game Version"
                value={currentInstance ? currentInstance.displayName : selectedVersion}
              />
              <ComboboxContent>
                <ComboboxEmpty>No installed versions found.</ComboboxEmpty>
                <ComboboxList>
                  {(ver) => {
                    const inst = instances?.find((i) => i.id === ver);
                    return (
                      <ComboboxItem key={ver} value={ver}>
                        <div className="flex flex-col min-w-0 pr-2">
                          <span className="font-semibold text-foreground truncate text-xs">
                            {inst?.displayName ?? ver}
                          </span>
                          {inst && (
                            <span className="text-[10px] text-muted-foreground/80 truncate">
                              {inst.gameVersion} · {inst.loader.toUpperCase()}
                            </span>
                          )}
                        </div>
                      </ComboboxItem>
                    );
                  }}
                </ComboboxList>
              </ComboboxContent>
            </Combobox>
          )}
        </LoadingSwap>
      </div>

      <div className="flex items-center space-x-2">
        <button
          className="flex items-center gap-1.5 rounded-xl border border-border/50 bg-background/60 px-3 py-2 font-medium text-foreground text-xs shadow-sm transition-all hover:bg-secondary"
          onClick={() => openFolder(activeCategory)}
          title={`Open ${categoryLabel} Folder`}
          type="button"
        >
          <FolderOpen className="h-4 w-4 text-muted-foreground" />
          <span>Folder</span>
        </button>

        <button
          className={`flex items-center gap-1.5 rounded-xl border border-border/50 px-3 py-2 font-medium text-xs shadow-sm transition-all ${
            isImporting
              ? "cursor-not-allowed bg-secondary text-muted-foreground"
              : "bg-background/60 text-foreground hover:bg-secondary"
          }`}
          disabled={isImporting}
          onClick={() => importItem(activeCategory)}
          title={`Import ${itemLabel} file`}
          type="button"
        >
          <PackagePlus
            className={`h-4 w-4 text-muted-foreground ${isImporting ? "animate-pulse" : ""}`}
          />
          <span>{isImporting ? "Importing..." : `Import ${itemLabel}`}</span>
        </button>

        <ActionButton
          action={async () => openBrowser()}
          className="flex items-center gap-1.5 px-3.5 py-2 text-xs"
        >
          <Download className="h-4 w-4" />
          <span>Get {categoryLabel}</span>
        </ActionButton>
      </div>
    </div>
  );
}
