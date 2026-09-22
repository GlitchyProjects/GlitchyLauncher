import { createMemoryRouter } from "react-router";
import Backpack from "@/pages/backpack.tsx";
import Library from "@/pages/library";
import Modpacks from "@/pages/modpacks.tsx";
import Settings from "@/pages/settings.tsx";
import { RouteErrorBoundary } from "./components/glitchy/error-boundary";
import Layout from "./layout";
import IndexPage from "./pages";
import AssistantPage from "./pages/assistant";
import Console from "./pages/console.tsx";
import Downloads from "./pages/downloads";
import Achievements from "./pages/glitchy/achievements";
import Badges from "./pages/glitchy/badges";
import Journey from "./pages/glitchy/journey";
import GlitchyProfile from "./pages/glitchy/profile";
import Community from "./pages/community";
import InstanceManager from "./pages/instance-manager";

export const router = createMemoryRouter([
  {
    children: [
      { element: <IndexPage />, path: "/" },
      { element: <Library />, path: "/library" },
      { element: <Community />, path: "/community" },
      { element: <GlitchyProfile />, path: "/profile" },
      { element: <AssistantPage />, path: "/assistant" },
      { element: <Settings />, path: "/settings" },
      { element: <Downloads />, path: "/downloads" },
      { element: <InstanceManager />, path: "/instances" },
      { element: <Backpack />, path: "/backpack" },
      { element: <Modpacks />, path: "/modpacks" },
      { element: <Console />, path: "/console" },
      { element: <Achievements />, path: "/achievements" },
      { element: <Badges />, path: "/badges" },
      { element: <Journey />, path: "/journey" },
    ],
    element: <Layout />,
    errorElement: <RouteErrorBoundary />,
  },
]);
