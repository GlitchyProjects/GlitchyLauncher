import { useState } from 'react';
import { LogOut, Sparkles, User, Shield, ChevronDown } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useAccountStore } from '@/stores/account';
import { toast } from 'sonner';

export function GlitchyProfileMenu() {
  const { user, openAuthModal, logout } = useAccountStore();
  const [menuOpen, setMenuOpen] = useState(false);

  if (!user) {
    return (
      <Button
        type='button'
        onClick={() => openAuthModal('login')}
        className='w-full h-10 px-3 rounded-xl bg-emerald-600/20 border border-emerald-500/30 text-emerald-400 hover:bg-emerald-600/30 text-xs font-bold transition-all flex items-center justify-between group-data-[state=collapsed]:justify-center group-data-[state=collapsed]:px-0 shadow-lg shadow-emerald-950/20'
        title="Glitchy Account"
      >
        <div className='flex items-center gap-2'>
          <Shield className='size-4 text-emerald-400 shrink-0' />
          <span className='group-data-[state=collapsed]:hidden tracking-wide text-xs'>Glitchy Account</span>
        </div>
        <Sparkles className='size-3.5 text-emerald-400/80 group-data-[state=collapsed]:hidden' />
      </Button>
    );
  }

  const handleLogout = async () => {
    setMenuOpen(false);
    await logout();
    toast.success('از حساب کاربری خارج شدید');
  };

  return (
    <div className='relative w-full'>
      <button
        type='button'
        onClick={() => setMenuOpen(!menuOpen)}
        className='w-full h-10 px-2.5 rounded-xl bg-white/5 border border-white/10 hover:bg-white/10 text-white text-xs font-semibold flex items-center justify-between transition-all group-data-[state=collapsed]:justify-center group-data-[state=collapsed]:px-0'
      >
        <div className='flex items-center gap-2 min-w-0'>
          <div className='size-7 rounded-lg bg-emerald-500/20 border border-emerald-500/40 text-emerald-400 flex items-center justify-center text-xs font-bold uppercase shrink-0'>
            {user.username.slice(0, 2)}
          </div>
          <div className='flex flex-col text-start min-w-0 group-data-[state=collapsed]:hidden'>
            <span className='truncate text-xs font-bold text-white leading-tight'>{user.username}</span>
            <span className='text-[10px] text-emerald-400 font-medium leading-tight'>{user.badge || 'Glitchy Account'}</span>
          </div>
        </div>
        <ChevronDown className='size-3.5 text-muted-foreground group-data-[state=collapsed]:hidden' />
      </button>

      {menuOpen && (
        <>
          <div
            className='fixed inset-0 z-40'
            onClick={() => setMenuOpen(false)}
          />
          <div
            className='absolute bottom-full mb-2 right-0 left-0 min-w-56 z-50 rounded-xl border border-white/10 bg-[#16161b] p-3 shadow-2xl backdrop-blur-xl'
            dir='rtl'
          >
            <div className='flex items-center gap-2.5 pb-3 border-b border-white/10'>
              <div className='size-9 rounded-xl bg-emerald-500/20 border border-emerald-500/40 text-emerald-400 flex items-center justify-center text-sm font-bold uppercase'>
                {user.username.slice(0, 2)}
              </div>
              <div className='min-w-0 flex-1'>
                <p className='text-xs font-bold text-white truncate'>{user.username}</p>
                <p className='text-[10px] text-muted-foreground truncate'>{user.email}</p>
              </div>
            </div>

            <div className='py-2 space-y-1 text-xs'>
              <div className='flex items-center justify-between px-2 py-1.5 rounded-lg bg-white/5'>
                <span className='text-muted-foreground text-[11px]'>نقش کاربری</span>
                <span className='text-[10px] font-bold text-emerald-400 bg-emerald-500/15 px-2 py-0.5 rounded-full'>
                  {user.badge || 'عضو گلیچی'}
                </span>
              </div>
            </div>

            <button
              type='button'
              onClick={handleLogout}
              className='w-full mt-2 flex items-center gap-2 px-2 py-2 rounded-lg text-xs font-medium text-red-400 hover:bg-red-500/10 transition-colors'
            >
              <LogOut className='size-3.5' />
              <span>خروج از حساب کاربری</span>
            </button>
          </div>
        </>
      )}
    </div>
  );
}
