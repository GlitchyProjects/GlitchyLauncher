import { create } from "zustand";
import type { DownloadSessionInfo } from "@/invokes";

/**
 * Global download-session state.
 *
 * Downloads run in the Rust backend on background tasks; this store is
 * only a mirror of the `download-session` events (kept in sync by
 * <DownloadsListener />, which is mounted once in the layout). Because
 * the state lives here instead of inside the downloads page, navigating
 * away and back never loses an in-progress download.
 */
interface DownloadsStore {
  /** Locally forget a terminal session (backend keeps its history). */
  dismiss: (id: string) => void;
  dismissFinished: () => void;
  /** Session ids in the order they were first seen (newest first). */
  order: string[];
  sessions: Record<string, DownloadSessionInfo>;
  upsert: (info: DownloadSessionInfo) => void;
  upsertMany: (infos: DownloadSessionInfo[]) => void;
}

export const useDownloads = create<DownloadsStore>()((set) => ({
  dismiss: (id) =>
    set((state) => {
      const sessions = { ...state.sessions };
      delete sessions[id];
      return { order: state.order.filter((x) => x !== id), sessions };
    }),

  dismissFinished: () =>
    set((state) => {
      const sessions: Record<string, DownloadSessionInfo> = {};
      const order: string[] = [];
      for (const id of state.order) {
        const s = state.sessions[id];
        if (s && (s.state === "running" || s.state === "paused")) {
          sessions[id] = s;
          order.push(id);
        }
      }
      return { order, sessions };
    }),
  order: [],
  sessions: {},

  upsert: (info) =>
    set((state) => {
      const known = state.sessions[info.id] !== undefined;
      return {
        order: known
          ? [info.id, ...state.order.filter((id) => id !== info.id)]
          : [info.id, ...state.order],
        sessions: { ...state.sessions, [info.id]: info },
      };
    }),

  upsertMany: (infos) =>
    set((state) => {
      const sessions = { ...state.sessions };
      const order = [...state.order];
      for (const info of infos) {
        if (sessions[info.id] === undefined) {
          order.unshift(info.id);
        }
        sessions[info.id] = info;
      }
      return { order, sessions };
    }),
}));

export const selectSessionList = (state: DownloadsStore) =>
  state.order.map((id) => state.sessions[id]);
