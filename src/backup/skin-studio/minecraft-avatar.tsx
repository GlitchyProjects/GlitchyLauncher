import { useEffect, useRef, useState } from "react";

interface MinecraftAvatarProps {
  skinUrl?: string | null;
  username?: string;
  size?: number;
  className?: string;
  fallback?: "steve" | "alex" | "glitchy";
}

// In-memory cache for sliced head data URLs
const headCache = new Map<string, string>();

/**
 * Procedurally draws the iconic Steve face (8x8 pixels) onto canvas.
 * Works 100% offline with zero network requests and zero VPN.
 */
function drawProceduralSteve(ctx: CanvasRenderingContext2D, size: number) {
  const p = size / 8;
  const colors: Record<string, string> = {
    H: "#2B1B10", // Hair dark brown
    S: "#BC8668", // Skin base
    D: "#986246", // Nose / shadow
    W: "#FFFFFF", // Eye white
    E: "#384B9E", // Eye blue
    M: "#4D2817", // Mouth
  };

  const grid = [
    ["H", "H", "H", "H", "H", "H", "H", "H"],
    ["H", "H", "H", "H", "H", "H", "H", "H"],
    ["H", "H", "S", "S", "S", "S", "H", "H"],
    ["S", "S", "S", "S", "S", "S", "S", "S"],
    ["W", "E", "S", "D", "D", "S", "E", "W"],
    ["S", "S", "S", "D", "D", "S", "S", "S"],
    ["S", "S", "M", "M", "M", "M", "S", "S"],
    ["S", "S", "M", "M", "M", "M", "S", "S"],
  ];

  for (let y = 0; y < 8; y++) {
    for (let x = 0; x < 8; x++) {
      ctx.fillStyle = colors[grid[y][x]] || "#BC8668";
      ctx.fillRect(x * p, y * p, p, p);
    }
  }
}

/**
 * Procedurally draws the iconic Alex face (8x8 pixels) onto canvas.
 */
function drawProceduralAlex(ctx: CanvasRenderingContext2D, size: number) {
  const p = size / 8;
  const colors: Record<string, string> = {
    H: "#B85724", // Orange hair
    S: "#E5B08F", // Pale skin
    D: "#D19675", // Shadow
    W: "#FFFFFF", // Eye white
    E: "#357A43", // Green eye
    M: "#A05545", // Lips
  };

  const grid = [
    ["H", "H", "H", "H", "H", "H", "H", "H"],
    ["H", "H", "H", "H", "H", "H", "H", "H"],
    ["H", "S", "S", "S", "S", "S", "H", "H"],
    ["S", "S", "S", "S", "S", "S", "S", "H"],
    ["W", "E", "S", "D", "D", "S", "E", "W"],
    ["S", "S", "S", "D", "D", "S", "S", "S"],
    ["S", "S", "S", "M", "M", "S", "S", "S"],
    ["S", "S", "S", "S", "S", "S", "S", "S"],
  ];

  for (let y = 0; y < 8; y++) {
    for (let x = 0; x < 8; x++) {
      ctx.fillStyle = colors[grid[y][x]] || "#E5B08F";
      ctx.fillRect(x * p, y * p, p, p);
    }
  }
}

export function MinecraftAvatar({
  skinUrl,
  username = "Steve",
  size = 40,
  className = "",
  fallback = "steve",
}: MinecraftAvatarProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [dataUrl, setDataUrl] = useState<string | null>(null);

  const cacheKey = `${skinUrl || username}_${size}`;

  useEffect(() => {
    // Check in-memory cache first
    if (headCache.has(cacheKey)) {
      setDataUrl(headCache.get(cacheKey)!);
      return;
    }

    const canvas = canvasRef.current || document.createElement("canvas");
    canvas.width = size;
    canvas.height = size;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    ctx.imageSmoothingEnabled = false;

    // Helper to render procedural fallback
    const renderFallback = () => {
      // Pick Steve or Alex based on username hash if not specified
      let isAlex = fallback === "alex";
      if (fallback === "steve" && username) {
        let hash = 0;
        for (let i = 0; i < username.length; i++) {
          hash = (hash << 5) - hash + username.charCodeAt(i);
          hash |= 0;
        }
        isAlex = Math.abs(hash) % 2 === 1;
      }

      ctx.clearRect(0, 0, size, size);
      if (isAlex) {
        drawProceduralAlex(ctx, size);
      } else {
        drawProceduralSteve(ctx, size);
      }
      const url = canvas.toDataURL();
      headCache.set(cacheKey, url);
      setDataUrl(url);
    };

    let active = true;
    const urlToLoad =
      skinUrl ||
      (username && !["Steve", "Alex"].includes(username)
        ? `https://crafthead.net/skin/${encodeURIComponent(username)}`
        : null);

    if (!urlToLoad) {
      renderFallback();
      return;
    }

    // Slice head from skin texture
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.src = urlToLoad;

    img.onload = () => {
      if (!active) return;
      try {
        ctx.clearRect(0, 0, size, size);
        // 1. Draw base face layer from skin: rect [8, 8, 8, 8] -> [0, 0, size, size]
        ctx.drawImage(img, 8, 8, 8, 8, 0, 0, size, size);
        // 2. Draw overlay hat/hair layer from skin: rect [40, 8, 8, 8] -> [0, 0, size, size]
        ctx.drawImage(img, 40, 8, 8, 8, 0, 0, size, size);

        const url = canvas.toDataURL();
        headCache.set(cacheKey, url);
        setDataUrl(url);
      } catch {
        renderFallback();
      }
    };

    img.onerror = () => {
      if (!active) return;
      renderFallback();
    };

    return () => {
      active = false;
    };
  }, [skinUrl, username, size, cacheKey, fallback]);

  return (
    <div
      className={`relative inline-flex shrink-0 items-center justify-center overflow-hidden rounded-xl border border-border/40 bg-background/50 shadow-xs ${className}`}
      style={{ width: size, height: size }}
    >
      <canvas ref={canvasRef} className="hidden" width={size} height={size} />
      {dataUrl ? (
        <img
          alt={username}
          className="size-full object-cover [image-rendering:pixelated]"
          src={dataUrl}
        />
      ) : (
        <div className="size-full animate-pulse bg-muted/40" />
      )}
    </div>
  );
}
