import { Alert01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Power, Trash2 } from "lucide-react";
import { useMemo } from "react";
import type { ModItem } from "@/components/blocks/mods/mod-item";
import { ActionButton } from "@/components/ui/action-button";
import { LoadingSwap } from "@/components/ui/animated/swapper";
import {
  Empty,
  EmptyDescription,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import {
  CATEGORY_ITEM_LABELS,
  CATEGORY_LABELS,
  useBackpackActions,
  useBackpackItems,
  useBackpackState,
} from "@/context/backpack-context";
import type { BackpackCategory, ModInfo } from "@/invokes";
import { errorText } from "@/messages";

/**
 * Item list for one Backpack category of the selected instance. The
 * query is keyed by (instance, category) so every tab shows exactly its
 * own folder contents — items never mix between versions or categories.
 */
export function ModsList({ category }: { category: BackpackCategory }) {
  const { selectedVersion } = useBackpackState();
  const { deleteItem, toggleItem } = useBackpackActions();
  const { data: fetchedItems, error, isLoading } = useBackpackItems(category);

  const itemsList = useMemo<ModItem[]>(
    () =>
      (fetchedItems ?? []).map(
        (info: ModInfo): ModItem => ({
          description: info.description,
          enabled: info.enabled,
          fileName: info.path,
          id: info.modId,
          name: info.name,
          version: info.version,
        })
      ),
    [fetchedItems]
  );

  const itemLabel = CATEGORY_ITEM_LABELS[category];
  const categoryLabel = CATEGORY_LABELS[category];

  const renderItemsContent = () => {
    if (error) {
      return (
        <div className="flex flex-1 items-center justify-center">
          <Empty>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={Alert01Icon} size={24} />
            </EmptyMedia>
            <EmptyTitle>{errorText(error.code).title}</EmptyTitle>
            <EmptyDescription>
              {errorText(error.code).description}
            </EmptyDescription>
          </Empty>
        </div>
      );
    }

    if (itemsList.length === 0) {
      return (
        <div className="flex flex-1 items-center justify-center">
          <Empty>
            <EmptyTitle>No {categoryLabel.toLowerCase()} installed</EmptyTitle>
            <EmptyDescription>
              No {categoryLabel.toLowerCase()} installed for{" "}
              {selectedVersion || "this version"}. Click “Get {categoryLabel}”
              or “Import {itemLabel}” to add some!
            </EmptyDescription>
          </Empty>
        </div>
      );
    }

    return (
      <div className="scrollbar-thin scrollbar-thumb-muted-foreground/20 space-y-2.5 pr-1">
        {itemsList.map((item) => (
          <div
            className={`flex items-center justify-between rounded-xl border p-3.5 transition-all duration-200 ${
              item.enabled
                ? "border-border/60 bg-background/80 shadow-sm"
                : "border-border/30 bg-background/30 opacity-60"
            }`}
            key={item.id || item.fileName}
          >
            <div className="flex min-w-0 items-center space-x-3.5 pr-4">
              <div className="flex h-10 w-10 shrink-0 items-center justify-center overflow-hidden rounded-lg border border-border/40 bg-secondary">
                {item.iconUrl ? (
                  <img
                    alt={item.name}
                    className="h-full w-full object-cover"
                    height={40}
                    src={item.iconUrl}
                    width={40}
                  />
                ) : (
                  <span className="font-bold text-muted-foreground text-xs uppercase">
                    {item.name?.slice(0, 2) ?? "??"}
                  </span>
                )}
              </div>

              <div className="min-w-0">
                <div className="flex items-center space-x-2">
                  <h4 className="truncate font-semibold text-foreground text-xs">
                    {item.name}
                  </h4>
                  {item.version && (
                    <span className="rounded bg-secondary px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
                      v{item.version}
                    </span>
                  )}
                </div>
                <p className="mt-0.5 max-w-md truncate text-[11px] text-muted-foreground">
                  {item.description || "No description provided."}
                </p>
              </div>
            </div>

            <div className="flex shrink-0 items-center space-x-2">
              <ActionButton
                action={() => toggleItem(item, !item.enabled, category)}
                className={`h-8 w-8 rounded-lg border-none p-2 transition-all ${
                  item.enabled
                    ? "bg-emerald-500/10 text-emerald-500 hover:bg-emerald-500/20"
                    : "bg-muted text-muted-foreground hover:bg-secondary"
                }`}
                size="icon"
                title={
                  item.enabled ? `Disable ${itemLabel}` : `Enable ${itemLabel}`
                }
                variant="outline"
              >
                <Power className="h-4 w-4" />
              </ActionButton>

              <ActionButton
                action={() => deleteItem(item, category)}
                className="h-8 w-8 rounded-lg border-none p-2 text-muted-foreground transition-all hover:bg-destructive/10 hover:text-destructive"
                size="icon"
                title={`Delete ${itemLabel}`}
                variant="outline"
              >
                <Trash2 className="h-4 w-4" />
              </ActionButton>
            </div>
          </div>
        ))}
      </div>
    );
  };

  return (
    <LoadingSwap className="flex h-full flex-col" isLoading={isLoading}>
      {renderItemsContent()}
    </LoadingSwap>
  );
}
