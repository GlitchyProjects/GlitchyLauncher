import {
  CpuIcon,
  Download02Icon,
  RefreshIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useMemo, useState } from "react";
import { toast } from "sonner";
import { LoadingSwap } from "@/components/ui/animated/swapper";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useBackend, useBackendMutation } from "@/hooks/use-backend";

export function LauncherSettings() {
  const minLimit = 1024;
  const maxLimit = 16_384;
  const [localMinRam, setLocalMinRam] = useState<number | null>(null);
  const [localMaxRam, setLocalMaxRam] = useState<number | null>(null);

  const minRamQuery = useBackend({ name: "get_minimum_ram_usage" });
  const maxRamQuery = useBackend({ name: "get_maximum_ram_usage" });

  const { mutateAsync: setMinRamMutation } = useBackendMutation({
    name: "set_minimum_ram_usage",
  });
  const { mutateAsync: setMaxRamMutation } = useBackendMutation({
    name: "set_maximum_ram_usage",
  });
  const { mutateAsync: saveMutation } = useBackendMutation({ name: "save" });

  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
  const { data: updateInfo, refetch: refetchUpdate } = useBackend({
    name: "check_launcher_update",
  });

  const isQueriesLoading = minRamQuery.isLoading || maxRamQuery.isLoading;

  const minRam = localMinRam ?? (minRamQuery.data as number) ?? 2048;
  const maxRam = localMaxRam ?? (maxRamQuery.data as number) ?? 4096;

  const sliderTrackStyle = useMemo(() => {
    const totalRange = maxLimit - minLimit;
    const minPercent = ((minRam - minLimit) / totalRange) * 100;
    const maxPercent = ((maxRam - minLimit) / totalRange) * 100;

    return {
      background: `linear-gradient(to right, #27272a 0%, #27272a ${minPercent}%, #0f766e ${minPercent}%, #0f766e ${maxPercent}%, #27272a ${maxPercent}%, #27272a 100%)`,
    };
  }, [minRam, maxRam]);

  const handleMinMaxRamChange = async (type: "min" | "max", value: number) => {
    if (type === "min") {
      const targetMin = Math.min(value, maxRam);
      setLocalMinRam(targetMin);
      await setMinRamMutation({ ramUsage: targetMin });
    } else {
      const targetMax = Math.max(value, minRam);
      setLocalMaxRam(targetMax);
      await setMaxRamMutation({ ramUsage: targetMax });
    }
    await saveMutation(undefined);
  };

  return (
    <LoadingSwap className="h-full w-full" isLoading={isQueriesLoading}>
      <div className="max-w-xl space-y-6">
        <div className="space-y-1">
          <h3 className="flex items-center gap-2 font-semibold text-foreground text-sm">
            <HugeiconsIcon className="text-primary" icon={CpuIcon} size={16} />{" "}
            Memory Allocation (RAM)
          </h3>
          <p className="text-muted-foreground text-xs">
            Adjust system memory parameters provisioned for game executions.
          </p>
        </div>

        <div className="space-y-6 rounded-xl border border-border/40 bg-secondary/30 p-5">
          <div className="flex items-center justify-between border-border/30 border-b pb-3 font-mono text-xs">
            <div className="flex flex-col">
              <span className="font-bold font-sans text-[10px] text-muted-foreground uppercase tracking-wider">
                Min allocation
              </span>
              <span className="font-bold text-primary text-sm">
                {minRam} MB (~{(minRam / 1024).toFixed(1)} GB)
              </span>
            </div>
            <div className="flex flex-col items-end">
              <span className="font-bold font-sans text-[10px] text-muted-foreground uppercase tracking-wider">
                Max allocation
              </span>
              <span className="font-bold text-emerald-400 text-sm">
                {maxRam} MB (~{(maxRam / 1024).toFixed(1)} GB)
              </span>
            </div>
          </div>

          <div className="relative flex h-6 w-full items-center pt-4 pb-2">
            <input
              className="pointer-events-none absolute top-0 bottom-0 z-30 m-auto h-1 w-full appearance-none bg-transparent accent-primary [&::-webkit-slider-thumb]:pointer-events-auto"
              max={maxLimit}
              min={minLimit}
              onChange={(e) =>
                handleMinMaxRamChange("min", Number(e.target.value))
              }
              step={512}
              type="range"
              value={minRam}
            />
            <input
              className="pointer-events-none absolute top-0 bottom-0 z-30 m-auto h-1 w-full appearance-none bg-transparent accent-emerald-500 [&::-webkit-slider-thumb]:pointer-events-auto"
              max={maxLimit}
              min={minLimit}
              onChange={(e) =>
                handleMinMaxRamChange("max", Number(e.target.value))
              }
              step={512}
              type="range"
              value={maxRam}
            />
            <div
              className="absolute top-0 bottom-0 z-10 m-auto h-1.5 w-full rounded-lg transition-[background] duration-75"
              style={sliderTrackStyle}
            />
          </div>
        </div>

        {/* Launcher Updates Section */}
        <div className="space-y-3 border-border/40 border-t pt-4">
          <div className="space-y-1">
            <h3 className="flex items-center gap-2 font-semibold text-foreground text-sm">
              <HugeiconsIcon
                className="text-primary"
                icon={Download02Icon}
                size={16}
              />{" "}
              Launcher Updates
            </h3>
            <p className="text-muted-foreground text-xs">
              Check for the latest Glitchy Launcher releases and feature
              updates.
            </p>
          </div>

          <div className="flex flex-col gap-4 rounded-xl border border-border/40 bg-secondary/30 p-5">
            <div className="flex items-center justify-between">
              <div className="flex flex-col gap-1">
                <div className="flex items-center gap-2">
                  <span className="font-bold text-foreground text-sm">
                    Glitchy Launcher
                  </span>
                  <Badge className="font-mono text-[11px]" variant="outline">
                    v{updateInfo?.currentVersion || "1.2.1"}
                  </Badge>
                </div>
                <span className="text-muted-foreground text-xs">
                  {updateInfo?.hasUpdate
                    ? `New version ${updateInfo.latestVersion} is available!`
                    : "Your launcher is up to date."}
                </span>
              </div>

              <Button
                className="gap-1.5 text-xs"
                disabled={isCheckingUpdate}
                onClick={async () => {
                  setIsCheckingUpdate(true);
                  try {
                    const res = await refetchUpdate();
                    if (res.data?.hasUpdate) {
                      toast.success(
                        `New version ${res.data.latestVersion} is available!`
                      );
                    } else {
                      toast.info("You are running the latest version.");
                    }
                  } finally {
                    setIsCheckingUpdate(false);
                  }
                }}
                size="sm"
                variant={updateInfo?.hasUpdate ? "default" : "outline"}
              >
                <HugeiconsIcon
                  className={isCheckingUpdate ? "animate-spin" : ""}
                  icon={RefreshIcon}
                  size={14}
                />
                <span>
                  {isCheckingUpdate ? "Checking..." : "Check for Updates"}
                </span>
              </Button>
            </div>

            {updateInfo?.hasUpdate && (
              <div className="flex flex-col gap-2 rounded-lg border border-primary/30 bg-primary/10 p-3 text-xs">
                <div className="flex items-center justify-between">
                  <span className="font-bold text-primary">
                    Changelog v{updateInfo.latestVersion}:
                  </span>
                  <a
                    className="inline-flex items-center gap-1 font-semibold text-primary underline hover:text-primary/80"
                    href={updateInfo.downloadUrl || updateInfo.releaseUrl}
                    rel="noopener noreferrer"
                    target="_blank"
                  >
                    <HugeiconsIcon icon={Download02Icon} size={13} />
                    <span>Download Update</span>
                  </a>
                </div>
                {updateInfo.releaseNotes && (
                  <p className="line-clamp-4 whitespace-pre-line font-mono text-[11px] text-muted-foreground">
                    {updateInfo.releaseNotes}
                  </p>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </LoadingSwap>
  );
}
