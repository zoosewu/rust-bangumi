import { defineConfig, loadEnv } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '')

  const corePort = env.CORE_PORT || '8000'
  const downloaderPort = env.DOWNLOADER_PORT || '8002'
  const viewerPort = env.VIEWER_PORT || '8003'

  return {
    plugins: [react(), tailwindcss()],
    resolve: {
      alias: {
        '@': path.resolve(__dirname, './src'),
      },
    },
    server: {
      port: 8004,
      proxy: {
        '/api/core': {
          target: `http://localhost:${corePort}`,
          rewrite: (p) => p.replace(/^\/api\/core/, ''),
          changeOrigin: true,
        },
        '/api/downloader': {
          target: `http://localhost:${downloaderPort}`,
          rewrite: (p) => p.replace(/^\/api\/downloader/, ''),
          changeOrigin: true,
        },
        '/api/viewer': {
          target: `http://localhost:${viewerPort}`,
          rewrite: (p) => p.replace(/^\/api\/viewer/, ''),
          changeOrigin: true,
        },
      },
    },
  }
})
