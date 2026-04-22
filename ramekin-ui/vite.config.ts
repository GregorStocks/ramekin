import { defineConfig, type PluginOption, type PreviewOptions, type ServerOptions } from 'vite'
import solid from 'vite-plugin-solid'
import { execSync } from 'child_process'
import fs from 'fs'
import path from 'path'
import http from 'http'
import httpProxy from 'http-proxy'

const selfSignedUrl = process.env.RAMEKIN_SELF_SIGNED_URL || 'https://localhost:5173'
const hostname = new URL(selfSignedUrl).hostname
const certDir = path.join(process.env.HOME || '', '.ramekin', 'certs', hostname)

// Only use HTTPS if cert files exist (not in CI)
const certPath = path.join(certDir, 'cert.pem')
const keyPath = path.join(certDir, 'key.pem')
const certsExist = fs.existsSync(certPath) && fs.existsSync(keyPath)

const httpPort = process.env.UI_PORT_HTTP ? parseInt(process.env.UI_PORT_HTTP) : null

// Plugin to serve HTTP mirror alongside HTTPS. The mirror proxies UI traffic
// to the vite server and routes /api straight to the backend, avoiding a
// double-proxy chain (mirror -> vite -> /api proxy -> backend) that returns
// 502s for API calls under `vite preview`.
function httpMirrorPlugin(): PluginOption {
  const attach = (httpServer: { once(event: 'listening', cb: () => void): unknown } | null | undefined) => {
    if (!httpPort || !httpServer) return
    httpServer.once('listening', () => {
      const uiProtocol = certsExist ? 'https' : 'http'
      const uiProxy = httpProxy.createProxyServer({
        target: `${uiProtocol}://localhost:${process.env.UI_PORT}`,
        secure: false, // Accept self-signed certs
        ws: true, // WebSocket support for HMR
      })
      const apiProxy = httpProxy.createProxyServer({
        target: `http://localhost:${process.env.PORT}`,
        changeOrigin: true,
      })
      apiProxy.on('proxyRes', (_proxyRes, req, res) => {
        const origin = req.headers.origin
        if (origin) {
          res.setHeader('Access-Control-Allow-Origin', origin)
          res.setHeader('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE, OPTIONS')
          res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization')
          res.setHeader('Access-Control-Allow-Credentials', 'true')
        }
      })

      const mirror = http.createServer((req, res) => {
        if (req.url?.startsWith('/api')) {
          apiProxy.web(req, res)
        } else {
          uiProxy.web(req, res)
        }
      })

      mirror.on('upgrade', (req, socket, head) => {
        uiProxy.ws(req, socket, head)
      })

      mirror.listen(httpPort, '0.0.0.0', () => {
        console.log(`  HTTP mirror:  http://localhost:${httpPort}/`)
      })
    })
  }
  return {
    name: 'http-mirror',
    configureServer(server) {
      attach(server.httpServer)
    },
    configurePreviewServer(server) {
      attach(server.httpServer)
    },
  }
}

const sharedServerOptions = {
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
} satisfies ServerOptions & PreviewOptions

const buildCommit = execSync('git rev-parse --short HEAD').toString().trim()
const buildTime = new Date().toISOString()
const externalUrl = process.env.RAMEKIN_EXTERNAL_URL
if (!externalUrl) {
  throw new Error('RAMEKIN_EXTERNAL_URL is required (see dev.env.example)')
}

export default defineConfig({
  define: {
    __BUILD_COMMIT__: JSON.stringify(buildCommit),
    __BUILD_TIME__: JSON.stringify(buildTime),
    __EXTERNAL_URL__: JSON.stringify(externalUrl),
  },
  plugins: [solid(), httpMirrorPlugin()],
  server: sharedServerOptions,
  preview: sharedServerOptions,
})
