import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { GlassPanel, SectionHeading } from "@/components/glitchy/primitives";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useBackend, useBackendMutation } from "@/hooks/use-backend";
import type {
  GlowStyle,
  ProfileBackground,
  ProfileCustomization,
  ProfileFrame,
} from "@/invokes";
import { PRESET_AVATARS, presetAvatarSrc } from "@/lib/avatars";

const FRAMES: { id: ProfileFrame; label: string }[] = [
  { id: "none", label: "None" },
  { id: "solid_blue", label: "Solid Blue" },
  { id: "gradient_electric", label: "Electric" },
  { id: "gradient_icy", label: "Icy" },
  { id: "neon_pulse", label: "Neon Pulse" },
];
const BACKGROUNDS: { id: ProfileBackground; label: string }[] = [
  { id: "aurora", label: "Aurora" },
  { id: "deep_sea", label: "Deep Sea" },
  { id: "grid", label: "Grid" },
  { id: "particles", label: "Particles" },
  { id: "solid", label: "Solid" },
];
const GLOWS: { id: GlowStyle; label: string }[] = [
  { id: "none", label: "None" },
  { id: "subtle", label: "Subtle" },
  { id: "normal", label: "Normal" },
  { id: "strong", label: "Strong" },
];

export function ProfileCustomizer({
  open,
  onOpenChange,
  current,
  onSaved,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  current: ProfileCustomization;
  onSaved: () => void;
}) {
  const { data: allowedAccents } = useBackend({
    name: "glitchy_get_allowed_accents",
  });
  const { mutateAsync: saveCustomization, isPending } = useBackendMutation({
    name: "glitchy_save_customization",
    onSuccess: () => {
      onSaved();
      onOpenChange(false);
    },
  });
  const { mutateAsync: uploadAvatar, isPending: uploading } =
    useBackendMutation({
      name: "glitchy_upload_avatar",
      onSuccess: (filename: string) =>
        toast.success("Avatar uploaded", { description: filename }),
    });

  const [draft, setDraft] = useState<ProfileCustomization>(current);
  const [avatarUrl, setAvatarUrl] = useState<string | null>(null);
  const { avatar } = draft;

  useEffect(() => {
    if (open) {
      setDraft(current);
    }
  }, [open, current]);

  useEffect(() => {
    if (avatar.kind === "preset") {
      setAvatarUrl(presetAvatarSrc(avatar.id));
    } else {
      invoke<string>("glitchy_avatar_data", { filename: avatar.filename })
        .then((dataUrl) => setAvatarUrl(dataUrl))
        .catch(() => setAvatarUrl(presetAvatarSrc("glitchy")));
    }
  }, [avatar]);

  const handleUploadAvatar = async () => {
    const selected = await openDialog({
      filters: [
        { extensions: ["png", "jpg", "jpeg", "webp", "gif"], name: "Images" },
      ],
      multiple: false,
    });
    if (typeof selected !== "string") {
      return;
    }
    const filename = await uploadAvatar({ sourcePath: selected });
    setDraft((d) => ({ ...d, avatar: { filename, kind: "custom" } }));
  };

  const handleSave = async () => {
    await saveCustomization({ customization: draft });
  };

  const accents = allowedAccents ?? [
    "#2484EC",
    "#247CD4",
    "#4C8CCC",
    "#94BCE4",
    "#B4D4F4",
    "#ACC4DC",
  ];
  const accentHex = draft.accent ?? "#2484EC";

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle className="text-gradient-blue">
            Customize Profile
          </DialogTitle>
          <DialogDescription>
            Personalize your Glitchy identity. Changes are saved to your local
            profile.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-5 py-2">
          <GlassPanel className="flex items-center gap-4 p-4" variant="strong">
            <div
              className="size-16 shrink-0 overflow-hidden rounded-full"
              style={{
                boxShadow: `0 0 0 3px ${accentHex}, 0 0 0 6px ${accentHex}33, 0 0 24px ${accentHex}55`,
              }}
            >
              {avatarUrl ? (
                <img
                  alt="preview"
                  className="size-full object-cover"
                  height={64}
                  src={avatarUrl}
                  width={64}
                />
              ) : (
                <div className="size-full bg-gradient-to-br from-[#2484EC] to-[#4C8CCC]" />
              )}
            </div>
            <div className="min-w-0 flex-1">
              <div className="truncate font-bold text-glow text-lg">
                Preview
              </div>
              <div className="text-muted-foreground text-xs">
                {draft.tagline?.trim() || "Minecraft Player"}
              </div>
            </div>
          </GlassPanel>

          <div className="space-y-2">
            <SectionHeading>Avatar</SectionHeading>
            <div className="flex flex-wrap gap-2.5">
              {PRESET_AVATARS.map((p) => {
                const av = draft.avatar;
                const active = av.kind === "preset" && av.id === p.id;
                return (
                  <button
                    className={`flex size-14 items-center justify-center overflow-hidden rounded-full transition-all ${
                      active
                        ? "scale-110 shadow-[0_0_16px_#2484ECaa] ring-2 ring-white/80"
                        : "opacity-70 ring-1 ring-white/15 hover:scale-105 hover:opacity-100"
                    }`}
                    key={p.id}
                    onClick={() =>
                      setDraft((d) => ({
                        ...d,
                        avatar: { id: p.id, kind: "preset" },
                      }))
                    }
                    title={p.label}
                    type="button"
                  >
                    <img
                      alt={p.label}
                      className="size-full object-cover"
                      draggable={false}
                      height={56}
                      src={p.src}
                      width={56}
                    />
                  </button>
                );
              })}
              <button
                aria-label="Choose a custom profile image"
                className={`flex size-14 flex-col items-center justify-center gap-1 rounded-full border border-white/25 border-dashed font-bold text-[9px] uppercase tracking-wide transition-all ${
                  draft.avatar.kind === "custom"
                    ? "scale-110 text-white shadow-[0_0_16px_#2484ECaa] ring-2 ring-white/80"
                    : "text-muted-foreground hover:scale-105 hover:border-white/50 hover:text-foreground"
                } disabled:opacity-50`}
                disabled={uploading}
                onClick={handleUploadAvatar}
                title="Upload your own image"
                type="button"
              >
                {draft.avatar.kind === "custom" && avatarUrl && !uploading ? (
                  <img
                    alt="Custom avatar"
                    className="size-full object-cover"
                    draggable={false}
                    height={56}
                    src={avatarUrl}
                    width={56}
                  />
                ) : (
                  <>
                    <span>{uploading ? "…" : "＋"}</span>
                    <span>Custom</span>
                  </>
                )}
              </button>
            </div>
            <div className="text-[10px] text-muted-foreground">
              {(() => {
                const av = draft.avatar;
                return av.kind === "preset"
                  ? (PRESET_AVATARS.find((a) => a.id === av.id)?.label ??
                      "Glitchy")
                  : "Custom upload";
              })()}
            </div>
          </div>

          <div className="space-y-2">
            <SectionHeading>Frame</SectionHeading>
            <div className="flex flex-wrap gap-2">
              {FRAMES.map((f) => (
                <PresetButton
                  active={draft.frame === f.id}
                  key={f.id}
                  onClick={() => setDraft((d) => ({ ...d, frame: f.id }))}
                >
                  {f.label}
                </PresetButton>
              ))}
            </div>
          </div>

          <div className="space-y-2">
            <SectionHeading>Background</SectionHeading>
            <div className="flex flex-wrap gap-2">
              {BACKGROUNDS.map((b) => (
                <PresetButton
                  active={draft.background === b.id}
                  key={b.id}
                  onClick={() => setDraft((d) => ({ ...d, background: b.id }))}
                >
                  {b.label}
                </PresetButton>
              ))}
            </div>
          </div>

          <div className="space-y-2">
            <SectionHeading>Accent Color</SectionHeading>
            <div className="flex flex-wrap gap-2">
              {accents.map((c) => (
                <button
                  className={`size-9 rounded-full transition-all ${accentHex === c ? "scale-110 ring-2 ring-white/70" : "hover:scale-105"}`}
                  key={c}
                  onClick={() => setDraft((d) => ({ ...d, accent: c }))}
                  style={{
                    background: c,
                    boxShadow: accentHex === c ? `0 0 16px ${c}` : "none",
                  }}
                  title={c}
                  type="button"
                />
              ))}
            </div>
          </div>

          <div className="space-y-2">
            <SectionHeading>Glow</SectionHeading>
            <div className="flex flex-wrap gap-2">
              {GLOWS.map((g) => (
                <PresetButton
                  active={draft.glow === g.id}
                  key={g.id}
                  onClick={() => setDraft((d) => ({ ...d, glow: g.id }))}
                >
                  {g.label}
                </PresetButton>
              ))}
            </div>
          </div>

          <div className="space-y-2">
            <SectionHeading>Tagline</SectionHeading>
            <Label className="sr-only" htmlFor="tagline">
              Tagline
            </Label>
            <Input
              className="border-white/10 bg-white/5"
              id="tagline"
              maxLength={80}
              onChange={(e) =>
                setDraft((d) => ({ ...d, tagline: e.target.value }))
              }
              placeholder="Minecraft Player"
              value={draft.tagline}
            />
            <div className="text-right text-[10px] text-muted-foreground">
              {(draft.tagline ?? "").length}/80
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button
            disabled={isPending}
            onClick={() => onOpenChange(false)}
            variant="outline"
          >
            Cancel
          </Button>
          <Button disabled={isPending} onClick={handleSave}>
            {isPending ? "Saving…" : "Save Changes"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function PresetButton({
  active,
  onClick,
  children,
  disabled,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
  disabled?: boolean;
}) {
  return (
    <button
      className={`rounded-lg px-3 py-1.5 font-medium text-xs transition-all ${
        active
          ? "border border-[#2484EC]/60 bg-[#2484EC]/20 text-white shadow-[0_0_12px_#2484EC55]"
          : "border border-white/10 bg-white/5 text-muted-foreground hover:bg-white/10 hover:text-foreground"
      } disabled:cursor-not-allowed disabled:opacity-50`}
      disabled={disabled}
      onClick={onClick}
      type="button"
    >
      {children}
    </button>
  );
}
