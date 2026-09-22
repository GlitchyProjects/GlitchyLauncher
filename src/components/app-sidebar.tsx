import type * as React from "react";

import { NavMenu } from "@/components/nav-menu";
import { NavProfile } from "@/components/nav-profile";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarRail,
} from "@/components/ui/sidebar";

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  return (
    <Sidebar className="border-0!" collapsible="icon" {...props}>
      <SidebarHeader>
        <div className="flex items-center justify-center gap-2 group-data-[state=collapsed]:gap-0">
          <img
            alt="Glitchy Launcher"
            className="size-8"
            height={32}
            src="/icon.png"
            width={32}
          />
          <h2 className="mt-1 line-clamp-1 w-46 overflow-hidden font-bold text-2xl transition-[width] group-data-[state=collapsed]:w-0">
            Glitchy
          </h2>
        </div>
      </SidebarHeader>
      <SidebarContent>
        <NavMenu />
      </SidebarContent>
      <SidebarFooter>
        <NavProfile />
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  );
}
