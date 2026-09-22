import { useQueryClient } from "@tanstack/react-query";
import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
} from "react";
import { toast } from "sonner";
import type { ModItem } from "@/components/blocks/mods/mod-item";
import { useBackend, useBackendMutation } from "@/hooks/use-backend";
import type { BackpackCategory, InvokeError, ModInfo } from "@/invokes";

/** Plural section label shown in headers, dialogs and toasts. */
export const CATEGORY_LABELS: Record<BackpackCategory, string> = {
  mods: "Mods",
  resourcePacks: "Resource Packs",
  shaderPacks: "Shader Packs",
};

/** Singular item label ("Import Mod", "Import Resource Pack", …). */
export const CATEGORY_ITEM_LABELS: Record<BackpackCategory, string> = {
  mods: "Mod",
  resourcePacks: "Resource Pack",
  shaderPacks: "Shader Pack",
};

/** Modrinth `project_type` facet for each Backpack category. Kept in
 *  sync with `BackpackCategory::project_type()` on the Rust side. */
export const CATEGORY_PROJECT_TYPES: Record<BackpackCategory, string> = {
  mods: "mod",
  resourcePacks: "resourcepack",
  shaderPacks: "shader",
};

interface BackpackState {
  activeCategory: BackpackCategory;
  installedVersions: string[];
  isBrowserOpen: boolean;
  isImporting: boolean;
  isLoadingVersions: boolean;
  selectedVersion: string;
  versionsError: InvokeError | null;
}

interface BackpackActions {
  closeBrowser: () => void;
  deleteItem: (item: ModItem, category: BackpackCategory) => Promise<void>;
  importItem: (category: BackpackCategory) => Promise<void>;
  invalidateCategory: (category: BackpackCategory) => void;
  openBrowser: () => void;
  openFolder: (category: BackpackCategory) => Promise<void>;
  setActiveCategory: (category: BackpackCategory) => void;
  setSelectedVersion: (version: string) => void;
  toggleItem: (
    item: ModItem,
    toggle: boolean,
    category: BackpackCategory
  ) => Promise<void>;
}

export const BackpackStateContext = createContext<BackpackState | null>(null);
export const BackpackActionsContext = createContext<BackpackActions | null>(
  null
);

export function useBackpackState() {
  const context = useContext(BackpackStateContext);
  if (!context) {
    throw new Error("useBackpackState must be used within BackpackProvider");
  }
  return context;
}

export function useBackpackActions() {
  const context = useContext(BackpackActionsContext);
  if (!context) {
    throw new Error("useBackpackActions must be used within BackpackProvider");
  }
  return context;
}

/** Convert a UI list item back into the backend `ModInfo` payload. */
export function modItemToInfo(item: ModItem): ModInfo {
  return {
    description: item.description,
    enabled: item.enabled,
    modId: item.id,
    name: item.name,
    path: item.fileName,
    version: item.version,
  };
}

/**
 * Per-category item list for the selected instance. Exposed as a hook
 * (not context state) so each category keeps its own React Query cache
 * entry — switching tabs restores instantly and only refetches when the
 * instance or category changes.
 */
export function useBackpackItems(category: BackpackCategory) {
  const { selectedVersion } = useBackpackState();
  return useBackend({
    args: { category, versionId: selectedVersion },
    enabled: selectedVersion !== "",
    name: "get_backpack_items",
    queryKey: ["backpack_items", selectedVersion, category],
  });
}

