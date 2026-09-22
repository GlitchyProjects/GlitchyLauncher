import { useState } from "react";
import { toast } from "sonner";
import {
  GlassPanel,
  GlowButton,
  SectionHeading,
} from "@/components/glitchy/primitives";
import { useBackend, useBackendMutation } from "@/hooks/use-backend";
import type { BadgeWithState } from "@/invokes";
import { HugeiconsIcon, resolveIcon } from "@/lib/icons";

const MAX_DISPLAYED = 6;

export default function Badges() {
  const { data: badges } = useBackend({ name: "glitchy_get_badges" });
  const { mutateAsync: saveDisplayed, isPending } = useBackendMutation({
    name: "glitchy_set_displayed_badges",
    onSuccess: () =>
      toast.success("Badges updated", {
        description: "Your displayed badges have been saved.",
      }),
  });

  const [displayed, setDisplayed] = useState<string[] | null>(null);
  const all = badges ?? [];
  const displayedIds =
    displayed ?? all.filter((b) => b.displayed).map((b) => b.id);

  const toggle = (id: string) => {
    if (displayedIds.includes(id)) {
      setDisplayed(displayedIds.filter((x) => x !== id));
    } else if (displayedIds.length < MAX_DISPLAYED) {
      setDisplayed([...displayedIds, id]);
    } else {
      toast.warning("Badge slots full", {
        description: `You can display up to ${MAX_DISPLAYED} badges at once.`,
      });
    }
  };

  const move = (id: string, dir: -1 | 1) => {
    const idx = displayedIds.indexOf(id);
    const newIdx = idx + dir;
    if (newIdx < 0 || newIdx >= displayedIds.length) {
      return;
    }
    const next = [...displayedIds];
    [next[idx], next[newIdx]] = [next[newIdx], next[idx]];
    setDisplayed(next);
  };

  const handleSave = async () => {
    await saveDisplayed({ badgeIds: displayedIds });
    setDisplayed(null);
  };

  return (
    <div className="mx-auto max-w-5xl animate-fade-up space-y-6">
      <GlassPanel className="p-6" variant="strong">
        <SectionHeading>Badges</SectionHeading>
        <p className="mt-2 max-w-xl text-muted-foreground text-xs">
          Badges represent milestones and status in your Glitchy journey. Pick
          up to {MAX_DISPLAYED} to display on your profile. Auto-awarded badges
          are earned by playing; others can be equipped freely.
        </p>
      </GlassPanel>

      <GlassPanel className="p-4">
        <div className="mb-3 flex items-center justify-between">
          <SectionHeading>
            Displayed · {displayedIds.length}/{MAX_DISPLAYED}
          </SectionHeading>
          <GlowButton
            className="px-4 py-2 text-xs"
            disabled={isPending || displayed === null}
            onClick={handleSave}
          >
            {isPending ? "Saving…" : "Save"}
          </GlowButton>
        </div>
        {displayedIds.length === 0 ? (
          <div className="text-muted-foreground text-xs italic">
            No badges selected.
          </div>
        ) : (
          <div className="flex flex-wrap gap-2">
            {displayedIds.map((id, idx) => {
              const badge = all.find((b) => b.id === id);
              if (!badge) {
                return null;
              }
              return (
                <div
                  className="flex animate-fade-up items-center gap-2 rounded-lg px-3 py-1.5 font-semibold text-xs"
                  key={id}
                  style={{
                    background: "#2484EC1a",
                    border: "1px solid #2484EC55",
                    boxShadow: "0 0 12px #2484EC33",
                    color: "#fff",
                  }}
                >
                  <HugeiconsIcon icon={resolveIcon(badge.icon)} size={14} />
                  <span>{badge.title}</span>
                  <div className="ml-1 flex gap-1">
                    <button
                      className="flex size-5 items-center justify-center rounded hover:bg-white/10 disabled:opacity-30"
                      disabled={idx === 0}
                      onClick={() => move(id, -1)}
                      title="Move left"
                      type="button"
                    >
                      ‹
                    </button>
                    <button
                      className="flex size-5 items-center justify-center rounded hover:bg-white/10 disabled:opacity-30"
                      disabled={idx === displayedIds.length - 1}
                      onClick={() => move(id, 1)}
                      title="Move right"
                      type="button"
                    >
                      ›
                    </button>
                    <button
                      className="flex size-5 items-center justify-center rounded hover:bg-red-500/20 hover:text-red-300"
                      onClick={() => toggle(id)}
                      title="Remove"
                      type="button"
                    >
                      ×
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </GlassPanel>

      <div className="space-y-3">
        <SectionHeading>All Badges · {all.length}</SectionHeading>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {all.map((b) => (
            <BadgeCard
              badge={b}
              isDisplayed={displayedIds.includes(b.id)}
              key={b.id}
              onToggle={() => toggle(b.id)}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

function BadgeCard({
  badge,
  isDisplayed,
  onToggle,
}: {
  badge: BadgeWithState;
  isDisplayed: boolean;
  onToggle: () => void;
}) {
  const earned = badge.earned || !badge.autoAwarded;
  return (
    <GlassPanel
      className="flex cursor-pointer items-center gap-3 p-4 transition-all"
      onClick={onToggle}
      role="button"
      style={
        isDisplayed
          ? { boxShadow: "0 0 0 1px #2484EC55, 0 0 24px #2484EC33" }
          : earned
            ? undefined
            : { opacity: 0.5 }
      }
    >
      <div
        className="flex size-12 shrink-0 items-center justify-center rounded-xl"
        style={{
          background: earned ? "#2484EC22" : "rgba(255,255,255,0.04)",
          border: `1px solid ${earned ? "#2484EC55" : "rgba(255,255,255,0.05)"}`,
          boxShadow: earned && isDisplayed ? "0 0 16px #2484EC44" : "none",
        }}
      >
        <HugeiconsIcon
          icon={resolveIcon(badge.icon)}
          size={24}
          style={{ color: earned ? "#94BCE4" : "#6c7a89" }}
        />
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate font-bold text-sm">{badge.title}</div>
        <div className="mt-0.5 line-clamp-2 text-muted-foreground text-xs">
          {badge.description}
        </div>
        <div className="mt-1.5 text-[10px] text-muted-foreground">
          {badge.autoAwarded ? (earned ? "Earned" : "Locked") : "Equip freely"}
        </div>
      </div>
      <div
        className={`flex size-5 items-center justify-center rounded-md font-bold text-[10px] transition-all ${
          isDisplayed
            ? "bg-[#2484EC] text-white"
            : "border border-white/20 text-transparent"
        }`}
      >
        ✓
      </div>
    </GlassPanel>
  );
}
