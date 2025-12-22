import { defineConfig } from 'vite'
import solid from 'vite-plugin-solid'

export default defineConfig({
  plugins: [solid()],
  server: {
    allowedHosts: ['media.noodles'],
    host: '0.0.0.0',
    proxy: {
      '/api': {
        target: process.env.API_URL || 'http://localhost:3000',
        changeOrigin: true,
      },
    },
  },
})
