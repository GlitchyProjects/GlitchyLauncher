import {
  Award01Icon,
  Medal01Icon,
  Route01Icon,
  UserGroupIcon,
} from "@hugeicons/core-free-icons";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import {
  GlassPanel,
  GlowButton,
  SectionHeading,
  StatTile,
} from "@/components/glitchy/primitives";
import { ProfileCustomizer } from "@/components/glitchy/profile-customizer";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useBackend } from "@/hooks/use-backend";
import type { ActivityEntry, BadgeWithState } from "@/invokes";
import { presetAvatarSrc } from "@/lib/avatars";
import { HugeiconsIcon, resolveIcon } from "@/lib/icons";
import Achievements from "@/pages/glitchy/achievements";
import Badges from "@/pages/glitchy/badges";
import Journey from "@/pages/glitchy/journey";

export default function GlitchyProfile() {
  const { data: profile, refetch: refetchProfile } = useBackend({
    name: "glitchy_get_profile",
  });
  const { data: badges } = useBackend({ name: "glitchy_get_badges" });
  const { data: activity } = useBackend({
    args: { limit: 8 },
    name: "glitchy_get_recent_activity",
  });
  const { data: selectedProfile } = useBackend({
    name: "get_selected_profile",
  });

  const [customizerOpen, setCustomizerOpen] = useState(false);
  const [avatarUrl, setAvatarUrl] = useState<string | null>(null);
  const [profileTab, setProfileTab] = useState<string>("overview");

  useEffect(() => {
    if (!profile) {
      return;
    }
    const avatar = profile.customization.avatar;
    if (avatar.kind === "preset") {
      setAvatarUrl(presetAvatarSrc(avatar.id));
    } else {
      invoke<string>("glitchy_avatar_data", { filename: avatar.filename })
        .then((dataUrl) => setAvatarUrl(dataUrl))
        .catch(() => setAvatarUrl(presetAvatarSrc("glitchy")));
    }
  }, [profile?.customization.avatar]);

  if (!profile) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="animate-pulse-glow text-muted-foreground text-sm">
          Loading profile…
        </div>
      </div>
    );
  }

  const customization = profile.customization;
  const accentHex = customization.accent ?? "#2484EC";
  const stats = profile.statistics;
  const playtimeHours = Math.floor(stats.totalPlaytimeSeconds / 3600);
  const displayedBadges = (badges ?? []).filter((b) => b.displayed);
  const username = selectedProfile?.username ?? "Glitchy";
  const tagline = customization.tagline?.trim() || "Minecraft Player";

  return (
    <Tabs
      className="flex h-full flex-col gap-4"
      onValueChange={setProfileTab}
      value={profileTab}
    >
      <div className="flex w-full shrink-0 items-center">
        <TabsList className="w-full bg-secondary/40 backdrop-blur-md">
          <TabsTrigger
            className="flex-1 gap-2 px-4 font-semibold text-xs"
            value="overview"
          >
            <HugeiconsIcon icon={UserGroupIcon} size={15} strokeWidth={2} />
            <span>Overview</span>
          </TabsTrigger>
          <TabsTrigger
            className="flex-1 gap-2 px-4 font-semibold text-xs"
            value="achievements"
          >
            <HugeiconsIcon icon={Award01Icon} size={15} strokeWidth={2} />
            <span>Achievements</span>
          </TabsTrigger>
          <TabsTrigger
            className="flex-1 gap-2 px-4 font-semibold text-xs"
            value="badges"
          >
            <HugeiconsIcon icon={Medal01Icon} size={15} strokeWidth={2} />
            <span>Badges</span>
          </TabsTrigger>
          <TabsTrigger
            className="flex-1 gap-2 px-4 font-semibold text-xs"
            value="journey"
          >
            <HugeiconsIcon icon={Route01Icon} size={15} strokeWidth={2} />
            <span>Journey</span>
          </TabsTrigger>
        </TabsList>
      </div>

      <TabsContent className="flex-1 overflow-y-auto" value="overview">
        <div className="w-full animate-fade-up space-y-6">
          <GlassPanel className="relative overflow-hidden" variant="strong">
            <ProfileBackgroundLayer
              accent={accentHex}
              kind={customization.background}
            />
            <div className="relative z-10 flex flex-col items-center gap-4 p-8 sm:flex-row sm:items-end sm:gap-6">
              <AvatarWithFrame
                accent={accentHex}
                avatarUrl={avatarUrl}
                frame={customization.frame}
                glow={customization.glow}
                onClick={() => setCustomizerOpen(true)}
              />
              <div className="flex-1 text-center sm:text-left">
                <div className="text-[11px] text-muted-foreground uppercase tracking-[0.22em]">
                  Glitchy Profile
                </div>
                <h1 className="mt-1 font-black text-4xl text-glow leading-tight">
                  {username}
                </h1>
                <div className="mt-1 text-muted-foreground text-sm">
                  {tagline}
                </div>
              </div>
              <GlowButton
                className="self-center sm:self-end"
                onClick={() => setCustomizerOpen(true)}
              >
                Customize
              </GlowButton>
            </div>
          </GlassPanel>

          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <StatTile label="Playtime (h)" value={playtimeHours} />
            <StatTile label="Sessions" value={stats.totalSessions} />
            <StatTile label="Achievements" value={stats.achievementsUnlocked} />
            <StatTile label="Badges" value={stats.badgesEarned} />
          </div>

          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <GlassPanel className="p-4">
              <SectionHeading>Favorite Version</SectionHeading>
              <div className="mt-2 font-bold text-2xl text-glow">
                {stats.favoriteVersion ?? "—"}
              </div>
            </GlassPanel>
            <GlassPanel className="p-4">
              <SectionHeading>Last Played</SectionHeading>
              <div className="mt-2 font-bold text-2xl text-glow">
                {stats.lastPlayedAt ? formatRelative(stats.lastPlayedAt) : "—"}
              </div>
            </GlassPanel>
          </div>

          <GlassPanel className="p-4">
            <SectionHeading>Displayed Badges</SectionHeading>
            <div className="mt-3 flex flex-wrap gap-2">
              {displayedBadges.length === 0 ? (
                <div className="text-muted-foreground text-xs italic">
                  No badges selected — visit the Badges page to pick up to 6.
                </div>
              ) : (
                displayedBadges.map((b) => (
                  <BadgeChip accent={accentHex} badge={b} key={b.id} />
                ))
              )}
            </div>
          </GlassPanel>

          <GlassPanel className="p-4">
            <SectionHeading>Recent Activity</SectionHeading>
            <div className="mt-3 space-y-2">
              {(activity ?? []).length === 0 ? (
                <div className="text-muted-foreground text-xs italic">
                  No activity yet — launch a game to start your journey.
                </div>
              ) : (
                (activity ?? []).map((a) => (
                  <ActivityRow entry={a} key={a.id} />
                ))
              )}
            </div>
          </GlassPanel>

          <ProfileCustomizer
            current={customization}
            onOpenChange={setCustomizerOpen}
            onSaved={() => {
              refetchProfile();
              toast.success("Profile saved", {
                description: "Your customization has been persisted.",
              });
            }}
            open={customizerOpen}
          />
        </div>
      </TabsContent>


      <TabsContent className="flex-1 overflow-y-auto" value="achievements">
        <Achievements />
      </TabsContent>

      <TabsContent className="flex-1 overflow-y-auto" value="badges">
        <Badges />
      </TabsContent>

      <TabsContent className="flex-1 overflow-y-auto" value="journey">
        <Journey />
      </TabsContent>
    </Tabs>
  );
}

