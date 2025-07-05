import { initTRPC } from '@trpc/server';
import { createHTTPServer } from '@trpc/server/adapters/standalone';
import 'dotenv/config';
import cors from 'cors';
import superjson from 'superjson';
import { StackServerApp } from '@stackframe/react';

const t = initTRPC.context<{ req: any; res: any }>().create({
  transformer: superjson,
});

const publicProcedure = t.procedure;
const router = t.router;

const appRouter = router({
  healthcheck: publicProcedure.query(async ({ ctx }) => {
    const stackServerApp = new StackServerApp({
      projectId: process.env['STACK_PROJECT_ID'],
      secretServerKey: process.env['STACK_SECRET_SERVER_KEY'],
      tokenStore: "memory",
      redirectMethod: ctx.req?.headers,
    });
    
    const cookies = ctx.req?.headers.cookie;
    
    console.log("user", await stackServerApp.getUser());

    return { status: 'ok', timestamp: new Date().toISOString(), currentUser: await stackServerApp.getUser() };
  }),
});

export type AppRouter = typeof appRouter;

async function start() {
  const port = process.env['SERVER_PORT'] || 2022;
  const server = createHTTPServer({
    middleware: (req, res, next) => {
      cors({
        origin: 'http://localhost:5173',
        credentials: true,
      })(req, res, next);
    },
    router: appRouter,
    createContext({ req, res }) {
      return { req, res };
    },
    basePath: '/',
  });
  server.listen(port);
  console.log(`TRPC server listening at port: ${port}`);
}

start();
