import {
  Check,
  Download,
  FolderOpen,
  Search,
  ShieldCheck,
  Shirt,
  Sparkles,
  Upload,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  FlyingAnimation,
  IdleAnimation,
  RunningAnimation,
  SkinViewer,
  WalkingAnimation,
} from "skinview3d";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { resolveUsernameSkin } from "@/lib/skin-resolver";
import { useLocale } from "@/stores/locale";

// Standard default Steve and Alex skin data URIs or public CDN URLs
const DEFAULT_STEVE =
  "https://textures.minecraft.net/texture/1a2c964991448b11c97a8e7a83d719e76da7fe68eb7fbab1f22e865f121e7a5";
const DEFAULT_ALEX =
  "https://textures.minecraft.net/texture/41f84ea04f5b5f884240fb55d64821a8141443653155f949c811559ab9f73367";

// Famous preset capes (Official Mojang & Glitchy Exclusive)
const PRESET_CAPES = [
  {
    id: "none",
    name: "No Cape",
    url: null,
  },
  {
    id: "migrator",
    name: "Migrator Cape",
    url: "https://textures.minecraft.net/texture/2340c0e03dd66default57fbf0a9a4b3f88f1704618798e404b904ffab237ecff",
  },
  {
    id: "15th_anniversary",
    name: "15th Anniversary",
    url: "https://textures.minecraft.net/texture/b6a524ff01258d4d12c6a0ba9d57a957c5e27a9cfec1421f155c5df3ec7dfd8d",
  },
  {
    id: "cherry",
    name: "Cherry Blossom",
    url: "https://textures.minecraft.net/texture/406085a5a1f267fa56a64402eb4b72ef789b7b9f8d6bb5e504c538a7c293758",
  },
  {
    id: "glitchy_cyber",
    name: "Glitchy Neon Cape",
    url: "https://textures.minecraft.net/texture/e7dfea16dc83c97269e12f5d715425714d4453deffc8f38c353a29bb8889419b",
  },
];

const WARDROBE_STORAGE_KEY = "glitchy_skin_wardrobe_v1";

interface WardrobeItem {
  capeUrl: string | null;
  id: string;
  model: "default" | "slim";
  name: string;
  skinUrl: string;
}

