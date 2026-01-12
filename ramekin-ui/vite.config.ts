import { defineConfig, type PluginOption } from 'vite'
import solid from 'vite-plugin-solid'
import fs from 'fs'
import path from 'path'
import http from 'http'
import httpProxy from 'http-proxy'

const hostname = process.env.UI_HOSTNAME || 'localhost'
const certDir = path.join(process.env.HOME || '', '.ramekin', 'certs', hostname)

// Only use HTTPS if cert files exist (not in CI)
const certPath = path.join(certDir, 'cert.pem')
const keyPath = path.join(certDir, 'key.pem')
const certsExist = fs.existsSync(certPath) && fs.existsSync(keyPath)

const httpPort = process.env.UI_PORT_HTTP ? parseInt(process.env.UI_PORT_HTTP) : null

// Plugin to serve HTTP mirror alongside HTTPS
function httpMirrorPlugin(): PluginOption {
  return {
    name: 'http-mirror',
    configureServer(server) {
      if (!httpPort) return

      server.httpServer?.once('listening', () => {
        const proxy = httpProxy.createProxyServer({
          target: `https://localhost:${process.env.UI_PORT}`,
          secure: false, // Accept self-signed certs
          ws: true, // WebSocket support for HMR
        })

        const httpServer = http.createServer((req, res) => {
          proxy.web(req, res)
        })

        httpServer.on('upgrade', (req, socket, head) => {
          proxy.ws(req, socket, head)
        })

        httpServer.listen(httpPort, '0.0.0.0', () => {
          console.log(`  HTTP mirror:  http://localhost:${httpPort}/`)
        })
      })
    },
  }
}

export default defineConfig({
  plugins: [solid(), httpMirrorPlugin()],
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
        configure: (proxy) => {
          proxy.on('proxyReq', (_proxyReq, req) => {
            console.log('[Vite Proxy] Request:', req.method, req.url);
            console.log('[Vite Proxy] Origin header:', req.headers.origin);
          });
          proxy.on('proxyRes', (proxyRes, req, res) => {
            console.log('[Vite Proxy] Response:', proxyRes.statusCode, req.url);
            // Add CORS headers to response for cross-origin bookmarklet requests
            const origin = req.headers.origin;
            if (origin) {
              res.setHeader('Access-Control-Allow-Origin', origin);
              res.setHeader('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE, OPTIONS');
              res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization');
              res.setHeader('Access-Control-Allow-Credentials', 'true');
            }
          });
        },
      },
    },
    // Handle CORS preflight for /api routes
    cors: {
      origin: true,
      methods: ['GET', 'POST', 'PUT', 'DELETE', 'OPTIONS'],
      allowedHeaders: ['Content-Type', 'Authorization'],
      credentials: true,
    },
  },
})
