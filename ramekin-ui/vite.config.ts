import { defineConfig } from 'vite'
import solid from 'vite-plugin-solid'
import fs from 'fs'
import path from 'path'

const hostname = process.env.UI_HOSTNAME || 'localhost'
const certDir = path.join(process.env.HOME || '', '.ramekin', 'certs', hostname)

export default defineConfig({
  plugins: [solid()],
  server: {
    allowedHosts: [hostname],
    host: '0.0.0.0',
    port: parseInt(process.env.UI_PORT!),
    https: {
      key: fs.readFileSync(path.join(certDir, 'key.pem')),
      cert: fs.readFileSync(path.join(certDir, 'cert.pem')),
    },
    proxy: {
      '/api': {
        target: `http://localhost:${process.env.PORT}`,
        changeOrigin: true,
      },
    },
  },
})
