import type { Hotkey } from './api';

/** Windows virtual-key code for a browser key code, or null if it can't be a shortcut key. */
export function vkFromCode(code: string): number | null {
  let m = /^Key([A-Z])$/.exec(code);
  if (m) return m[1].charCodeAt(0);
  m = /^Digit([0-9])$/.exec(code);
  if (m) return m[1].charCodeAt(0);
  m = /^F([0-9]{1,2})$/.exec(code);
  if (m && +m[1] >= 1 && +m[1] <= 24) return 0x6f + +m[1];
  m = /^Numpad([0-9])$/.exec(code);
  if (m) return 0x60 + +m[1];
  const named: Record<string, number> = {
    Space: 0x20,
    Backspace: 0x08,
    Tab: 0x09,
    Enter: 0x0d,
    Pause: 0x13,
    PageUp: 0x21,
    PageDown: 0x22,
    End: 0x23,
    Home: 0x24,
    ArrowLeft: 0x25,
    ArrowUp: 0x26,
    ArrowRight: 0x27,
    ArrowDown: 0x28,
    Insert: 0x2d,
    Delete: 0x2e,
    Semicolon: 0xba,
    Equal: 0xbb,
    Comma: 0xbc,
    Minus: 0xbd,
    Period: 0xbe,
    Slash: 0xbf,
    Backquote: 0xc0,
    BracketLeft: 0xdb,
    Backslash: 0xdc,
    BracketRight: 0xdd,
    Quote: 0xde,
  };
  return named[code] ?? null;
}

const NAMES: Record<number, string> = {
  0x20: 'Space',
  0x08: 'Backspace',
  0x09: 'Tab',
  0x0d: 'Enter',
  0x13: 'Pause',
  0x1b: 'Esc',
  0x21: 'Page Up',
  0x22: 'Page Down',
  0x23: 'End',
  0x24: 'Home',
  0x25: '←',
  0x26: '↑',
  0x27: '→',
  0x28: '↓',
  0x2d: 'Insert',
  0x2e: 'Delete',
  0xba: ';',
  0xbb: '=',
  0xbc: ',',
  0xbd: '-',
  0xbe: '.',
  0xbf: '/',
  0xc0: '`',
  0xdb: '[',
  0xdc: '\\',
  0xdd: ']',
  0xde: "'",
};

export function keyName(vk: number): string {
  if ((vk >= 0x30 && vk <= 0x39) || (vk >= 0x41 && vk <= 0x5a)) return String.fromCharCode(vk);
  if (vk >= 0x70 && vk <= 0x87) return `F${vk - 0x6f}`;
  if (vk >= 0x60 && vk <= 0x69) return `Num ${vk - 0x60}`;
  return NAMES[vk] ?? `Key ${vk}`;
}

/** The keys of a shortcut, one per keycap. */
export function hotkeyParts(h: Hotkey): string[] {
  const parts: string[] = [];
  if (h.ctrl) parts.push('Ctrl');
  if (h.alt) parts.push('Alt');
  if (h.shift) parts.push('Shift');
  if (h.win) parts.push('Win');
  parts.push(keyName(h.key));
  return parts;
}

export function sameHotkey(a: Hotkey | null, b: Hotkey | null): boolean {
  return !!a && !!b && a.ctrl === b.ctrl && a.alt === b.alt && a.shift === b.shift && a.win === b.win && a.key === b.key;
}
