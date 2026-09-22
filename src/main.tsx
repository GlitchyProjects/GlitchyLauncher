import React from "react";
import { createRoot } from "react-dom/client";
import { RouterProvider } from "react-router";
import { router } from "./router";
import "./lib/i18n";

const BLOCKED_FUNCTION_KEYS = new Set([
  "F1",
  "F2",
  "F3",
  "F4",
  "F5",
  "F6",
  "F7",
  "F8",
  "F9",
  "F10",
  "F12",
]);
const BLOCKED_BROWSER_KEYS = new Set([
  "BrowserBack",
  "BrowserFavorites",
  "BrowserForward",
  "BrowserHome",
  "BrowserRefresh",
  "BrowserSearch",
  "BrowserStop",
]);

const isBrowserShortcut = (event: KeyboardEvent): boolean => {
  if (
    BLOCKED_FUNCTION_KEYS.has(event.key) ||
    BLOCKED_BROWSER_KEYS.has(event.key)
  ) {
    return true;
  }

  const key = event.key.toLowerCase();
  const blockedControlShortcut =
    (event.ctrlKey || event.metaKey) &&
    (key === "f" ||
      key === "0" ||
      key === "+" ||
      key === "-" ||
      key === "=" ||
      key === "p" ||
      key === "r" ||
      key === "s" ||
      key === "u" ||
      (event.shiftKey && (key === "c" || key === "i" || key === "j")));
  const blockedNavigationShortcut =
    event.altKey && (event.key === "ArrowLeft" || event.key === "ArrowRight");

  return blockedControlShortcut || blockedNavigationShortcut;
};

// Remove browser chrome behaviours so the Tauri window feels like a
// desktop launcher. F11 remains available for maximize / restore.
if (typeof window !== "undefined") {
  window.addEventListener("contextmenu", (e) => e.preventDefault());
  window.addEventListener(
    "wheel",
    (event) => {
      if (event.ctrlKey || event.metaKey) {
        event.preventDefault();
      }
    },
    { passive: false }
  );
  window.addEventListener(
    "keydown",
    (event) => {
      if (isBrowserShortcut(event)) {
        event.preventDefault();
        event.stopImmediatePropagation();
      }
    },
    { capture: true }
  );
}

// Render React. Wrap in try/catch so a render-time error shows a visible
// message instead of leaving the "Loading Glitchy Launcher…" placeholder
// from index.html sitting there forever.
try {
  const rootElement = document.getElementById("root");
  if (!rootElement) {
    throw new Error("Root element #root not found in index.html");
  }
  createRoot(rootElement).render(
    <React.StrictMode>
      <RouterProvider router={router} />
    </React.StrictMode>
  );
} catch (err) {
  const rootElement = document.getElementById("root");
  if (rootElement) {
    rootElement.innerHTML =
      '<div style="font-family:Inter,system-ui,sans-serif;color:#b4d4f4;padding:32px;background:#0a0e1a;height:100vh;box-sizing:border-box;">' +
      '<h2 style="color:#2484EC;margin:0 0 12px 0;">Glitchy Launcher failed to start</h2>' +
      '<pre style="white-space:pre-wrap;font-size:12px;color:#94BCE4;">' +
      String(err?.toString?.() ?? err) +
      "</pre>" +
      '<p style="margin-top:16px;font-size:11px;color:#6c7a89;">' +
      "If this keeps happening, please report the error message above." +
      "</p></div>";
  }
}
