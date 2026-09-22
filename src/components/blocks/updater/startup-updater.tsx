import { useEffect, useState } from 'react';
import { DownloadCloud, RefreshCw, Sparkles, CheckCircle2 } from 'lucide-react';
import { toast } from 'sonner';
import { backend } from '@/lib/utils';
import type { LauncherUpdateInfo } from '@/invokes';

export function StartupUpdater() {
  const [updateInfo, setUpdateInfo] = useState<LauncherUpdateInfo | null>(null);
  const [isUpdating, setIsUpdating] = useState(false);
  const [updateStatus, setUpdateStatus] = useState<string>('');

  useEffect(() => {
    let cancelled = false;

    async function checkUpdate() {
      try {
        // Silent version check on startup
        const res = await backend('check_launcher_update');
        if (cancelled) return;

        if (res && res.hasUpdate && res.downloadUrl) {
          setUpdateInfo(res);
          setIsUpdating(true);
          setUpdateStatus(`نسخه جدید (${res.latestVersion}) یافت شد. در حال دانلود و نصب خودکار...`);

          // Automatically apply update
          try {
            await backend('apply_launcher_update', { downloadUrl: res.downloadUrl });
          } catch (err: any) {
            if (!cancelled) {
              setIsUpdating(false);
              setUpdateStatus('خطا در اعمال خودکار آپدیت. می‌توانید به صورت دستی دانلود نمایید.');
              toast.error('خطا در دریافت خودکار نسخه جدید: ' + (err?.message || ''));
            }
          }
        }
      } catch {
        // SILENT IGNORE: User is offline or rate-limited, silently proceed without any intrusive dialogs or errors
      }
    }

    // Small delay to let the UI finish initial layout
    const timer = setTimeout(() => {
      checkUpdate();
    }, 1500);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, []);

  if (!updateInfo || !updateInfo.hasUpdate) return null;

  return (
    <div
      className='fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-md p-4 animate-in fade-in duration-300'
      dir='rtl'
    >
      <div className='relative w-full max-w-md overflow-hidden rounded-2xl border border-emerald-500/30 bg-[#121216] p-6 shadow-2xl shadow-emerald-950/40 text-center'>
        <div className='absolute -top-24 -left-24 h-48 w-48 rounded-full bg-emerald-500/20 blur-3xl' />

        <div className='inline-flex items-center justify-center size-14 rounded-2xl bg-emerald-500/15 border border-emerald-500/30 text-emerald-400 mb-4 shadow-inner animate-pulse'>
          <DownloadCloud className='size-7' />
        </div>

        <h3 className='text-lg font-black text-white'>
          به‌روزرسانی خودکار <span className='text-emerald-400'>Glitchy Launcher</span>
        </h3>
        <p className='text-xs text-muted-foreground mt-1'>
          نسخه فعلی: {updateInfo.currentVersion} ← نسخه جدید: {updateInfo.latestVersion}
        </p>

        <div className='my-5 rounded-xl bg-white/5 border border-white/10 p-3.5 text-xs text-emerald-300 flex items-center justify-center gap-2'>
          {isUpdating ? (
            <>
              <RefreshCw className='size-4 animate-spin text-emerald-400' />
              <span>{updateStatus}</span>
            </>
          ) : (
            <>
              <CheckCircle2 className='size-4 text-emerald-400' />
              <span>{updateStatus}</span>
            </>
          )}
        </div>

        {updateInfo.releaseNotes && (
          <div className='max-h-32 overflow-y-auto rounded-lg bg-black/40 p-3 text-right text-[11px] text-muted-foreground mb-4 border border-white/5'>
            <div className='font-bold text-white mb-1'>تغییرات این نسخه:</div>
            <pre className='whitespace-pre-wrap font-sans text-xs'>{updateInfo.releaseNotes}</pre>
          </div>
        )}

        {!isUpdating && updateInfo.downloadUrl && (
          <div className='flex items-center gap-2'>
            <button
              type='button'
              onClick={async () => {
                setIsUpdating(true);
                setUpdateStatus('در حال تلاش مجدد برای دریافت آپدیت...');
                try {
                  await backend('apply_launcher_update', { downloadUrl: updateInfo.downloadUrl! });
                } catch (e: any) {
                  setIsUpdating(false);
                  setUpdateStatus('خطا در دانلود آپدیت: ' + e?.message);
                }
              }}
              className='flex-1 h-10 rounded-xl bg-emerald-600 hover:bg-emerald-500 text-white font-bold text-xs shadow-lg shadow-emerald-600/30 transition-all'
            >
              تلاش مجدد برای نصب
            </button>
            <button
              type='button'
              onClick={() => setUpdateInfo(null)}
              className='px-4 h-10 rounded-xl bg-white/5 hover:bg-white/10 text-muted-foreground hover:text-white text-xs font-semibold'
            >
              بعداً
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