function AvatarWithFrame({
  avatarUrl,
  frame,
  glow,
  accent,
  onClick,
}: {
  avatarUrl: string | null;
  frame: string;
  glow: string;
  accent: string;
  onClick: () => void;
}) {
  const glowIntensity =
    {
      none: "none",
      normal: `0 0 24px ${accent}88`,
      strong: `0 0 36px ${accent}cc, 0 0 60px ${accent}55`,
      subtle: `0 0 12px ${accent}55`,
    }[glow] ?? "none";

  const frameStyle: React.CSSProperties =
    frame === "none"
      ? { boxShadow: "none" }
      : frame === "solid_blue"
        ? { boxShadow: `0 0 0 3px ${accent}, 0 0 0 6px ${accent}33` }
        : frame === "gradient_electric"
          ? {
              boxShadow: `0 0 0 3px ${accent}, 0 0 0 6px #94BCE4, ${glowIntensity}`,
            }
          : frame === "gradient_icy"
            ? {
                boxShadow: `0 0 0 3px #B4D4F4, 0 0 0 6px ${accent}55, ${glowIntensity}`,
              }
            : frame === "neon_pulse"
              ? {
                  animation: "glitchy-pulse 2.4s ease-in-out infinite",
                  boxShadow: `0 0 0 3px ${accent}, ${glowIntensity}`,
                }
              : { boxShadow: "none" };

  return (
    <button
      className="group relative size-24 shrink-0 rounded-full transition-transform hover:scale-105"
      onClick={onClick}
      style={frameStyle}
      title="Change avatar"
      type="button"
    >
      {avatarUrl ? (
        <img
          alt="avatar"
          className="size-full rounded-full object-cover"
          src={avatarUrl}
        />
      ) : (
        <div className="size-full rounded-full bg-gradient-to-br from-[#2484EC] to-[#4C8CCC]" />
      )}
      <div className="absolute inset-0 flex items-center justify-center rounded-full bg-black/0 transition-colors group-hover:bg-black/30">
        <div className="font-bold text-[10px] text-white uppercase tracking-wider opacity-0 transition-opacity group-hover:opacity-100">
          Edit
        </div>
      </div>
    </button>
  );
}

