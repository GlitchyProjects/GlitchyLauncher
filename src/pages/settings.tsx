import {
  ConsoleIcon,
  Download02Icon,
  GameController01Icon,
  Settings01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { ConsoleFilters } from "@/components/blocks/console/console-filters";
import { ConsoleViewer } from "@/components/blocks/console/console-viewer";
import { GameOptions } from "@/components/blocks/settings/game-options";
import { LauncherSettings } from "@/components/blocks/settings/launcher-settings";
import { MirrorSettings } from "@/components/blocks/settings/mirror-settings";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ConsoleProvider } from "@/context/console-context";

export default function Settings() {
  return (
    <Tabs className="flex h-full flex-col gap-3" defaultValue="launcher">
      {/* Horizontal Navigation Header Layout */}
      <TabsList className="w-full bg-secondary/40 backdrop-blur-md">
        {[
          { icon: Settings01Icon, id: "launcher", label: "Launcher Settings" },
          { icon: GameController01Icon, id: "game", label: "Game Options" },
          { icon: Download02Icon, id: "mirror", label: "Mirrors" },
          { icon: ConsoleIcon, id: "console", label: "Console" },
        ].map((tab) => (
          <TabsTrigger className="flex-1 gap-2 px-4" key={tab.id} value={tab.id}>
            <HugeiconsIcon icon={tab.icon} size={16} strokeWidth={2} />
            <span className="font-medium text-xs">{tab.label}</span>
          </TabsTrigger>
        ))}
      </TabsList>

      {/* Config Panels Panel Box */}
      <div className="min-h-0 flex-1 overflow-y-auto rounded-2xl border border-border/60 bg-background/40 p-6">
        <TabsContent value="launcher">
          <LauncherSettings />
        </TabsContent>

        <TabsContent value="game">
          <GameOptions />
        </TabsContent>

        <TabsContent value="mirror">
          <MirrorSettings />
        </TabsContent>

        <TabsContent className="h-full" value="console">
          <ConsoleProvider>
            <div className="flex h-[calc(100vh-14rem)] w-full min-w-0 flex-col overflow-hidden rounded-xl border border-border/40 bg-background">
              <ConsoleFilters />
              <ConsoleViewer />
            </div>
          </ConsoleProvider>
        </TabsContent>
      </div>
    </Tabs>
  );
}
