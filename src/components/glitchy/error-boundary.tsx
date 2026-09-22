import { useEffect } from "react";
import { useNavigate, useRouteError } from "react-router";
import { GlowButton } from "@/components/glitchy/primitives";

/**
 * Router-level error boundary. If any route throws during render, this
 * component catches it and shows a readable error UI with retry + go-home
 * buttons instead of a blank screen.
 *
 * This is a SAFETY NET — it does NOT substitute for fixing the underlying
 * crash. Every error caught here should also be root-caused and fixed
 * at the source.
 */
export function RouteErrorBoundary() {
  const error = useRouteError();
  const navigate = useNavigate();

  useEffect(() => {
    // Log to console for debugging.
    console.error("[Glitchy] route error:", error);
  }, [error]);

  const message =
    error instanceof Error
      ? error.message
      : typeof error === "string"
        ? error
        : "An unexpected error occurred while rendering this page.";

  return (
    <div className="flex h-full items-center justify-center p-8">
      <div className="glass-strong w-full max-w-md space-y-4 rounded-2xl p-8 text-center">
        <div className="text-5xl">⚠️</div>
        <h2 className="font-bold text-glow text-xl">Something went wrong</h2>
        <p className="break-words text-muted-foreground text-sm">{message}</p>
        <div className="flex justify-center gap-2 pt-2">
          <GlowButton
            className="px-4 py-2 text-xs"
            onClick={() => navigate("/")}
          >
            Go Home
          </GlowButton>
          <button
            className="rounded-xl border border-white/10 bg-white/5 px-4 py-2 font-medium text-muted-foreground text-xs transition-all hover:bg-white/10"
            onClick={() => window.location.reload()}
            type="button"
          >
            Reload
          </button>
        </div>
      </div>
    </div>
  );
}
