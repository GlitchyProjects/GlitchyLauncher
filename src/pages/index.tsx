import { Alert01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { listen } from "@tauri-apps/api/event";
import { Check, Pencil, X } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { ActionButton } from "@/components/ui/action-button";
import { Button } from "@/components/ui/button";
import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
} from "@/components/ui/combobox";
import { Empty, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { useBackend, useBackendMutation } from "@/hooks/use-backend";
import type { InstanceSummary, LaunchProgress } from "@/invokes";
import { cn } from "@/lib/utils";
import { errorText } from "@/messages";
import { useConfig } from "@/stores/config";
import { useAccountStore } from "@/stores/account";

const WALLPAPERS = ["/wallpapers/wallpaper.png"];

function WallpaperSlideshow() {
  return (
    <div className="pointer-events-none absolute inset-0 overflow-hidden">
      <img
        alt="Minecraft artwork"
        className="absolute inset-0 size-full object-cover object-center"
        height={1080}
        src="/wallpapers/wallpaper.png"
        width={1920}
      />
    </div>
  );
}

export default function IndexPage() {
  return (
    <div className="h-full">
      {/* Main Content Area */}
      <div className="relative flex h-full flex-1 flex-col overflow-hidden rounded-xl bg-black">
        {/* Launcher key art slideshow (15-minute fade cycle) */}
        <WallpaperSlideshow />

        {/* Gradient Overlay */}
        <div className="pointer-events-none absolute inset-0 bg-gradient-to-br from-black/15 via-black/20 to-black/80" />

        {/* Content */}
        <div className="relative z-10 flex flex-1 flex-col justify-end p-8">
          <div className="max-w-2xl">
            <h2 className="mb-4 w-fit bg-transparent font-black text-5xl text-white mix-blend-difference [text-shadow:0_2px_10px_rgba(127,127,127,0.45)]">
              Glitchy Launcher
            </h2>
          </div>
        </div>

        {/* Bottom Action Bar */}
        <div className="relative z-10 flex h-24 items-center justify-between border-white/[0.08] border-t bg-black/65 px-8 shadow-[0_-12px_40px_rgba(0,0,0,0.7)] backdrop-blur-xl">
          <div className="flex w-[340px] flex-col gap-1.5">
            <span className="font-semibold text-[11px] text-muted-foreground/70 uppercase tracking-wider">
              Minecraft Instance
            </span>
            <VersionSelect />
          </div>

          <div className="w-64">
            <PlayButton />
          </div>
        </div>
      </div>
    </div>
  );
}

function VersionSelect() {
  const { version, setVersion } = useConfig();
  const [isRenaming, setIsRenaming] = useState(false);
  const [customNameInput, setCustomNameInput] = useState("");

  const {
    data: instances,
    error,
    refetch,
  } = useBackend({
    initialData: [],
    initialDataUpdatedAt: 0,
    name: "list_instances",
    queryKey: ["instances"],
  });

  const updateSettings = useBackendMutation({
    name: "update_instance_settings",
  });

  const currentInstance =
    instances?.find((i) => i.id === version) ?? instances?.[0] ?? null;

  useEffect(() => {
    if (instances && instances.length > 0) {
      if (!version || !instances.some((i) => i.id === version)) {
        setVersion(instances[0].id);
      }
    } else if (instances && instances.length === 0 && version) {
      setVersion("");
    }
  }, [instances, version, setVersion]);

  useEffect(() => {
    if (currentInstance) {
      setCustomNameInput(currentInstance.displayName);
    }
  }, [currentInstance?.id, currentInstance?.displayName]);

  const handleSaveRename = async () => {
    if (!currentInstance || !customNameInput.trim()) {
      return;
    }
    try {
      await updateSettings.mutateAsync({
        displayName: customNameInput.trim(),
        instanceId: currentInstance.id,
        ramMaxMb: currentInstance.ramMaxMb ?? null,
        ramMinMb: currentInstance.ramMinMb ?? null,
      });
      await refetch();
      setIsRenaming(false);
      toast.success("Instance renamed successfully");
    } catch {
      toast.error("Failed to rename instance");
    }
  };

  if (error) {
    return (
      <Empty className="h-12 w-full flex-row justify-start gap-2 rounded-xl border border-destructive/20 bg-destructive/5 p-2">
        <HugeiconsIcon
          className="text-destructive"
          icon={Alert01Icon}
          size={20}
        />
        <EmptyTitle className="text-destructive text-sm">
          {errorText(error.code).title}
        </EmptyTitle>
      </Empty>
    );
  }

  if (isRenaming && currentInstance) {
    return (
      <div className="flex h-12 items-center gap-1.5">
        <Input
          autoFocus
          className="h-12 flex-1 rounded-lg border-primary/50 bg-white/10 px-3 text-white text-xs placeholder:text-muted-foreground/60 focus:border-primary shadow-inner"
          onChange={(e) => setCustomNameInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              handleSaveRename();
            } else if (e.key === "Escape") {
              setIsRenaming(false);
            }
          }}
          placeholder="Custom instance name..."
          value={customNameInput}
        />
        <Button
          className="h-12 px-3 text-xs font-semibold bg-emerald-600 hover:bg-emerald-500 text-white rounded-lg shadow-sm"
          disabled={!customNameInput.trim() || updateSettings.isPending}
          onClick={handleSaveRename}
          size="sm"
          type="button"
        >
          <Check className="size-4" />
        </Button>
        <Button
          className="h-12 px-2.5 text-xs text-muted-foreground hover:text-white rounded-lg"
          onClick={() => setIsRenaming(false)}
          size="sm"
          type="button"
          variant="ghost"
        >
          <X className="size-4" />
        </Button>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2">
      <div className="min-w-0 flex-1">
        <Combobox
          autoHighlight
          isItemEqualToValue={(a: InstanceSummary | null, b: InstanceSummary | null) => a?.id === b?.id}
          items={instances}
          onValueChange={(selectedInst: InstanceSummary | null) => {
            if (selectedInst) {
              setVersion(selectedInst.id);
            }
          }}
          value={currentInstance}
        >
          <ComboboxInput
            className="h-12 w-full rounded-lg border-white/10 bg-white/5 text-white shadow-inner transition-colors placeholder:text-muted-foreground/60 hover:border-white/20 focus:border-primary/50"
            placeholder="Select an Instance"
            value={currentInstance?.displayName ?? ""}
          />
          <ComboboxContent className="rounded-lg border-white/10 bg-[#161616]/95 text-white shadow-2xl backdrop-blur-xl">
            <ComboboxEmpty className="p-3 text-center text-muted-foreground text-xs">
              No instances found.
            </ComboboxEmpty>
            <ComboboxList>
              {(inst: InstanceSummary) => (
                <ComboboxItem
                  className="cursor-pointer flex items-center justify-between rounded-md px-3 py-2.5 text-xs transition-colors hover:bg-white/10"
                  key={inst.id}
                  value={inst}
                >
                  <div className="flex flex-col min-w-0 pr-2">
                    <span className="font-semibold text-white truncate text-xs">
                      {inst.displayName}
                    </span>
                    <span className="text-[10px] text-muted-foreground/80 truncate">
                      {inst.gameVersion} · {inst.loader.toUpperCase()}
                    </span>
                  </div>
                  {inst.id === version && (
                    <span className="shrink-0 text-[10px] font-medium text-emerald-400 bg-emerald-500/15 px-1.5 py-0.5 rounded">
                      Active
                    </span>
                  )}
                </ComboboxItem>
              )}
            </ComboboxList>
          </ComboboxContent>
        </Combobox>
      </div>

      {currentInstance && (
        <Button
          className="h-12 w-12 shrink-0 rounded-lg border border-white/10 bg-white/5 text-muted-foreground hover:bg-white/10 hover:text-white"
          onClick={() => {
            setCustomNameInput(currentInstance.displayName);
            setIsRenaming(true);
          }}
          size="icon"
          title="Rename instance"
          type="button"
          variant="ghost"
        >
          <Pencil className="size-4" />
        </Button>
      )}
    </div>
  );
}

