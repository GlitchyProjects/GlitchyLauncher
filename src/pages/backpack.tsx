import { Package, Palette, Sparkles } from "lucide-react";
import { ModrinthBrowser } from "@/components/blocks/mods/modrinth-browser";
import { ModsHeader } from "@/components/blocks/mods/mods-header";
import { ModsList as ModsListComponent } from "@/components/blocks/mods/mods-list";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  BackpackProvider,
  useBackpackActions,
  useBackpackState,
} from "@/context/backpack-context";
import type { BackpackCategory } from "@/invokes";

const CATEGORIES: {
  category: BackpackCategory;
  icon: React.ReactNode;
  label: string;
}[] = [
  { category: "mods", icon: <Package className="size-4" />, label: "Mods" },
  {
    category: "resourcePacks",
    icon: <Palette className="size-4" />,
    label: "Resource Packs",
  },
  {
    category: "shaderPacks",
    icon: <Sparkles className="size-4" />,
    label: "Shader Packs",
  },
];

/**
 * Backpack — per-instance content manager with three isolated sections
 * (Mods / Resource Packs / Shader Packs). Every section reads from and
 * writes to the selected instance's own folder, so no file ever mixes
 * between versions.
 */
export default function Backpack() {
  return (
    <BackpackProvider>
      {/* Rendered inside the provider so the dialog can read the active
          category and the selected instance from the context. */}
      <BackpackPage />
      <ModrinthBrowser />
    </BackpackProvider>
  );
}

function BackpackPage() {
  const { activeCategory } = useBackpackState();
  const { setActiveCategory } = useBackpackActions();

  return (
    <div className="flex h-full flex-col space-y-4 p-2">
      <ModsHeader />

      <Tabs
        className="flex flex-1 flex-col overflow-hidden"
        onValueChange={(value) => setActiveCategory(value as BackpackCategory)}
        value={activeCategory}
      >
        <TabsList className="w-full shrink-0 justify-start overflow-x-auto">
          {CATEGORIES.map((c) => (
            <TabsTrigger
              className="flex-1 min-w-max whitespace-nowrap px-3 sm:px-5"
              key={c.category}
              value={c.category}
            >
              {c.icon}
              <span className="whitespace-nowrap">{c.label}</span>
            </TabsTrigger>
          ))}
        </TabsList>

        {CATEGORIES.map((c) => (
          <TabsContent
            className="flex-1 overflow-hidden"
            key={c.category}
            value={c.category}
          >
            <div className="scrollbar-thin scrollbar-thumb-muted-foreground/20 flex h-full flex-col overflow-hidden overflow-y-auto rounded-2xl border border-border/40 bg-secondary/20 p-4">
              <ModsListComponent category={c.category} />
            </div>
          </TabsContent>
        ))}
      </Tabs>
    </div>
  );
}