function ProfileBackgroundLayer({
  kind,
  accent,
}: {
  kind: string;
  accent: string;
}) {
  if (kind === "deep_sea") {
    return (
      <div
        className="absolute inset-0 opacity-50"
        style={{
          background: `radial-gradient(800px 400px at 20% 0%, ${accent}44, transparent 60%), radial-gradient(600px 300px at 80% 100%, #4C8CCC44, transparent 60%)`,
        }}
      />
    );
  }
  if (kind === "grid") {
    return (
      <div
        className="absolute inset-0 opacity-30"
        style={{
          backgroundImage: `linear-gradient(${accent}22 1px, transparent 1px), linear-gradient(90deg, ${accent}22 1px, transparent 1px)`,
          backgroundSize: "32px 32px",
        }}
      />
    );
  }
  if (kind === "particles") {
    return (
      <div
        className="absolute inset-0 opacity-40"
        style={{
          background: `radial-gradient(2px 2px at 20% 30%, ${accent}, transparent), radial-gradient(2px 2px at 60% 70%, ${accent}, transparent), radial-gradient(1px 1px at 80% 20%, #B4D4F4, transparent), radial-gradient(1px 1px at 40% 80%, #B4D4F4, transparent)`,
          backgroundSize: "200px 200px",
        }}
      />
    );
  }
  if (kind === "solid") {
    return (
      <div
        className="absolute inset-0 opacity-20"
        style={{ background: accent }}
      />
    );
  }
  return (
    <div
      className="absolute inset-0 opacity-50"
      style={{
        background: `linear-gradient(135deg, ${accent}33, transparent 50%, #94BCE433)`,
      }}
    />
  );
}

function BadgeChip({
  badge,
  accent,
}: {
  badge: BadgeWithState;
  accent: string;
}) {
  return (
    <div
      className="flex animate-fade-up items-center gap-2 rounded-lg px-3 py-1.5 font-semibold text-xs"
      style={{
        background: `${accent}1a`,
        border: `1px solid ${accent}55`,
        boxShadow: `0 0 12px ${accent}33`,
        color: "#fff",
      }}
    >
      <HugeiconsIcon icon={resolveIcon(badge.icon)} size={14} />
      <span>{badge.title}</span>
    </div>
  );
}

function ActivityRow({ entry }: { entry: ActivityEntry }) {
  return (
    <div className="flex items-center gap-3 rounded-lg px-3 py-2 transition-colors hover:bg-white/[0.03]">
      <div className="flex size-8 items-center justify-center rounded-md bg-[#2484EC]/15 text-[#94BCE4]">
        <HugeiconsIcon icon={resolveIcon(entry.icon)} size={14} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate font-medium text-sm">{entry.title}</div>
        <div className="text-[10px] text-muted-foreground">
          {formatRelative(entry.timestamp)}
        </div>
      </div>
    </div>
  );
}

function formatRelative(unixSeconds: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = now - unixSeconds;
  if (diff < 60) {
    return "just now";
  }
  if (diff < 3600) {
    return `${Math.floor(diff / 60)}m ago`;
  }
  if (diff < 86_400) {
    return `${Math.floor(diff / 3600)}h ago`;
  }
  if (diff < 604_800) {
    return `${Math.floor(diff / 86_400)}d ago`;
  }
  return new Date(unixSeconds * 1000).toLocaleDateString();
}
