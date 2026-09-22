import { GlassPanel, SectionHeading } from "@/components/glitchy/primitives";
import { useBackend } from "@/hooks/use-backend";
import type { AchievementRarity, AchievementWithState } from "@/invokes";
import { HugeiconsIcon, resolveIcon } from "@/lib/icons";

const RARITY_LABEL: Record<AchievementRarity, string> = {
  common: "Common",
  epic: "Epic",
  legendary: "Legendary",
  rare: "Rare",
};
const RARITY_COLOR: Record<AchievementRarity, string> = {
  common: "#94BCE4",
  epic: "#2484EC",
  legendary: "#247CD4",
  rare: "#4C8CCC",
};

/**
 * Coerce a maybe-undefined numeric field to a safe number for display.
 * The Rust backend marks `target` and `unlockedAt` as `Option<...>`,
 * which serialize as `null` (not `undefined`), but we defend against
 * both here so the page can NEVER crash because a legitimately-optional
 * field is missing.
 */
function safeNum(v: unknown, fallback = 0): number {
  if (typeof v === "number" && Number.isFinite(v)) {
    return v;
  }
  return fallback;
}

export default function Achievements() {
  const { data: achievements } = useBackend({
    name: "glitchy_get_achievements",
  });
  // Defensive: backend should always return an array, but if the IPC
  // layer ever hands us null/undefined we treat it as empty.
  const list: AchievementWithState[] = Array.isArray(achievements)
    ? achievements
    : [];
  const unlocked = list.filter((a) => a?.unlocked === true);
  const locked = list.filter((a) => !a?.unlocked);
  const totalProgress =
    list.length === 0 ? 0 : Math.round((unlocked.length / list.length) * 100);

  return (
    <div className="mx-auto max-w-5xl animate-fade-up space-y-6">
      <GlassPanel className="p-6" variant="strong">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <SectionHeading>Achievements</SectionHeading>
            <div className="mt-2 font-black text-4xl text-glow">
              {unlocked.length}
              <span className="text-2xl text-muted-foreground">
                {" "}
                / {list.length}
              </span>
            </div>
            <div className="mt-1 text-muted-foreground text-xs">
              Unlocked achievements on your Glitchy journey.
            </div>
          </div>
          <div className="w-full sm:w-64">
            <div className="mb-2 text-[10px] text-muted-foreground uppercase tracking-[0.16em]">
              Overall progress
            </div>
            <div className="h-2 overflow-hidden rounded-full bg-white/5">
              <div
                className="h-full bg-gradient-to-r from-[#2484EC] to-[#B4D4F4] transition-all duration-700"
                style={{
                  boxShadow: "0 0 12px #2484ECCC",
                  width: `${totalProgress}%`,
                }}
              />
            </div>
            <div className="mt-1 text-right text-[10px] text-muted-foreground tabular-nums">
              {totalProgress}%
            </div>
          </div>
        </div>
      </GlassPanel>

      {unlocked.length > 0 && (
        <div className="space-y-3">
          <SectionHeading>Unlocked · {unlocked.length}</SectionHeading>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {unlocked.map((a) => (
              <AchievementCard achievement={a} key={a?.id ?? Math.random()} />
            ))}
          </div>
        </div>
      )}

      {locked.length > 0 && (
        <div className="space-y-3">
          <SectionHeading>Locked · {locked.length}</SectionHeading>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {locked.map((a) => (
              <AchievementCard achievement={a} key={a?.id ?? Math.random()} />
            ))}
          </div>
        </div>
      )}

      {list.length === 0 && (
        <GlassPanel className="p-8 text-center text-muted-foreground text-sm">
          No achievements available. Launch a game to begin your journey.
        </GlassPanel>
      )}
    </div>
  );
}

function AchievementCard({
  achievement,
}: {
  achievement: AchievementWithState;
}) {
  // Destructure with explicit fallbacks so a missing field never throws.
  const title = achievement?.title ?? "Unknown";
  const description = achievement?.description ?? "";
  const icon = achievement?.icon ?? "Star01Icon";
  const rarity: AchievementRarity = achievement?.rarity ?? "common";
  const unlocked = achievement?.unlocked === true;
  const progress = safeNum(achievement?.progress);
  const target = achievement?.target ?? null;
  const unlockedAt = achievement?.unlockedAt ?? null;

  const color = RARITY_COLOR[rarity] ?? "#94BCE4";
  const hasTarget = typeof target === "number" && target > 0;
  const progressPct = hasTarget
    ? Math.min(100, Math.round((progress / (target as number)) * 100))
    : 0;

  return (
    <GlassPanel
      className="flex flex-col gap-3 p-4 transition-all"
      style={
        unlocked
          ? {
              background: `linear-gradient(135deg, ${color}11, transparent 60%)`,
              boxShadow: `0 0 0 1px ${color}55, 0 0 24px ${color}33`,
            }
          : undefined
      }
    >
      <div className="flex items-start gap-3">
        <div
          className={
            unlocked
              ? "flex size-12 shrink-0 items-center justify-center rounded-xl"
              : "flex size-12 shrink-0 items-center justify-center rounded-xl opacity-50 grayscale"
          }
          style={{
            background: unlocked ? `${color}22` : "rgba(255,255,255,0.04)",
            border:
              "1px solid " +
              (unlocked ? color + "55" : "rgba(255,255,255,0.05)"),
            boxShadow: unlocked ? `0 0 16px ${color}44` : "none",
          }}
        >
          <HugeiconsIcon
            icon={resolveIcon(icon)}
            size={24}
            style={{ color: unlocked ? color : "#6c7a89" }}
          />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <div
              className={`truncate font-bold text-sm ${unlocked ? "text-glow" : "text-muted-foreground"}`}
            >
              {title}
            </div>
            <span
              className="rounded px-1.5 py-0.5 font-bold text-[9px] uppercase tracking-wider"
              style={{
                background: unlocked ? `${color}22` : "rgba(255,255,255,0.04)",
                color: unlocked ? color : "#6c7a89",
              }}
            >
              {RARITY_LABEL[rarity] ?? rarity}
            </span>
          </div>
          <div className="mt-1 line-clamp-2 text-muted-foreground text-xs">
            {description}
          </div>
        </div>
      </div>

      {unlocked ? (
        <div className="text-[10px] text-muted-foreground tabular-nums">
          {unlockedAt
            ? `Unlocked ${new Date(unlockedAt * 1000).toLocaleDateString()}`
            : "Unlocked"}
        </div>
      ) : hasTarget ? (
        <div>
          <div className="mb-1 flex justify-between text-[10px] text-muted-foreground tabular-nums">
            <span>{progress.toLocaleString()}</span>
            <span>{(target as number).toLocaleString()}</span>
          </div>
          <div className="h-1.5 overflow-hidden rounded-full bg-white/5">
            <div
              className="h-full bg-gradient-to-r from-[#4C8CCC] to-[#94BCE4] transition-all duration-500"
              style={{ width: `${progressPct}%` }}
            />
          </div>
        </div>
      ) : null}
    </GlassPanel>
  );
}
