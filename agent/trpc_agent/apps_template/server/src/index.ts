import { initTRPC } from '@trpc/server';
import { nodeHTTPRequestHandler } from '@trpc/server/adapters/node-http';
import 'dotenv/config';
import superjson from 'superjson';
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const t = initTRPC.create({
  transformer: superjson,
});

const publicProcedure = t.procedure;
const router = t.router;

const appRouter = router({
  healthcheck: publicProcedure.query(() => {
    return { status: 'ok', timestamp: new Date().toISOString() };
  }),
});

export type AppRouter = typeof appRouter;

const STATIC_DIR = path.join(__dirname, '..', 'dist');

// MIME types for common file extensions
const MIME_TYPES: Record<string, string> = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.gif': 'image/gif',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf',
  '.eot': 'application/vnd.ms-fontobject',
};

async function start() {
  const port = process.env['SERVER_PORT'] || 2022;

  const server = http.createServer(async (req, res) => {
    // Enable CORS
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

    if (req.method === 'OPTIONS') {
      res.writeHead(204);
      res.end();
      return;
    }

    const url = req.url || '/';

    // Handle tRPC routes
    if (url.startsWith('/api')) {
      const trpcPath = url.replace(/^\/api/, '');
      const modifiedReq = Object.assign(req, { url: trpcPath });

      await nodeHTTPRequestHandler({
        router: appRouter,
        createContext() {
          return {};
        },
        req: modifiedReq,
        res,
        path: trpcPath,
      });
      return;
    }

    // Serve static files for frontend
    let filePath = path.join(STATIC_DIR, url === '/' ? 'index.html' : url);

    // Check if file exists
    if (!fs.existsSync(filePath)) {
      // For SPA routing - serve index.html for non-existent routes
      filePath = path.join(STATIC_DIR, 'index.html');
    }

    // If still doesn't exist (no built frontend), return 404
    if (!fs.existsSync(filePath)) {
      res.writeHead(404, { 'Content-Type': 'text/plain' });
      res.end('Not Found');
      return;
    }

    const ext = path.extname(filePath);
    const contentType = MIME_TYPES[ext] || 'application/octet-stream';

    fs.readFile(filePath, (err, data) => {
      if (err) {
        res.writeHead(500, { 'Content-Type': 'text/plain' });
        res.end('Internal Server Error');
        return;
      }

      res.writeHead(200, { 'Content-Type': contentType });
      res.end(data);
    });
  });

  server.listen(port, () => {
    console.log(`Server listening at port: ${port}`);
    console.log(`tRPC endpoint: http://localhost:${port}/api`);
    console.log(`Frontend: http://localhost:${port}/`);
  });
}

start();
