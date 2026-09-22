import { GlassPanel, SectionHeading } from "@/components/glitchy/primitives";
import { useBackend } from "@/hooks/use-backend";
import type { JourneyEvent } from "@/invokes";
import { HugeiconsIcon, resolveIcon } from "@/lib/icons";

const journeyDayKey = (timestamp: number): string => {
  const date = new Date(timestamp * 1000);
  return `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;
};

const removeRepeatedEvents = (events: JourneyEvent[]): JourneyEvent[] => {
  const seen = new Set<string>();

  return events.filter((event) => {
    const key = [
      journeyDayKey(event.timestamp),
      event.kind,
      event.title.trim(),
      event.description.trim(),
    ].join("\u0000");
    if (seen.has(key)) {
      return false;
    }
    seen.add(key);
    return true;
  });
};

export default function Journey() {
  const { data: journey } = useBackend({ name: "glitchy_get_journey" });
  const events = removeRepeatedEvents(
    (journey?.events ?? []).slice().sort((a, b) => b.timestamp - a.timestamp)
  );

  const byYear = new Map<number, JourneyEvent[]>();
  for (const ev of events) {
    const year = new Date(ev.timestamp * 1000).getFullYear();
    if (!byYear.has(year)) {
      byYear.set(year, []);
    }
    byYear.get(year)?.push(ev);
  }
  const years = Array.from(byYear.keys()).sort((a, b) => b - a);

  return (
    <div className="mx-auto max-w-3xl animate-fade-up space-y-6">
      <GlassPanel className="p-6" variant="strong">
        <SectionHeading>Minecraft Journey</SectionHeading>
        <p className="mt-2 max-w-xl text-muted-foreground text-xs">
          A visual timeline of your history with Glitchy Launcher. The timeline
          grows automatically as you play, customize, and reach milestones.
        </p>
        {events.length === 0 && (
          <div className="mt-4 text-muted-foreground text-sm italic">
            Your journey begins with your first launch.
          </div>
        )}
      </GlassPanel>

      {years.map((year) => (
        <div className="space-y-3" key={year}>
          <div className="flex items-center gap-3">
            <div className="font-black text-2xl text-glow text-gradient-blue">
              {year}
            </div>
            <div className="h-px flex-1 bg-gradient-to-r from-[#2484EC]/50 to-transparent" />
          </div>
          <div className="ml-3 space-y-2 border-[#2484EC]/30 border-l pl-2">
            {byYear.get(year)?.map((ev) => (
              <JourneyRow event={ev} key={ev.id} />
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

function JourneyRow({ event }: { event: JourneyEvent }) {
  return (
    <div className="relative flex animate-fade-up items-start gap-3 pl-4">
      <div
        className="absolute top-3 -left-[5px] size-2.5 rounded-full bg-[#2484EC]"
        style={{ boxShadow: "0 0 8px #2484ECCC" }}
      />
      <GlassPanel className="flex flex-1 items-center gap-3 p-3">
        <div
          className="flex size-10 shrink-0 items-center justify-center rounded-lg"
          style={{ background: "#2484EC22", border: "1px solid #2484EC55" }}
        >
          <HugeiconsIcon
            icon={resolveIcon(event.icon)}
            size={18}
            style={{ color: "#94BCE4" }}
          />
        </div>
        <div className="min-w-0 flex-1">
          <div className="truncate font-semibold text-sm">{event.title}</div>
          <div className="truncate text-muted-foreground text-xs">
            {event.description}
          </div>
        </div>
        <div className="shrink-0 text-[10px] text-muted-foreground tabular-nums">
          {new Date(event.timestamp * 1000).toLocaleDateString(undefined, {
            day: "numeric",
            month: "short",
          })}
        </div>
      </GlassPanel>
    </div>
  );
}
