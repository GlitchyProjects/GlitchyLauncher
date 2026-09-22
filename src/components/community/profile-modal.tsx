import { Clock, Gamepad2, Play, ShieldCheck, X } from "lucide-react";
import { useEffect, useRef } from "react";
import { IdleAnimation, SkinViewer } from "skinview3d";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { resolveUsernameSkin } from "@/lib/skin-resolver";
import { useLocale } from "@/stores/locale";

export interface CommunityUser {
  avatarUrl?: string;
  badges?: string[];
  bio?: string;
  currentServerIp?: string | null;
  currentServerName?: string | null;
  currentServerPort?: number | null;
  gameVersion?: string | null;
  id: string;
  memberSince?: string;
  model?: "default" | "slim";
  skinUrl?: string;
  status: "online" | "in_game" | "offline";
  username: string;
}

interface ProfileModalProps {
  onClose: () => void;
  onJoinServer?: (serverIp: string, port?: number) => void;
  user: CommunityUser | null;
}

function getStatusBadgeColor(status: CommunityUser["status"]) {
  if (status === "in_game") {
    return "animate-pulse bg-emerald-500 shadow-emerald-500/50 shadow-sm";
  }
  if (status === "online") {
    return "bg-primary shadow-primary/50 shadow-sm";
  }
  return "bg-muted-foreground/40";
}

function getStatusLabel(
  user: CommunityUser,
  isPlaying: boolean | string | null | undefined,
  isFa: boolean
) {
  if (user.status === "in_game") {
    if (isPlaying) {
      const server = user.currentServerName || user.currentServerIp;
      return isFa ? `در حال بازی در ${server}` : `Playing on ${server}`;
    }
    return isFa ? "در بازی ماینکرفت" : "Playing Minecraft";
  }
  if (user.status === "online") {
    return isFa ? "آنلاین در لانچر" : "Online in Launcher";
  }
  return isFa ? "آفلاین" : "Offline";
}

export function UserProfileModal({
  user,
  onClose,
  onJoinServer,
}: ProfileModalProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const viewerRef = useRef<SkinViewer | null>(null);
  const { locale } = useLocale();
  const isFa = locale === "fa";

  useEffect(() => {
    if (!(user && canvasRef.current)) {
      return;
    }

    let active = true;
    const viewer = new SkinViewer({
      animation: new IdleAnimation(),
      canvas: canvasRef.current,
      height: 240,
      width: 160,
    });

    viewer.fov = 60;
    viewer.zoom = 0.9;
    viewer.autoRotate = true;
    viewer.autoRotateSpeed = 0.7;
    viewerRef.current = viewer;

    async function loadSkinTexture() {
      let skinToLoad = user?.skinUrl;
      if (!skinToLoad && user?.username) {
        try {
          skinToLoad = await resolveUsernameSkin(user.username);
        } catch {
          skinToLoad = `https://crafthead.net/skin/${encodeURIComponent(user.username)}`;
        }
      }
      if (!skinToLoad) {
        skinToLoad = "https://crafthead.net/skin/Steve";
      }

      if (active && viewerRef.current) {
        try {
          await viewerRef.current.loadSkin(skinToLoad, {
            model: user?.model || "default",
          });
        } catch {
          // If network failed, attempt fallback
          if (active && viewerRef.current) {
            viewerRef.current
              .loadSkin("https://crafthead.net/skin/Steve", {
                model: "default",
              })
              .catch(() => {});
          }
        }
      }
    }

    loadSkinTexture();

    return () => {
      active = false;
      viewer.dispose();
      viewerRef.current = null;
    };
  }, [user]);

  if (!user) {
    return null;
  }

  const isPlaying = Boolean(user.status === "in_game" && user.currentServerIp);

  return (
    <div className="fixed inset-0 z-50 flex animate-fade-in items-center justify-center bg-black/60 p-4 backdrop-blur-xs">
      <div className="relative w-full max-w-lg animate-scale-up overflow-hidden rounded-3xl border border-border/60 bg-secondary/30 p-6 shadow-2xl backdrop-blur-xl">
        {/* Close Button */}
        <button
          className="absolute top-4 right-4 rounded-full border border-border/40 bg-background/50 p-1.5 text-muted-foreground transition-colors hover:bg-background hover:text-foreground"
          onClick={onClose}
          type="button"
        >
          <X className="size-4" />
        </button>

        <div className="flex flex-col items-center gap-6 sm:flex-row sm:items-start">
          {/* 3D Skin Avatar Canvas */}
          <div className="flex flex-col items-center justify-center rounded-2xl border border-border/40 bg-background/40 p-2 shadow-inner">
            <canvas className="cursor-grab rounded-xl" ref={canvasRef} />
            <span className="mt-1 font-bold text-[10px] text-muted-foreground uppercase tracking-wider">
              {user.model === "slim" ? "Slim 3px" : "Classic 4px"}
            </span>
          </div>

          {/* User Info Details */}
          <div className="w-full min-w-0 flex-1 space-y-3">
            <div>
              <div className="flex items-center gap-2">
                <h2 className="truncate font-black text-2xl text-foreground">
                  {user.username}
                </h2>
                {user.badges && user.badges.length > 0 && (
                  <span className="rounded-md bg-primary/20 px-2 py-0.5 font-bold text-[10px] text-primary">
                    {user.badges[0]}
                  </span>
                )}
              </div>
              <p className="mt-1 line-clamp-2 text-muted-foreground text-xs italic">
                {user.bio || "No status set."}
              </p>
            </div>

            {/* Status Indicator Pill */}
            <div className="flex items-center gap-2">
              <div className={`size-2.5 rounded-full ${getStatusBadgeColor(user.status)}`} />
              <span className="font-semibold text-foreground text-xs">
                {getStatusLabel(user, isPlaying, isFa)}
              </span>
            </div>

            {/* Live Playing Server Box & Quick Join Button */}
            {isPlaying && (
              <div className="space-y-2 rounded-2xl border border-emerald-500/30 bg-emerald-500/10 p-3.5">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-1.5 font-bold text-emerald-400 text-xs">
                    <Gamepad2 className="size-4" />
                    <span>{user.currentServerName || "Minecraft Server"}</span>
                  </div>
                  <span className="font-mono text-[11px] text-muted-foreground">
                    {user.currentServerIp}
                  </span>
                </div>

                {/* The Magic "Join Server" Button */}
                <Button
                  className="w-full gap-2 bg-emerald-600 font-black text-white text-xs shadow-emerald-900/30 shadow-lg hover:bg-emerald-500"
                  onClick={() => {
                    if (onJoinServer && user.currentServerIp) {
                      onJoinServer(
                        user.currentServerIp,
                        user.currentServerPort ?? 25_565
                      );
                    } else {
                      toast.info(
                        isFa
                          ? `در حال ورود به ${user.currentServerIp}...`
                          : `Connecting directly to ${user.currentServerIp}...`
                      );
                    }
                  }}
                  size="sm"
                >
                  <Play className="size-3.5" />
                  <span>
                    {isFa ? "ورود به سرور دوست" : "Join Game Server (Direct)"}
                  </span>
                </Button>
              </div>
            )}

            {/* Quick Details */}
            <div className="grid grid-cols-2 gap-2 pt-1 text-[11px] text-muted-foreground">
              <div className="flex items-center gap-1.5">
                <Clock className="size-3.5 text-primary" />
                <span>Joined {user.memberSince || "Recently"}</span>
              </div>
              <div className="flex items-center gap-1.5">
                <ShieldCheck className="size-3.5 text-primary" />
                <span>Glitchy Player</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
