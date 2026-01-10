import { defineConfig } from 'vite'
import solid from 'vite-plugin-solid'
import fs from 'fs'
import path from 'path'

const hostname = process.env.UI_HOSTNAME || 'localhost'
const certDir = path.join(process.env.HOME || '', '.ramekin', 'certs', hostname)

// Only use HTTPS if cert files exist (not in CI)
const certPath = path.join(certDir, 'cert.pem')
const keyPath = path.join(certDir, 'key.pem')
const certsExist = fs.existsSync(certPath) && fs.existsSync(keyPath)

export default defineConfig({
  plugins: [solid()],
  server: {
    allowedHosts: [hostname],
    host: '0.0.0.0',
    port: parseInt(process.env.UI_PORT!),
    https: certsExist
      ? {
          key: fs.readFileSync(keyPath),
          cert: fs.readFileSync(certPath),
        }
      : undefined,
    proxy: {
      '/api': {
        target: `http://localhost:${process.env.PORT}`,
        changeOrigin: true,
      },
    },
  },
})
