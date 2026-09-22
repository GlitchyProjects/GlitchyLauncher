import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  ArchiveRestore,
  Boxes,
  Copy,
  DatabaseBackup,
  FolderOpen,
  HardDrive,
  PackageOpen,
  RotateCcw,
  Save,
  Trash2,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { ActionButton } from "@/components/ui/action-button";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useBackend, useBackendMutation } from "@/hooks/use-backend";
import type { BackpackCategory, InstanceSummary } from "@/invokes";
import { cn } from "@/lib/utils";

const formatBytes = (bytes: number): string => {
  if (bytes <= 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1
  );
  return `${(bytes / 1024 ** index).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
};

const formatDate = (timestamp: number): string =>
  timestamp > 0 ? new Date(timestamp).toLocaleString() : "Unknown";

export default function InstanceManager() {
  const instancesQuery = useBackend({
    initialData: [],
    name: "list_instances",
    queryKey: ["instances"],
  });
  const instances = instancesQuery.data ?? [];
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selected = useMemo(
    () =>
      instances.find((instance) => instance.id === selectedId) ??
      instances[0] ??
      null,
    [instances, selectedId]
  );

  useEffect(() => {
    if (selected && selectedId !== selected.id) {
      setSelectedId(selected.id);
    }
  }, [selected, selectedId]);

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <header className="flex items-center justify-between gap-4">
        <div>
          <h1 className="flex items-center gap-2 font-black text-2xl tracking-tight">
            <Boxes className="size-6 text-primary" /> Instance Manager
          </h1>
          <p className="mt-1 text-muted-foreground text-xs">
            Isolated paths, RAM, content, worlds, recovery and backups.
          </p>
        </div>
        <Button onClick={() => instancesQuery.refetch()} variant="outline">
          <RotateCcw className="size-4" /> Refresh
        </Button>
      </header>

      <div className="grid min-h-0 flex-1 gap-4 lg:grid-cols-[280px_minmax(0,1fr)]">
        <aside className="scrollbar-thin min-h-0 space-y-2 overflow-y-auto rounded-2xl border border-border/60 bg-secondary/15 p-3">
          {instances.map((instance) => (
            <button
              className={cn(
                "w-full rounded-xl border p-3 text-start transition-colors",
                selected?.id === instance.id
                  ? "border-primary/50 bg-primary/10"
                  : "border-transparent bg-background/45 hover:border-border hover:bg-background/70"
              )}
              key={instance.id}
              onClick={() => setSelectedId(instance.id)}
              type="button"
            >
              <span className="block truncate font-semibold text-sm">
                {instance.displayName}
              </span>
              <span className="mt-1 block truncate text-[11px] text-muted-foreground">
                {instance.gameVersion} · {instance.loader} ·{" "}
                {formatBytes(instance.sizeBytes)}
              </span>
            </button>
          ))}
          {!instancesQuery.isLoading && instances.length === 0 && (
            <div className="p-6 text-center text-muted-foreground text-xs">
              No installed instances. Install Minecraft from Downloads first.
            </div>
          )}
        </aside>

        <main className="min-h-0 overflow-hidden rounded-2xl border border-border/60 bg-background/50">
          {selected ? (
            <InstanceDetail
              instance={selected}
              onDeleted={() => {
                setSelectedId(null);
                instancesQuery.refetch();
              }}
              refresh={() => instancesQuery.refetch()}
            />
          ) : (
            <div className="flex h-full items-center justify-center text-muted-foreground text-sm">
              Select an instance to manage it.
            </div>
          )}
        </main>
      </div>
    </div>
  );
}

function InstanceDetail({
  instance,
  onDeleted,
  refresh,
}: {
  instance: InstanceSummary;
  onDeleted: () => void;
  refresh: () => Promise<unknown>;
}) {
  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="border-border/60 border-b p-4">
        <h2 className="truncate font-bold text-lg">{instance.displayName}</h2>
        <p className="truncate text-muted-foreground text-xs">
          {instance.path}
        </p>
      </div>
      <Tabs className="min-h-0 flex-1 gap-0" defaultValue="general">
        <div className="w-full shrink-0 overflow-x-auto border-border/60 border-b p-2">
          <TabsList className="w-max min-w-full flex-nowrap bg-transparent p-0">
            {[
              ["general", "General"],
              ["content", "Modpack & content"],
              ["worlds", "Worlds"],
              ["backups", "Backups"],
            ].map(([value, label]) => (
              <TabsTrigger
                className="min-w-max whitespace-nowrap"
                key={value}
                value={value}
              >
                {label}
              </TabsTrigger>
            ))}
          </TabsList>
        </div>
        <TabsContent
          className="scrollbar-thin min-h-0 overflow-y-auto p-5"
          value="general"
        >
          <GeneralTab
            instance={instance}
            onDeleted={onDeleted}
            refresh={refresh}
          />
        </TabsContent>
        <TabsContent
          className="scrollbar-thin min-h-0 overflow-y-auto p-5"
          value="content"
        >
          <ContentTab instance={instance} />
        </TabsContent>
        <TabsContent
          className="scrollbar-thin min-h-0 overflow-y-auto p-5"
          value="worlds"
        >
          <WorldsTab instanceId={instance.id} refreshInstances={refresh} />
        </TabsContent>
        <TabsContent
          className="scrollbar-thin min-h-0 overflow-y-auto p-5"
          value="backups"
        >
          <BackupsTab instanceId={instance.id} refreshInstances={refresh} />
        </TabsContent>
      </Tabs>
    </div>
  );
}

function GeneralTab({
  instance,
  onDeleted,
  refresh,
}: {
  instance: InstanceSummary;
  onDeleted: () => void;
  refresh: () => Promise<unknown>;
}) {
  const [displayName, setDisplayName] = useState(instance.displayName);
  const [ramMin, setRamMin] = useState(instance.ramMinMb?.toString() ?? "");
  const [ramMax, setRamMax] = useState(instance.ramMaxMb?.toString() ?? "");
  const [cloneId, setCloneId] = useState("");
  const update = useBackendMutation({ name: "update_instance_settings" });
  const setPath = useBackendMutation({ name: "set_instance_path" });
  const resetPath = useBackendMutation({ name: "reset_instance_path" });
  const clone = useBackendMutation({ name: "clone_instance" });
  const reinstall = useBackendMutation({ name: "reinstall_instance" });
  const remove = useBackendMutation({ name: "delete_instance" });
  const openFolder = useBackendMutation({ name: "open_instance_folder" });

  useEffect(() => {
    setDisplayName(instance.displayName);
    setRamMin(instance.ramMinMb?.toString() ?? "");
    setRamMax(instance.ramMaxMb?.toString() ?? "");
    setCloneId("");
  }, [instance]);

  return (
    <div className="space-y-4">
      <section className="grid gap-4 rounded-xl border border-border/50 bg-secondary/15 p-4 md:grid-cols-2">
        <label className="space-y-2 text-xs" htmlFor="instance-display-name">
          <span className="font-medium">Display name</span>
          <Input
            id="instance-display-name"
            onChange={(event) => setDisplayName(event.target.value)}
            value={displayName}
          />
        </label>
        <div className="grid grid-cols-2 gap-2">
          <label className="space-y-2 text-xs" htmlFor="instance-ram-min">
            <span className="font-medium">Minimum RAM (MB)</span>
            <Input
              id="instance-ram-min"
              min={512}
              onChange={(event) => setRamMin(event.target.value)}
              placeholder="Global"
              type="number"
              value={ramMin}
            />
          </label>
          <label className="space-y-2 text-xs" htmlFor="instance-ram-max">
            <span className="font-medium">Maximum RAM (MB)</span>
            <Input
              id="instance-ram-max"
              min={512}
              onChange={(event) => setRamMax(event.target.value)}
              placeholder="Global"
              type="number"
              value={ramMax}
            />
          </label>
        </div>
        <ActionButton
          action={async () => {
            await update.mutateAsync({
              displayName,
              instanceId: instance.id,
              ramMaxMb: ramMax ? Number(ramMax) : null,
              ramMinMb: ramMin ? Number(ramMin) : null,
            });
            await refresh();
            toast.success("Instance settings saved");
          }}
          className="md:col-span-2 md:w-fit"
        >
          <Save className="size-4" /> Save settings
        </ActionButton>
      </section>

      <section className="rounded-xl border border-border/50 bg-secondary/15 p-4">
        <div className="mb-3 flex items-start gap-3">
          <HardDrive className="mt-0.5 size-5 text-primary" />
          <div className="min-w-0">
            <h3 className="font-semibold text-sm">Instance storage path</h3>
            <p className="truncate text-muted-foreground text-xs">
              {instance.path}
            </p>
          </div>
        </div>
        <div className="flex flex-wrap gap-2">
          <ActionButton
            action={async () => {
              const selected = await openDialog({
                directory: true,
                multiple: false,
              });
              if (typeof selected !== "string") {
                return;
              }
              await setPath.mutateAsync({
                instanceId: instance.id,
                moveFiles: true,
                newPath: selected,
              });
              await refresh();
              toast.success("Instance moved to the selected path");
            }}
            variant="outline"
          >
            <FolderOpen className="size-4" /> Choose & move
          </ActionButton>
          <ActionButton
            action={() =>
              openFolder.mutateAsync({
                folder: "root",
                instanceId: instance.id,
              })
            }
            variant="outline"
          >
            <FolderOpen className="size-4" /> Open folder
          </ActionButton>
          <ActionButton
            action={async () => {
              await resetPath.mutateAsync({ instanceId: instance.id });
              await refresh();
              toast.success("Instance returned to the default path");
            }}
            variant="outline"
          >
            <RotateCcw className="size-4" /> Default path
          </ActionButton>
        </div>
      </section>

      <section className="rounded-xl border border-border/50 bg-secondary/15 p-4">
        <h3 className="mb-3 flex items-center gap-2 font-semibold text-sm">
          <Copy className="size-4 text-primary" /> Clone instance
        </h3>
        <div className="flex gap-2">
          <Input
            onChange={(event) => setCloneId(event.target.value)}
            placeholder="New unique instance ID"
            value={cloneId}
          />
          <ActionButton
            action={async () => {
              await clone.mutateAsync({
                instanceId: instance.id,
                newInstanceId: cloneId,
              });
              setCloneId("");
              await refresh();
              toast.success("Instance cloned with worlds, mods and settings");
            }}
            disabled={cloneId.trim().length === 0}
          >
            Clone
          </ActionButton>
        </div>
      </section>

      <section className="flex flex-wrap gap-2 rounded-xl border border-destructive/25 bg-destructive/5 p-4">
        <ActionButton
          action={async () => {
            const report = await reinstall.mutateAsync({
              instanceId: instance.id,
            });
            toast.success(
              report.brokenCount > 0
                ? `Reinstalled ${report.brokenCount} damaged files`
                : "Instance files are already healthy"
            );
          }}
          variant="outline"
        >
          <RotateCcw className="size-4" /> Reinstall / repair
        </ActionButton>
        <ActionButton
          action={async () => {
            await remove.mutateAsync({ instanceId: instance.id });
            toast.success("Instance deleted");
            onDeleted();
          }}
          areYouSureButton="Delete instance"
          areYouSureDescription="Version files, mods, worlds and instance data will be permanently deleted. External backups remain available."
          requireAreYouSure
          variant="destructive"
        >
          <Trash2 className="size-4" /> Delete instance
        </ActionButton>
      </section>
    </div>
  );
}

function ContentTab({ instance }: { instance: InstanceSummary }) {
  return (
    <div className="space-y-4">
      <section className="rounded-xl border border-border/50 bg-secondary/15 p-4">
        <h3 className="flex items-center gap-2 font-semibold text-sm">
          <PackageOpen className="size-4 text-primary" /> Modpack
        </h3>
        <p className="mt-2 text-muted-foreground text-xs">
          {instance.modpackName
            ? `Installed modpack: ${instance.modpackName}`
            : "This instance is not linked to a managed modpack."}
        </p>
        <div className="mt-3 grid gap-2 sm:grid-cols-3">
          <Metric label="Mods" value={instance.modCount.toString()} />
          <Metric label="Worlds" value={instance.worldCount.toString()} />
          <Metric label="Total size" value={formatBytes(instance.sizeBytes)} />
        </div>
      </section>
      <Tabs defaultValue="mods">
        <div className="w-full overflow-x-auto pb-1">
          <TabsList className="w-max min-w-full flex-nowrap">
            <TabsTrigger className="min-w-max whitespace-nowrap" value="mods">
              Mods
            </TabsTrigger>
            <TabsTrigger
              className="min-w-max whitespace-nowrap"
              value="resourcePacks"
            >
              Resource Packs
            </TabsTrigger>
            <TabsTrigger
              className="min-w-max whitespace-nowrap"
              value="shaderPacks"
            >
              Shader Packs
            </TabsTrigger>
          </TabsList>
        </div>
        {(["mods", "resourcePacks", "shaderPacks"] as const).map((category) => (
          <TabsContent key={category} value={category}>
            <InstanceContentList category={category} instanceId={instance.id} />
          </TabsContent>
        ))}
      </Tabs>
    </div>
  );
}

const CONTENT_LABELS: Record<BackpackCategory, string> = {
  mods: "Mods",
  resourcePacks: "Resource Packs",
  shaderPacks: "Shader Packs",
};

const CONTENT_FOLDERS = {
  mods: "mods",
  resourcePacks: "resourcepacks",
  shaderPacks: "shaderpacks",
} as const;

function InstanceContentList({
  category,
  instanceId,
}: {
  category: BackpackCategory;
  instanceId: string;
}) {
  const itemsQuery = useBackend({
    args: { category, versionId: instanceId },
    initialData: [],
    name: "get_backpack_items",
    queryKey: ["instance-content", instanceId, category],
  });
  const toggle = useBackendMutation({ name: "toggle_backpack_item" });
  const remove = useBackendMutation({ name: "delete_backpack_item" });
  const importItem = useBackendMutation({ name: "import_backpack_item" });
  const openFolder = useBackendMutation({ name: "open_instance_folder" });
  const items = itemsQuery.data ?? [];
  const folder = CONTENT_FOLDERS[category];

  return (
    <div className="space-y-2 pt-2">
      <div className="mb-3 flex flex-wrap justify-end gap-2">
        <ActionButton
          action={() => openFolder.mutateAsync({ folder, instanceId })}
          size="sm"
          variant="outline"
        >
          <FolderOpen className="size-4" /> Open folder
        </ActionButton>
        <ActionButton
          action={async () => {
            await importItem.mutateAsync({ category, versionId: instanceId });
            await itemsQuery.refetch();
          }}
          size="sm"
        >
          Import {CONTENT_LABELS[category]}
        </ActionButton>
      </div>
      {items.map((item) => (
        <div
          className="flex items-center gap-3 rounded-xl border border-border/50 bg-background/55 p-3"
          key={item.path}
        >
          <PackageOpen className="size-4 shrink-0 text-primary" />
          <div className="min-w-0 flex-1">
            <p className="truncate font-medium text-sm">{item.name}</p>
            <p className="truncate text-[11px] text-muted-foreground">
              {item.version || "Unknown version"} ·{" "}
              {item.enabled ? "Enabled" : "Disabled"}
            </p>
          </div>
          <ActionButton
            action={async () => {
              await toggle.mutateAsync({
                category,
                item,
                toggle: !item.enabled,
              });
              await itemsQuery.refetch();
            }}
            size="xs"
            variant="outline"
          >
            {item.enabled ? "Disable" : "Enable"}
          </ActionButton>
          <ActionButton
            action={async () => {
              await remove.mutateAsync({ item });
              await itemsQuery.refetch();
            }}
            areYouSureButton="Delete file"
            areYouSureDescription={`“${item.name}” will be deleted from this instance.`}
            requireAreYouSure
            size="icon-sm"
            variant="destructive"
          >
            <Trash2 className="size-4" />
          </ActionButton>
        </div>
      ))}
      {items.length === 0 && (
        <p className="p-6 text-center text-muted-foreground text-xs">
          No {CONTENT_LABELS[category].toLowerCase()} installed in this
          instance.
        </p>
      )}
    </div>
  );
}

function WorldsTab({
  instanceId,
  refreshInstances,
}: {
  instanceId: string;
  refreshInstances: () => Promise<unknown>;
}) {
  const worlds = useBackend({
    args: { instanceId },
    initialData: [],
    name: "list_instance_worlds",
    queryKey: ["instance-worlds", instanceId],
  });
  const remove = useBackendMutation({ name: "delete_instance_world" });
  const openFolder = useBackendMutation({ name: "open_instance_folder" });
  const worldItems = worlds.data ?? [];
  return (
    <div className="space-y-3">
      <div className="flex justify-end">
        <ActionButton
          action={() => openFolder.mutateAsync({ folder: "saves", instanceId })}
          variant="outline"
        >
          <FolderOpen className="size-4" /> Open saves folder
        </ActionButton>
      </div>
      {worldItems.map((world) => (
        <div
          className="flex items-center gap-3 rounded-xl border border-border/50 bg-secondary/15 p-3"
          key={world.name}
        >
          <HardDrive className="size-5 shrink-0 text-primary" />
          <div className="min-w-0 flex-1">
            <p className="truncate font-medium text-sm">{world.name}</p>
            <p className="text-[11px] text-muted-foreground">
              {formatBytes(world.sizeBytes)} · Modified{" "}
              {formatDate(world.modifiedAt)}
            </p>
          </div>
          <ActionButton
            action={async () => {
              await remove.mutateAsync({ instanceId, worldName: world.name });
              await worlds.refetch();
              await refreshInstances();
              toast.success(`World ${world.name} deleted`);
            }}
            areYouSureButton="Delete world"
            areYouSureDescription={`The world “${world.name}” will be permanently deleted. Create an instance backup first if needed.`}
            requireAreYouSure
            size="icon"
            variant="destructive"
          >
            <Trash2 className="size-4" />
          </ActionButton>
        </div>
      ))}
      {worldItems.length === 0 && (
        <p className="p-8 text-center text-muted-foreground text-xs">
          No worlds in this instance.
        </p>
      )}
    </div>
  );
}

function BackupsTab({
  instanceId,
  refreshInstances,
}: {
  instanceId: string;
  refreshInstances: () => Promise<unknown>;
}) {
  const backups = useBackend({
    args: { instanceId },
    initialData: [],
    name: "list_instance_backups",
    queryKey: ["instance-backups", instanceId],
  });
  const create = useBackendMutation({ name: "create_instance_backup" });
  const restore = useBackendMutation({ name: "restore_instance_backup" });
  const remove = useBackendMutation({ name: "delete_instance_backup" });
  const openFolder = useBackendMutation({ name: "open_instance_folder" });
  const backupItems = backups.data ?? [];
  const refresh = async () => {
    await backups.refetch();
    await refreshInstances();
  };
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap justify-end gap-2">
        <ActionButton
          action={() =>
            openFolder.mutateAsync({ folder: "backups", instanceId })
          }
          variant="outline"
        >
          <FolderOpen className="size-4" /> Open backup folder
        </ActionButton>
        <ActionButton
          action={async () => {
            const backup = await create.mutateAsync({ instanceId });
            await refresh();
            toast.success(`Backup created: ${backup.name}`);
          }}
        >
          <DatabaseBackup className="size-4" /> Create full backup
        </ActionButton>
      </div>
      {backupItems.map((backup) => (
        <div
          className="flex items-center gap-3 rounded-xl border border-border/50 bg-secondary/15 p-3"
          key={backup.name}
        >
          <DatabaseBackup className="size-5 shrink-0 text-primary" />
          <div className="min-w-0 flex-1">
            <p className="truncate font-medium text-sm">{backup.name}</p>
            <p className="text-[11px] text-muted-foreground">
              {formatBytes(backup.sizeBytes)} · {formatDate(backup.createdAt)}
            </p>
          </div>
          <ActionButton
            action={async () => {
              await restore.mutateAsync({
                backupName: backup.name,
                instanceId,
              });
              await refresh();
              toast.success("Backup restored successfully");
            }}
            areYouSureButton="Restore backup"
            areYouSureDescription="Current instance files will be replaced by this backup. A rollback directory is kept until restoration succeeds."
            requireAreYouSure
            size="icon"
            variant="outline"
          >
            <ArchiveRestore className="size-4" />
          </ActionButton>
          <ActionButton
            action={async () => {
              await remove.mutateAsync({ backupName: backup.name, instanceId });
              await refresh();
              toast.success("Backup deleted");
            }}
            areYouSureButton="Delete backup"
            requireAreYouSure
            size="icon"
            variant="destructive"
          >
            <Trash2 className="size-4" />
          </ActionButton>
        </div>
      ))}
      {backupItems.length === 0 && (
        <p className="p-8 text-center text-muted-foreground text-xs">
          No backups created yet.
        </p>
      )}
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg bg-background/60 p-3">
      <p className="text-[10px] text-muted-foreground uppercase tracking-wider">
        {label}
      </p>
      <p className="mt-1 font-semibold text-sm">{value}</p>
    </div>
  );
}