export function BackpackProvider({ children }: { children: React.ReactNode }) {
  const queryClient = useQueryClient();
  const [localSelectedVersion, setLocalSelectedVersion] = useState<
    string | null
  >(null);
  const [activeCategory, setActiveCategory] =
    useState<BackpackCategory>("mods");
  const [isBrowserOpen, setBrowserOpen] = useState(false);

  // Only installed versions can own backpack items — each one is an
  // isolated instance with its own mods/resourcepacks/shaderpacks
  // folders, so nothing ever mixes between versions.
  const {
    data: installedVersions,
    isLoading: isLoadingVersions,
    error: versionsError,
  } = useBackend({ name: "get_installed_versions" });

  const selectedVersion = localSelectedVersion ?? installedVersions?.[0] ?? "";

  const invalidateCategory = useCallback(
    (category: BackpackCategory) => {
      queryClient.invalidateQueries({
        queryKey: ["backpack_items", selectedVersion, category],
      });
    },
    [queryClient, selectedVersion]
  );

  const { mutateAsync: toggleBackend } = useBackendMutation({
    name: "toggle_backpack_item",
  });
  const { mutateAsync: deleteBackend } = useBackendMutation({
    name: "delete_backpack_item",
  });
  const { mutateAsync: openFolderBackend } = useBackendMutation({
    name: "open_backpack_folder",
  });
  const { mutateAsync: importBackend, isPending: isImporting } =
    useBackendMutation({
      name: "import_backpack_item",
      onSuccess: (_result, variables) => {
        toast.success(`${CATEGORY_ITEM_LABELS[variables.category]} imported`, {
          description: `The file has been copied to this instance's ${CATEGORY_LABELS[variables.category].toLowerCase()} folder.`,
        });
        invalidateCategory(variables.category);
      },
    });

  const handleToggleItem = useCallback(
    async (item: ModItem, toggle: boolean, category: BackpackCategory) => {
      try {
        await toggleBackend({ category, item: modItemToInfo(item), toggle });
        invalidateCategory(category);
      } catch (e) {
        console.error(e);
      }
    },
    [toggleBackend, invalidateCategory]
  );

  const handleDeleteItem = useCallback(
    async (item: ModItem, category: BackpackCategory) => {
      try {
        await deleteBackend({ item: modItemToInfo(item) });
        toast.success(`${CATEGORY_ITEM_LABELS[category]} deleted`, {
          description: item.name,
        });
        invalidateCategory(category);
      } catch (e) {
        console.error(e);
      }
    },
    [deleteBackend, invalidateCategory]
  );

  const handleImportItem = useCallback(
    async (category: BackpackCategory) => {
      try {
        await importBackend({ category, versionId: selectedVersion });
      } catch {
        // handled globally by useBackendMutation onError toast
      }
    },
    [importBackend, selectedVersion]
  );

  const handleOpenFolder = useCallback(
    async (category: BackpackCategory) => {
      try {
        await openFolderBackend({ category, versionId: selectedVersion });
      } catch {
        // handled globally by useBackendMutation onError toast
      }
    },
    [openFolderBackend, selectedVersion]
  );

  // Opens the integrated Modrinth browser for the active category.
  // Search results are filtered to the selected instance's game version
  // (and mod loader, for mods), and downloads go straight into that
  // instance's own folder for the category.
  const handleOpenBrowser = useCallback(() => {
    if (selectedVersion === "") {
      toast.info("No instance selected", {
        description: "Install a version first, then manage its content.",
      });
      return;
    }
    setBrowserOpen(true);
  }, [selectedVersion]);

  const handleCloseBrowser = useCallback(() => {
    setBrowserOpen(false);
  }, []);

  const actions = useMemo(
    () => ({
      closeBrowser: handleCloseBrowser,
      deleteItem: handleDeleteItem,
      importItem: handleImportItem,
      invalidateCategory,
      openBrowser: handleOpenBrowser,
      openFolder: handleOpenFolder,
      setActiveCategory,
      setSelectedVersion: setLocalSelectedVersion,
      toggleItem: handleToggleItem,
    }),
    [
      handleCloseBrowser,
      handleDeleteItem,
      handleImportItem,
      handleOpenBrowser,
      handleOpenFolder,
      handleToggleItem,
      invalidateCategory,
    ]
  );

  const state = useMemo(
    () => ({
      activeCategory,
      installedVersions: installedVersions ?? [],
      isBrowserOpen,
      isImporting,
      isLoadingVersions,
      selectedVersion,
      versionsError,
    }),
    [
      activeCategory,
      installedVersions,
      isBrowserOpen,
      isImporting,
      isLoadingVersions,
      selectedVersion,
      versionsError,
    ]
  );

  return (
    <BackpackActionsContext.Provider value={actions}>
      <BackpackStateContext.Provider value={state}>
        {children}
      </BackpackStateContext.Provider>
    </BackpackActionsContext.Provider>
  );
}
