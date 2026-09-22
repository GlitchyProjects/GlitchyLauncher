import { create } from 'zustand';
import { backend } from '@/lib/utils';
import type { GlitchyUser } from '@/invokes';

interface AccountStore {
  user: GlitchyUser | null;
  isLoading: boolean;
  isAuthModalOpen: boolean;
  authModalTab: 'login' | 'register';
  authModalReason: string | null;
  setUser: (user: GlitchyUser | null) => void;
  openAuthModal: (tab?: 'login' | 'register', reason?: string) => void;
  closeAuthModal: () => void;
  fetchUser: () => Promise<GlitchyUser | null>;
  logout: () => Promise<void>;
}

export const useAccountStore = create<AccountStore>((set) => ({
  user: null,
  isLoading: true,
  isAuthModalOpen: false,
  authModalTab: 'login',
  authModalReason: null,

  setUser: (user) => set({ user }),

  openAuthModal: (tab = 'login', reason) =>
    set({
      isAuthModalOpen: true,
      authModalTab: tab,
      authModalReason: reason || null,
    }),

  closeAuthModal: () =>
    set({
      isAuthModalOpen: false,
      authModalReason: null,
    }),

  fetchUser: async () => {
    try {
      set({ isLoading: true });
      const user = await backend('glitchy_account_get_current');
      set({ user, isLoading: false });
      return user;
    } catch {
      set({ user: null, isLoading: false });
      return null;
    }
  },

  logout: async () => {
    try {
      await backend('glitchy_account_logout');
    } finally {
      set({ user: null });
    }
  },
}));
