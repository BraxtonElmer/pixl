import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

export type { Update };

/** The newer version on GitHub, or null when up to date or offline. */
export async function findUpdate(): Promise<Update | null> {
  try {
    return await check();
  } catch {
    return null;
  }
}

/**
 * Download, verify (the updater refuses anything not signed with our key) and
 * install. Windows closes the app for the installer, which starts it again.
 */
export async function installUpdate(update: Update, progress: (fraction: number) => void): Promise<void> {
  let total = 0;
  let done = 0;
  await update.downloadAndInstall((e) => {
    if (e.event === 'Started') total = e.data.contentLength ?? 0;
    else if (e.event === 'Progress') {
      done += e.data.chunkLength;
      progress(total > 0 ? done / total : 0);
    } else if (e.event === 'Finished') progress(1);
  });
  await relaunch();
}
