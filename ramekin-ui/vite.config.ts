import { defineConfig } from 'vite'
import solid from 'vite-plugin-solid'
import basicSsl from '@vitejs/plugin-basic-ssl'

export default defineConfig({
  plugins: [solid(), basicSsl()],
  server: {
    allowedHosts: ['media.noodles'],
    host: '0.0.0.0',
    port: parseInt(process.env.UI_PORT!),
    proxy: {
      '/api': {
        target: `http://localhost:${process.env.PORT}`,
        changeOrigin: true,
      },
    },
  },
})
