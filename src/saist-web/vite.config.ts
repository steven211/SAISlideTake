import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

export default defineConfig({
  plugins: [vue()],
  base: './',
  server: { host: '127.0.0.1', port: 1425, strictPort: true },
  build: { outDir: '../../9.output/web', emptyOutDir: true },
});
