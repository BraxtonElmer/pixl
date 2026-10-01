// Builds the tray app and places it where Tauri's bundler picks it up, so the
// installer ships both programs. Runs as part of `npm run tauri build`.
import { execSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const repo = fileURLToPath(new URL('../../', import.meta.url));
const binaries = fileURLToPath(new URL('../src-tauri/binaries/', import.meta.url));

const triple =
  process.env.TAURI_ENV_TARGET_TRIPLE ?? execSync('rustc -vV').toString().match(/host: (\S+)/)[1];

execSync('cargo build --release -p pixl', { cwd: repo, stdio: 'inherit' });
mkdirSync(binaries, { recursive: true });
copyFileSync(`${repo}target/release/pixl-tray.exe`, `${binaries}Pixl-${triple}.exe`);
console.log(`Tray app ready for bundling (${triple}).`);
