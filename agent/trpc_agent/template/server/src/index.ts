import { initTRPC } from "@trpc/server";
import { createHTTPServer } from "@trpc/server/adapters/standalone";
import "dotenv/config";
import cors from "cors";
import superjson from "superjson";
import { StackServerApp } from "@stackframe/react";

const t = initTRPC.context<{ req: any; res: any }>().create({
  transformer: superjson,
});

const publicProcedure = t.procedure;
const router = t.router;

function getStackServerApp(requestContext: { headers: Headers }) {
  return new StackServerApp({
    projectId: process.env["STACK_PROJECT_ID"],
    secretServerKey: process.env["STACK_SECRET_SERVER_KEY"],
    tokenStore: {
      headers: new Headers(requestContext.headers),
    },
  });
}

async function getCurrentUser(requestContext: { headers: Headers }) {
  const stackServerApp = getStackServerApp(requestContext);
  return await stackServerApp.getUser();
}

const appRouter = router({
  healthcheck: publicProcedure.query(async ({ ctx }) => {
    return {
      status: "ok",
      timestamp: new Date().toISOString(),
      currentUser: await getCurrentUser(ctx.req),
    };
  }),
});

export type AppRouter = typeof appRouter;

async function start() {
  const port = process.env["SERVER_PORT"] || 2022;

  let middleware;
  if (process.env.NODE_ENV !== "production") {
    middleware = (
      req: Parameters<ReturnType<typeof cors>>[0],
      res: Parameters<ReturnType<typeof cors>>[1],
      next: Parameters<ReturnType<typeof cors>>[2]
    ) => {
      cors({
        origin: "http://localhost:5173",
        credentials: true,
      })(req, res, next);
    };
  }

  const server = createHTTPServer({
    middleware,
    router: appRouter,
    createContext({ req, res }) {
      return { req, res };
    },
    basePath: "/",
  });
  server.listen(port);
  console.log(`TRPC server listening at port: ${port}`);
}

start();
