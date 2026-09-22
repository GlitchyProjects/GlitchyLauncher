import { Backpack01Icon, Package01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Boxes } from "lucide-react";
import { useState } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import Backpack from "@/pages/backpack";
import InstanceManager from "@/pages/instance-manager";
import Modpacks from "@/pages/modpacks";

export default function Library() {
  const [activeTab, setActiveTab] = useState<
    "instances" | "modpacks" | "backpack"
  >("instances");

  return (
    <div className="flex h-full flex-col gap-3">
      {/* Top Glassmorphism Pill Sub-Tabs */}
      <Tabs
        className="flex h-full flex-1 flex-col overflow-hidden"
        onValueChange={(val) =>
          setActiveTab(val as "instances" | "modpacks" | "backpack")
        }
        value={activeTab}
      >
        <div className="flex w-full shrink-0 items-center">
          <TabsList className="w-full bg-secondary/40 backdrop-blur-md">
            <TabsTrigger
              className="flex-1 gap-2 px-4 font-semibold text-xs"
              value="instances"
            >
              <Boxes
                className={`size-4 transition-colors ${
                  activeTab === "instances" ? "text-white" : "text-primary"
                }`}
              />
              <span>Instances</span>
            </TabsTrigger>
            <TabsTrigger
              className="flex-1 gap-2 px-4 font-semibold text-xs"
              value="modpacks"
            >
              <HugeiconsIcon icon={Package01Icon} size={15} strokeWidth={2} />
              <span>Modpacks</span>
            </TabsTrigger>
            <TabsTrigger
              className="flex-1 gap-2 px-4 font-semibold text-xs"
              value="backpack"
            >
              <HugeiconsIcon icon={Backpack01Icon} size={15} strokeWidth={2} />
              <span>Backpack</span>
            </TabsTrigger>
          </TabsList>
        </div>

        <TabsContent className="flex-1 overflow-hidden" value="instances">
          <InstanceManager />
        </TabsContent>

        <TabsContent className="flex-1 overflow-hidden" value="modpacks">
          <Modpacks />
        </TabsContent>

        <TabsContent className="flex-1 overflow-hidden" value="backpack">
          <Backpack />
        </TabsContent>
      </Tabs>
    </div>
  );
}
