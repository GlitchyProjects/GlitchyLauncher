/**
 * Multi-CDN Minecraft Skin Resolver
 * Bypasses Iranian ISP blocks on minotar.net by cascading across
 * Cloudflare-based and international mirrors with automatic fallback.
 */

// Known fast endpoints for resolving skins that work in Iran
const SKIN_RESOLVERS = [
  // 1. Crafthead (Cloudflare Workers powered, excellent accessibility in Iran)
  (username: string) => `https://crafthead.net/skin/${username}`,
  // 2. PlayerDB (Global CDN, very high uptime)
  async (username: string) => {
    const res = await fetch(`https://playerdb.co/api/player/minecraft/${username}`, {
      signal: AbortSignal.timeout(4000),
    });
    if (!res.ok) throw new Error("PlayerDB failed");
    const json = await res.json();
    const skinUrl = json?.data?.player?.skin_texture;
    if (!skinUrl) throw new Error("No skin texture found in PlayerDB");
    return skinUrl as string;
  },
  // 3. Ashcon Mojang API
  async (username: string) => {
    const res = await fetch(`https://api.ashcon.app/mojang/v2/user/${username}`, {
      signal: AbortSignal.timeout(4000),
    });
    if (!res.ok) throw new Error("Ashcon failed");
    const json = await res.json();
    const skinUrl = json?.textures?.skin?.url;
    if (!skinUrl) throw new Error("No skin texture in Ashcon");
    return skinUrl as string;
  },
  // 4. Cravatar EU
  (username: string) => `https://cravatar.eu/skin/${username}`,
];

/**
 * Resolves a Minecraft username to an image-loadable skin URL.
 * Automatically attempts multiple independent mirrors if one times out or is blocked.
 */
export async function resolveUsernameSkin(username: string): Promise<string> {
  const cleanUser = username.trim();
  if (!cleanUser) {
    throw new Error("Username cannot be empty");
  }

  for (const resolver of SKIN_RESOLVERS) {
    try {
      let resolvedUrl: string;
      if (typeof resolver === "function" && resolver.length === 1 && resolver.constructor.name !== "AsyncFunction") {
        resolvedUrl = (resolver as (u: string) => string)(cleanUser);
      } else {
        resolvedUrl = await (resolver as (u: string) => Promise<string>)(cleanUser);
      }

      // Verify the image can actually be loaded (with a 4-second timeout)
      await new Promise<void>((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error("Image timeout")), 4000);
        const img = new Image();
        img.crossOrigin = "anonymous";
        img.onload = () => {
          clearTimeout(timer);
          resolve();
        };
        img.onerror = () => {
          clearTimeout(timer);
          reject(new Error("Image load error"));
        };
        img.src = resolvedUrl;
      });

      return resolvedUrl;
    } catch {
      // Continue to next resolver in cascade
    }
  }

  throw new Error(`Could not resolve skin for ${cleanUser} across mirrors.`);
}