// Percent at which we consider the launch "almost there".
const LAUNCH_NEAR_THRESHOLD = 80;
// How long the completed bar stays visible after a successful launch.
const LAUNCH_DONE_LINGER_MS = 3000;

function LaunchProgressBar({
  isPending,
  justLaunched,
  progress,
}: {
  isPending: boolean;
  justLaunched: boolean;
  progress: LaunchProgress | null;
}) {
  if (!(isPending || justLaunched)) {
    return null;
  }

  let percent = 100;
  let label = "Game launched";
  if (isPending) {
    percent = progress ? progress.percent : 0;
    label = progress ? progress.phase : "Starting launch...";
  }
  const near = isPending && percent >= LAUNCH_NEAR_THRESHOLD;

  return (
    <div className="absolute inset-x-8 bottom-full mb-3 flex flex-col gap-1.5">
      <div className="flex items-center justify-between font-semibold text-xs drop-shadow">
        <span className="text-gray-300">{label}</span>
        {near ? (
          <span className="rounded-full bg-emerald-500/15 px-2 py-0.5 text-emerald-400">
            Almost there!
          </span>
        ) : (
          <span className="text-gray-400">{percent}%</span>
        )}
      </div>
      <div className="h-1.5 w-full overflow-hidden rounded-full bg-[#111]">
        <div
          className={cn(
            "h-full rounded-full transition-all duration-500",
            near || justLaunched ? "bg-emerald-500" : "bg-primary"
          )}
          style={{ width: `${percent}%` }}
        />
      </div>
    </div>
  );
}

