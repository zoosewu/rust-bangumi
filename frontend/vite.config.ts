import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: 5173,
    proxy: {
      '/api/core': {
        target: 'http://localhost:8000',
        rewrite: (p) => p.replace(/^\/api\/core/, ''),
        changeOrigin: true,
      },
      '/api/downloader': {
        target: 'http://localhost:8002',
        rewrite: (p) => p.replace(/^\/api\/downloader/, ''),
        changeOrigin: true,
      },
      '/api/viewer': {
        target: 'http://localhost:8003',
        rewrite: (p) => p.replace(/^\/api\/viewer/, ''),
        changeOrigin: true,
      },
    },
  },
})
