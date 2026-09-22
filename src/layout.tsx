import { QueryClientProvider } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Square, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Outlet } from "react-router";
import { AppSidebar } from "./components/app-sidebar";
import { DownloadsListener } from "./components/blocks/downloads/downloads-listener";
import { GlitchyUpdateListener } from "./components/glitchy/update-listener";
import { StartupUpdater } from "./components/blocks/updater/startup-updater";
import { AuthModal } from "./components/blocks/auth/auth-modal";
import { useAccountStore } from "./stores/account";
import { ThemeProvider } from "./components/theme-provider";
import { SidebarProvider } from "./components/ui/sidebar";
import { Toaster } from "./components/ui/sonner";
import { useBackend, useBackendMutation } from "./hooks/use-backend";
import { queryClient } from "./lib/query-client";

import { useLocale } from "./stores/locale";

/**
 * OuterLayout — installs all the React context providers (Theme, QueryClient,
 * Sidebar, Toaster). This component itself does NOT call any hook that
 * depends on those providers, so it's safe to use as the router's root
 * element.
 *
 * The providers wrap <InnerLayout />, which is allowed to call useBackend /
 * useBackendMutation because by then QueryClientProvider is already mounted.
 */
export default function Layout() {
  return (
    <ThemeProvider>
      <QueryClientProvider client={queryClient}>
        <InnerLayout />
        <Toaster />
      </QueryClientProvider>
    </ThemeProvider>
  );
}

/**
 * InnerLayout — the actual chrome (sidebar, titlebar buttons, content area).
 * All useBackend / useBackendMutation / Tauri window calls live here.
 */
function InnerLayout() {
  const { locale } = useLocale();
  const isRtl = locale === "fa";
  const tauriWindow = getCurrentWindow();
  const [isMaximized, setIsMaximized] = useState(false);

  const { data: backendMaximized } = useBackend({
    name: "glitchy_is_maximized",
  });
  const { mutateAsync: toggleMaximized } = useBackendMutation({
    name: "glitchy_toggle_maximized",
  });

  useEffect(() => {
    if (backendMaximized !== undefined) {
      setIsMaximized(backendMaximized);
    }
  }, [backendMaximized]);

  useEffect(() => {
    const unlisten = tauriWindow.onResized(() => {
      invoke<boolean>("glitchy_is_maximized")
        .then(setIsMaximized)
        .catch(() => undefined);
    });
    return () => {
      unlisten.then((stopListening) => stopListening());
    };
  }, [tauriWindow]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "F11") {
        e.preventDefault();
        toggleMaximized()
          .then(setIsMaximized)
          .catch(() => undefined);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggleMaximized]);

  useEffect(() => {
    useAccountStore.getState().fetchUser();
  }, []);

  const handleToggleMaximized = async () => {
    const next = await toggleMaximized();
    setIsMaximized(next);
  };

  return (
    <>
      <StartupUpdater />
      <AuthModal />
      <GlitchyUpdateListener />
      <DownloadsListener />
      <SidebarProvider className="bg-sidebar">
        <AppSidebar side={isRtl ? "right" : "left"} />
        <div className="flex h-screen w-full min-w-0 flex-col antialiased">
          {/* Native Desktop Titlebar */}
          <header
            className="flex h-9 shrink-0 select-none items-center justify-between px-2"
            data-tauri-drag-region
          >
            <div className="h-full flex-1" data-tauri-drag-region />
            <div className="flex h-full select-none items-stretch" dir="ltr">
              <button
                aria-label="Minimize"
                className="flex h-full w-11 items-center justify-center text-muted-foreground/70 transition-colors hover:bg-white/[0.08] hover:text-foreground active:bg-white/[0.14]"
                onClick={() => tauriWindow.minimize()}
                title="Minimize"
                type="button"
              >
                <Minus className="size-3.5" />
              </button>
              <button
                aria-label={
                  isMaximized ? "Restore window (F11)" : "Maximize window (F11)"
                }
                className="flex h-full w-11 items-center justify-center text-muted-foreground/70 transition-colors hover:bg-white/[0.08] hover:text-foreground active:bg-white/[0.14]"
                onClick={handleToggleMaximized}
                title={
                  isMaximized ? "Restore window (F11)" : "Maximize window (F11)"
                }
                type="button"
              >
                {isMaximized ? (
                  <Copy className="size-3 rotate-180 -scale-y-100" />
                ) : (
                  <Square className="size-3" />
                )}
              </button>
              <button
                aria-label="Close"
                className="flex h-full w-11 items-center justify-center text-muted-foreground/70 transition-colors hover:bg-[#e81123] hover:text-white active:bg-[#bf0f1d] active:text-white"
                onClick={() => tauriWindow.close()}
                title="Close"
                type="button"
              >
                <X className="size-4" />
              </button>
            </div>
          </header>
          <main className="min-h-0 flex-1 overflow-y-auto bg-background/70 p-4">
            <Outlet />
          </main>
        </div>
      </SidebarProvider>
    </>
  );
}
