/** Stops on the timeout slider, in seconds. */
export const STEPS = [15, 30, 45, 60, 90, 120, 180, 300, 600, 900, 1200, 1800, 2700, 3600, 7200];

export const MIN_SECS = 10;
export const MAX_SECS = 24 * 60 * 60;

export type Unit = 'sec' | 'min' | 'hr';
export const UNIT_SECS: Record<Unit, number> = { sec: 1, min: 60, hr: 3600 };

/** "2 min", "1 min 30 sec", "1 hr 15 min" */
export function describe(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  const parts: string[] = [];
  if (h) parts.push(`${h} hr`);
  if (m) parts.push(`${m} min`);
  if (s) parts.push(`${s} sec`);
  return parts.join(' ') || '0 sec';
}

/** "4:05" or "1:02:05" for a countdown. */
export function clock(secs: number): string {
  const s = Math.max(0, Math.round(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, '0');
  return h ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}

/** The unit a value reads best in. */
export function bestUnit(secs: number): Unit {
  if (secs >= 3600 && secs % 3600 === 0) return 'hr';
  if (secs >= 60 && secs % 60 === 0) return 'min';
  return 'sec';
}

/** The slider stop nearest to a typed value. */
export function nearestStep(secs: number): number {
  let best = 0;
  STEPS.forEach((s, i) => {
    if (Math.abs(s - secs) < Math.abs(STEPS[best] - secs)) best = i;
  });
  return best;
}

export function clampSecs(secs: number): number {
  return Math.min(MAX_SECS, Math.max(MIN_SECS, Math.round(secs)));
}
