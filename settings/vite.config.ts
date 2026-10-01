import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5174, strictPort: true },
  build: { outDir: 'build', target: 'es2022', emptyOutDir: true },
});
