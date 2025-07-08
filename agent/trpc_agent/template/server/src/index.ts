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

const appRouter = router({
  healthcheck: publicProcedure.query(async ({ ctx }) => {
    const stackServerApp = new StackServerApp({
      projectId: process.env["STACK_PROJECT_ID"],
      secretServerKey: process.env["STACK_SECRET_SERVER_KEY"],

      // Since we receive the cookies in the request, we need to initiate the
      // token store from the request's context. If we were not using tRPC,
      // we could just set `tokenStore` to `ctx.req` and it would work out of the
      // box, but this request object is a little bit different.
      tokenStore: {
        headers: new Headers(ctx.req.headers),
      },
    });

    return {
      status: "ok",
      timestamp: new Date().toISOString(),
      currentUser: await stackServerApp.getUser(),
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