export function SkinStudio({ currentUsername }: { currentUsername?: string }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const viewerRef = useRef<SkinViewer | null>(null);

  const { locale } = useLocale();
  const isFa = locale === "fa";

  const [modelType, setModelType] = useState<"default" | "slim">("default");
  const [skinUrl, setSkinUrl] = useState<string>(DEFAULT_STEVE);
  const [capeUrl, setCapeUrl] = useState<string | null>(null);
  const [backEquipment, setBackEquipment] = useState<"cape" | "elytra">("cape");
  const [animation, setAnimation] = useState<"idle" | "walk" | "run" | "fly">(
    "walk"
  );
  const [autoRotate, setAutoRotate] = useState<boolean>(true);

  const [layers, setLayers] = useState({
    hat: true,
    jacket: true,
    leftArm: true,
    leftLeg: true,
    rightArm: true,
    rightLeg: true,
  });

  const [usernameInput, setUsernameInput] = useState<string>("");
  const [isImporting, setIsImporting] = useState<boolean>(false);
  const [wardrobe, setWardrobe] = useState<WardrobeItem[]>([]);

  // Load wardrobe from localStorage and active skin from backend
  useEffect(() => {
    try {
      const saved = localStorage.getItem(WARDROBE_STORAGE_KEY);
      if (saved) {
        setWardrobe(JSON.parse(saved));
      }
    } catch {
      // ignore
    }

    invoke<{ skinUrlOrData: string; model: "default" | "slim"; capeUrl: string | null } | null>(
      "glitchy_get_active_skin"
    )
      .then((active) => {
        if (active?.skinUrlOrData) {
          setSkinUrl(active.skinUrlOrData);
          setModelType(active.model);
          if (active.capeUrl) setCapeUrl(active.capeUrl);
        }
      })
      .catch(() => {});
  }, []);

  // Initialize SkinViewer
  useEffect(() => {
    if (!canvasRef.current) {
      return;
    }

    const viewer = new SkinViewer({
      animation: new WalkingAnimation(),
      canvas: canvasRef.current,
      height: 480,
      width: 320,
    });

    viewer.fov = 65;
    viewer.zoom = 0.85;
    viewer.autoRotate = autoRotate;
    viewer.autoRotateSpeed = 0.8;
    viewerRef.current = viewer;

    // Load initial skin
    viewer.loadSkin(skinUrl, { model: modelType });

    return () => {
      viewer.dispose();
      viewerRef.current = null;
    };
  }, []);

  // Update Skin & Model
  useEffect(() => {
    if (!viewerRef.current) {
      return;
    }
    viewerRef.current.loadSkin(skinUrl, { model: modelType });
  }, [skinUrl, modelType]);

  // Update Cape & Elytra
  useEffect(() => {
    if (!viewerRef.current) {
      return;
    }
    if (capeUrl) {
      viewerRef.current.loadCape(capeUrl, { backEquipment });
    } else {
      viewerRef.current.resetCape();
    }
  }, [capeUrl, backEquipment]);

  // Update Animation
  useEffect(() => {
    if (!viewerRef.current) {
      return;
    }
    switch (animation) {
      case "idle":
        viewerRef.current.animation = new IdleAnimation();
        break;
      case "walk":
        viewerRef.current.animation = new WalkingAnimation();
        break;
      case "run":
        viewerRef.current.animation = new RunningAnimation();
        break;
      case "fly":
        viewerRef.current.animation = new FlyingAnimation();
        break;
    }
  }, [animation]);

  // Update Auto-Rotate
  useEffect(() => {
    if (!viewerRef.current) {
      return;
    }
    viewerRef.current.autoRotate = autoRotate;
  }, [autoRotate]);

  // Update Layers
  useEffect(() => {
    if (!viewerRef.current) {
      return;
    }
    const player = viewerRef.current.playerObject;
    if (!player) {
      return;
    }

    player.skin.head.outerLayer.visible = layers.hat;
    player.skin.body.outerLayer.visible = layers.jacket;
    player.skin.leftArm.outerLayer.visible = layers.leftArm;
    player.skin.rightArm.outerLayer.visible = layers.rightArm;
    player.skin.leftLeg.outerLayer.visible = layers.leftLeg;
    player.skin.rightLeg.outerLayer.visible = layers.rightLeg;
  }, [layers]);

  // File Upload Handler
  const handleFileUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) {
      return;
    }

    if (!file.type.includes("png")) {
      toast.error(
        isFa
          ? "فرمت فایل باید تصویر PNG باشد."
          : "Skin file must be a PNG image."
      );
      return;
    }

    const reader = new FileReader();
    reader.onload = (event) => {
      const dataUrl = event.target?.result as string;
      const img = new Image();
      img.onload = () => {
        if (
          (img.width === 64 && (img.height === 64 || img.height === 32)) ||
          (img.width === 128 && img.height === 128)
        ) {
          setSkinUrl(dataUrl);
          invoke("glitchy_set_active_skin", {
            skinData: dataUrl,
            model: modelType,
            capeUrl: capeUrl,
          }).catch(() => {});
          toast.success(
            isFa ? "اسکین جدید با موفقیت لود و ذخیره شد." : "Skin loaded and saved."
          );
        } else {
          toast.error(
            isFa
              ? "ابعاد اسکین نامعتبر است (باید 64x64 یا 64x32 باشد)."
              : "Invalid skin dimensions (must be 64x64 or 64x32)."
          );
        }
      };
      img.src = dataUrl;
    };
    reader.readAsDataURL(file);
  };

  // Cape File Upload Handler
  const handleCapeUpload = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) {
      return;
    }

    const reader = new FileReader();
    reader.onload = (event) => {
      const dataUrl = event.target?.result as string;
      setCapeUrl(dataUrl);
      invoke("glitchy_set_active_skin", {
        skinData: skinUrl,
        model: modelType,
        capeUrl: dataUrl,
      }).catch(() => {});
      toast.success(
        isFa ? "شنل با موفقیت آپلود و ذخیره شد." : "Cape uploaded and saved."
      );
    };
    reader.readAsDataURL(file);
  };

  // Import Skin from Minecraft Username
  const handleImportUsername = async () => {
    const username = usernameInput.trim();
    if (!username) {
      return;
    }

    setIsImporting(true);
    try {
      // Use multi-CDN cascade (Crafthead/Cloudflare, PlayerDB, Ashcon, Cravatar)
      const resolvedSkin = await resolveUsernameSkin(username);
      setSkinUrl(resolvedSkin);
      invoke("glitchy_set_active_skin", {
        capeUrl: capeUrl,
        model: modelType,
        skinData: resolvedSkin,
      }).catch(() => {});
      toast.success(
        isFa
          ? `اسکین ${username} با موفقیت دریافت و ذخیره شد.`
          : `Skin for ${username} imported and saved.`
      );
    } catch {
      toast.error(
        isFa
          ? `اسکینی برای بازیکن ${username} یافت نشد.`
          : `Could not find a skin for ${username}.`
      );
    } finally {
      setIsImporting(false);
    }
  };

  // Save Current Outfit to Wardrobe
  const handleSaveToWardrobe = () => {
    const newItem: WardrobeItem = {
      capeUrl,
      id: Date.now().toString(),
      model: modelType,
      name: `Outfit #${wardrobe.length + 1}`,
      skinUrl,
    };
    const updated = [newItem, ...wardrobe.slice(0, 9)];
    setWardrobe(updated);
    try {
      localStorage.setItem(WARDROBE_STORAGE_KEY, JSON.stringify(updated));
      toast.success(
        isFa ? "در کمد اسکین‌ها ذخیره شد." : "Saved to skin wardrobe."
      );
    } catch {
      // ignore
    }
  };

  // Export / Download Skin
  const handleDownloadSkin = () => {
    const link = document.createElement("a");
    link.download = `${currentUsername || "glitchy"}-skin.png`;
    link.href = skinUrl;
    link.click();
  };

  // Apply Changes to In-Game Multiplayer
  const handleApplyMultiplayer = async () => {
    try {
      await invoke("glitchy_set_active_skin", {
        skinData: skinUrl,
        model: modelType,
        capeUrl: capeUrl,
      });
      localStorage.setItem("glitchy_active_skin_url", skinUrl);
      localStorage.setItem("glitchy_active_skin_model", modelType);
      if (capeUrl) {
        localStorage.setItem("glitchy_active_cape_url", capeUrl);
      } else {
        localStorage.removeItem("glitchy_active_cape_url");
      }
      toast.success(
        isFa
          ? "اسکین و شنل با موفقیت برای بازی و سرورها تنظیم شد!"
          : "Skin & Cape configured and applied for in-game play!"
      );
    } catch (err) {
      toast.error(
        isFa
          ? `خطا در ذخیره اسکین: ${String(err)}`
          : `Failed to apply skin: ${String(err)}`
      );
    }
  };

  return (
    <div className="grid h-full min-h-0 gap-6 lg:grid-cols-[340px_minmax(0,1fr)]">
      {/* Left Column: 3D Interactive Canvas */}
      <div className="flex flex-col items-center justify-between rounded-2xl border border-border/50 bg-secondary/15 p-5 backdrop-blur-md">
        <div className="flex w-full items-center justify-between">
          <div className="flex items-center gap-2">
            <Sparkles className="size-4 animate-pulse text-primary" />
            <span className="font-bold text-muted-foreground text-xs uppercase tracking-wider">
              3D Live Preview
            </span>
          </div>
          <button
            className={`rounded-full border px-2 py-0.5 text-xs transition-all ${
              autoRotate
                ? "border-primary/50 bg-primary/20 font-semibold text-primary"
                : "border-border/40 text-muted-foreground"
            }`}
            onClick={() => setAutoRotate(!autoRotate)}
            type="button"
          >
            {autoRotate ? "Auto-Rotate: ON" : "Rotate: OFF"}
          </button>
        </div>

        {/* The 3D Canvas */}
        <div className="relative my-2 flex items-center justify-center">
          <canvas
            className="cursor-grab rounded-xl active:cursor-grabbing"
            ref={canvasRef}
          />
        </div>

        {/* Animation Quick Bar */}
        <div className="flex w-full items-center justify-center gap-1.5 rounded-xl border border-border/40 bg-background/50 p-1.5">
          {(["idle", "walk", "run", "fly"] as const).map((anim) => (
            <button
              className={`flex-1 rounded-lg py-1 font-medium text-xs capitalize transition-all ${
                animation === anim
                  ? "bg-primary text-primary-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground"
              }`}
              key={anim}
              onClick={() => setAnimation(anim)}
              type="button"
            >
              {anim}
            </button>
          ))}
        </div>

        {/* Big Apply to Multiplayer Button */}
        <Button
          className="mt-4 w-full gap-2 font-bold shadow-lg shadow-primary/20"
          onClick={handleApplyMultiplayer}
          size="lg"
        >
          <ShieldCheck className="size-5" />
          <span>
            {isFa
              ? "اعمال اسکین برای بازی چندنفره"
              : "Apply Skin for Multiplayer"}
          </span>
        </Button>
      </div>

      {/* Right Column: Customization Controls */}
      <div className="scrollbar-thin flex flex-col gap-5 overflow-y-auto pr-1">
        {/* Model Type & Quick Imports */}
        <section className="space-y-4 rounded-2xl border border-border/50 bg-secondary/15 p-5">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="font-bold text-foreground text-sm">
                {isFa ? "مدل دست‌ها و بارگذاری" : "Model Style & Import"}
              </h3>
              <p className="text-muted-foreground text-xs">
                {isFa
                  ? "نوع بازوها (استیو ۴ پیکسلی یا الکس ۳ پیکسلی) را انتخاب کنید."
                  : "Choose between Classic 4px (Steve) and Slim 3px (Alex) arm models."}
              </p>
            </div>
            {/* Classic / Slim Toggle */}
            <div className="flex rounded-xl border border-border/60 bg-background p-1">
              <button
                className={`rounded-lg px-3 py-1 font-semibold text-xs transition-all ${
                  modelType === "default"
                    ? "bg-primary text-primary-foreground shadow-xs"
                    : "text-muted-foreground hover:text-foreground"
                }`}
                onClick={() => setModelType("default")}
                type="button"
              >
                Classic (4px)
              </button>
              <button
                className={`rounded-lg px-3 py-1 font-semibold text-xs transition-all ${
                  modelType === "slim"
                    ? "bg-primary text-primary-foreground shadow-xs"
                    : "text-muted-foreground hover:text-foreground"
                }`}
                onClick={() => setModelType("slim")}
                type="button"
              >
                Slim (3px)
              </button>
            </div>
          </div>

          {/* Import / Upload Controls Grid */}
          <div className="grid gap-3 sm:grid-cols-2">
            {/* Upload File */}
            <div className="flex flex-col gap-2 rounded-xl border border-border/40 bg-background/40 p-3">
              <Label className="font-semibold text-xs">
                {isFa ? "آپلود فایل اسکین" : "Upload Custom Skin"}
              </Label>
              <label className="flex cursor-pointer items-center justify-center gap-2 rounded-lg border border-primary/40 border-dashed bg-primary/5 px-3 py-2 font-medium text-primary text-xs transition-all hover:bg-primary/10">
                <Upload className="size-4" />
                <span>{isFa ? "انتخاب فایل PNG" : "Choose PNG File"}</span>
                <input
                  accept="image/png"
                  className="hidden"
                  onChange={handleFileUpload}
                  type="file"
                />
              </label>
            </div>

            {/* Steal / Import from Minecraft Username */}
            <div className="flex flex-col gap-2 rounded-xl border border-border/40 bg-background/40 p-3">
              <Label className="font-semibold text-xs">
                {isFa ? "دریافت از نام کاربری" : "Steal from Username"}
              </Label>
              <div className="flex gap-2">
                <Input
                  className="h-8 text-xs"
                  onChange={(e) => setUsernameInput(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && handleImportUsername()}
                  placeholder={
                    isFa ? "مثلاً Dream یا Notch" : "e.g. Dream or Notch"
                  }
                  value={usernameInput}
                />
                <Button
                  className="h-8 gap-1 px-3 text-xs"
                  disabled={isImporting || !usernameInput.trim()}
                  onClick={handleImportUsername}
                  size="sm"
                  variant="secondary"
                >
                  <Search className="size-3.5" />
                  <span>{isImporting ? "..." : isFa ? "یافتن" : "Fetch"}</span>
                </Button>
              </div>
            </div>
          </div>
        </section>

        {/* Capes & Wings Section */}
        <section className="space-y-4 rounded-2xl border border-border/50 bg-secondary/15 p-5">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="font-bold text-foreground text-sm">
                {isFa ? "شنل و بال‌های الیترا" : "Capes & Elytra Wings"}
              </h3>
              <p className="text-muted-foreground text-xs">
                {isFa
                  ? "یکی از شنل‌های رسمی یا شنل نئونی اختصاصی گلچی را انتخاب کنید."
                  : "Select from official Mojang capes or exclusive Glitchy designs."}
              </p>
            </div>
            {/* Cape vs Elytra Wings Mode Toggle */}
            <button
              className={`rounded-lg border px-3 py-1 font-semibold text-xs transition-all ${
                backEquipment === "elytra"
                  ? "border-primary bg-primary/20 text-primary"
                  : "border-border/60 bg-background text-muted-foreground"
              }`}
              onClick={() =>
                setBackEquipment(backEquipment === "cape" ? "elytra" : "cape")
              }
              type="button"
            >
              {backEquipment === "elytra" ? "Mode: Elytra Wings" : "Mode: Cape"}
            </button>
          </div>

          <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 md:grid-cols-5">
            {PRESET_CAPES.map((cape) => {
              const isSelected = capeUrl === cape.url;
              return (
                <button
                  className={`flex flex-col items-center justify-center gap-2 rounded-xl border p-3 text-center transition-all ${
                    isSelected
                      ? "border-primary bg-primary/10 shadow-xs"
                      : "border-border/40 bg-background/40 hover:border-border hover:bg-background/70"
                  }`}
                  key={cape.id}
                  onClick={() => setCapeUrl(cape.url)}
                  type="button"
                >
                  <Shirt
                    className={`size-6 ${
                      isSelected ? "text-primary" : "text-muted-foreground"
                    }`}
                  />
                  <span className="truncate font-semibold text-[11px]">
                    {cape.name}
                  </span>
                  {isSelected && (
                    <Check className="size-3 shrink-0 text-primary" />
                  )}
                </button>
              );
            })}
          </div>

          <div className="flex items-center justify-between pt-1">
            <label className="flex cursor-pointer items-center gap-2 text-primary text-xs underline hover:text-primary/80">
              <Upload className="size-3.5" />
              <span>
                {isFa ? "آپلود شنل کاستوم" : "Upload Custom Cape PNG"}
              </span>
              <input
                accept="image/png"
                className="hidden"
                onChange={handleCapeUpload}
                type="file"
              />
            </label>
            {capeUrl && (
              <button
                className="text-destructive text-xs hover:underline"
                onClick={() => setCapeUrl(null)}
                type="button"
              >
                {isFa ? "حذف شنل" : "Remove Cape"}
              </button>
            )}
          </div>
        </section>

        {/* Skin Layers Visibility */}
        <section className="space-y-3 rounded-2xl border border-border/50 bg-secondary/15 p-5">
          <h3 className="font-bold text-foreground text-sm">
            {isFa ? "لایه‌های لباس و کلاه" : "Outer Clothing Layers"}
          </h3>
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
            {[
              ["hat", isFa ? "کلاه / سر" : "Hat / Head"],
              ["jacket", isFa ? "ژاکت / تنه" : "Jacket / Torso"],
              ["leftArm", isFa ? "دست چپ" : "Left Sleeve"],
              ["rightArm", isFa ? "دست راست" : "Right Sleeve"],
              ["leftLeg", isFa ? "پای چپ" : "Left Pants"],
              ["rightLeg", isFa ? "پای راست" : "Right Pants"],
            ].map(([layerKey, label]) => {
              const active = layers[layerKey as keyof typeof layers];
              return (
                <button
                  className={`flex items-center justify-between rounded-xl border p-2.5 font-medium text-xs transition-all ${
                    active
                      ? "border-primary/40 bg-primary/10 text-foreground"
                      : "border-border/40 bg-background/30 text-muted-foreground line-through opacity-60"
                  }`}
                  key={layerKey}
                  onClick={() =>
                    setLayers((prev) => ({
                      ...prev,
                      [layerKey]: !prev[layerKey as keyof typeof layers],
                    }))
                  }
                  type="button"
                >
                  <span>{label}</span>
                  <div
                    className={`size-2 rounded-full ${
                      active ? "bg-primary" : "bg-muted-foreground/40"
                    }`}
                  />
                </button>
              );
            })}
          </div>
        </section>

        {/* Skin Wardrobe (History / Favorites) */}
        <section className="space-y-3 rounded-2xl border border-border/50 bg-secondary/15 p-5">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="font-bold text-foreground text-sm">
                {isFa ? "کمد اسکین‌ها (Wardrobe)" : "Skin Wardrobe"}
              </h3>
              <p className="text-muted-foreground text-xs">
                {isFa
                  ? "اسکین‌های دلخواه خود را ذخیره کنید تا با ۱ کلیک تغییر دهید."
                  : "Save your favorite skins for 1-click swapping anytime."}
              </p>
            </div>
            <div className="flex gap-2">
              <Button
                className="h-8 gap-1.5 text-xs"
                onClick={handleSaveToWardrobe}
                size="sm"
                variant="outline"
              >
                <FolderOpen className="size-3.5" />
                <span>{isFa ? "ذخیره در کمد" : "Save Outfit"}</span>
              </Button>
              <Button
                className="h-8 gap-1.5 text-xs"
                onClick={handleDownloadSkin}
                size="sm"
                variant="outline"
              >
                <Download className="size-3.5" />
                <span>{isFa ? "دانلود فایل" : "Export PNG"}</span>
              </Button>
            </div>
          </div>

          {wardrobe.length > 0 ? (
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
              {wardrobe.map((item) => (
                <button
                  className="flex items-center gap-2 rounded-xl border border-border/40 bg-background/40 p-2 text-start transition-all hover:border-primary/50"
                  key={item.id}
                  onClick={() => {
                    setSkinUrl(item.skinUrl);
                    setModelType(item.model);
                    setCapeUrl(item.capeUrl);
                    toast.success(
                      isFa ? "اسکین بارگذاری شد." : "Outfit applied."
                    );
                  }}
                  type="button"
                >
                  <img
                    alt={item.name}
                    className="size-8 rounded-lg border border-border/40 object-cover"
                    src={item.skinUrl}
                  />
                  <div className="min-w-0 flex-1">
                    <p className="truncate font-semibold text-xs">
                      {item.name}
                    </p>
                    <p className="text-[10px] text-muted-foreground uppercase">
                      {item.model}
                    </p>
                  </div>
                </button>
              ))}
            </div>
          ) : (
            <p className="py-3 text-center text-muted-foreground text-xs">
              {isFa
                ? "هنوز اسکینی در کمد ذخیره نکرده‌اید."
                : "No saved outfits yet. Click 'Save Outfit' to keep favorites here."}
            </p>
          )}
        </section>
      </div>
    </div>
  );
}