function PlayButton() {
  const version = useConfig((state) => state.version);
  const { user, openAuthModal } = useAccountStore();

  const { mutateAsync, isPending } = useBackendMutation({
    name: "play",
  });

  const [progress, setProgress] = useState<LaunchProgress | null>(null);
  const [justLaunched, setJustLaunched] = useState(false);

  // The Rust `play` command emits `launch-progress` events while it
  // prepares the game (manifest → java → args → spawn).
  useEffect(() => {
    const unlisten = listen<LaunchProgress>("launch-progress", (event) => {
      setProgress(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Reset the bar when a new launch starts.
  useEffect(() => {
    if (isPending) {
      setProgress(null);
      setJustLaunched(false);
    }
  }, [isPending]);

  // Keep the completed bar visible briefly after the command resolves.
  useEffect(() => {
    if (!justLaunched) {
      return;
    }
    const timer = setTimeout(
      () => setJustLaunched(false),
      LAUNCH_DONE_LINGER_MS
    );
    return () => clearTimeout(timer);
  }, [justLaunched]);

  const noVersion = version === null;
  const title = !user
    ? "Login and Play with your Glitchy Account"
    : noVersion
      ? "Select an instance to play"
      : "Play Minecraft";
  const text = !user
    ? "Login and Play"
    : playButtonText(isPending, noVersion);

  return (
    <>
      <LaunchProgressBar
        isPending={isPending}
        justLaunched={justLaunched}
        progress={progress}
      />

      <ActionButton
        action={async () => {
          if (!user) {
            openAuthModal(
              "login",
              "برای ورود به بازی و همگام‌سازی ابری، داشتن حساب کاربری گلیچی الزامی است."
            );
            return;
          }
          if (version === null) {
            return;
          }
          await mutateAsync({ selectedVersion: version });
          setJustLaunched(true);
        }}
        className={cn(
          "h-14 w-full select-none rounded-lg font-black text-xl uppercase tracking-wider transition-all",
          isPending
            ? "bg-amber-600/90 text-white shadow-[0_4px_20px_rgba(217,119,6,0.35)]"
            : noVersion && user
              ? "border border-white/5 bg-secondary/70 text-muted-foreground"
              : "bg-emerald-600 text-white shadow-[0_4px_24px_rgba(16,185,129,0.35)] hover:bg-emerald-500 hover:shadow-[0_4px_30px_rgba(16,185,129,0.55)] active:scale-[0.99]"
        )}
        disabled={isPending || (Boolean(user) && noVersion)}
        title={title}
      >
        {text}
      </ActionButton>
    </>
  );
}



function playButtonText(isPending: boolean, noVersion: boolean) {
  if (isPending) {
    return "LAUNCHING...";
  }
  if (noVersion) {
    return "SELECT VERSION";
  }
  return "PLAY";
}
