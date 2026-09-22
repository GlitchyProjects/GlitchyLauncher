"use client";

import {
  Award01Icon,
  Download01Icon,
  GameboyIcon,
  Package01Icon,
  Settings01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import { NavLink } from "react-router";
import {
  SidebarGroup,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";

export function NavMenu() {
  const { t } = useTranslation();

  const items = [
    {
      icon: <HugeiconsIcon icon={GameboyIcon} strokeWidth={2} />,
      title: t("nav_play", "Play"),
      url: "/",
    },
    {
      icon: <HugeiconsIcon icon={Package01Icon} strokeWidth={2} />,
      title: t("nav_library", "Library"),
      url: "/library",
    },
    {
      icon: <HugeiconsIcon icon={Download01Icon} strokeWidth={2} />,
      title: t("nav_downloads", "Downloads"),
      url: "/downloads",
    },
    {
      icon: <HugeiconsIcon icon={Award01Icon} strokeWidth={2} />,
      title: t("nav_profile", "Profile"),
      url: "/profile",
    },
    {
      icon: <Sparkles className="size-5 text-primary" />,
      title: t("nav_assistant", "AI Assistant"),
      url: "/assistant",
    },
    {
      icon: <HugeiconsIcon icon={Settings01Icon} strokeWidth={2} />,
      title: t("nav_settings", "Settings"),
      url: "/settings",
    },
  ];

  return (
    <div className="space-y-1">
      <SidebarGroup className="py-1">
        <SidebarMenu>
          {items.map((item) => (
            <NavLink key={item.url} to={item.url}>
              {({ isActive }) => (
                <SidebarMenuItem>
                  <SidebarMenuButton
                    className="rounded-xl transition-colors"
                    isActive={isActive}
                    tooltip={item.title}
                  >
                    {item.icon}
                    <span className="font-semibold">{item.title}</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              )}
            </NavLink>
          ))}
        </SidebarMenu>
      </SidebarGroup>
    </div>
  );
}
