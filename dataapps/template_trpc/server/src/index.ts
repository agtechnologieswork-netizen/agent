import { initTRPC } from "@trpc/server";
import { createExpressMiddleware } from "@trpc/server/adapters/express";
import express from "express";
import "dotenv/config";
import superjson from "superjson";
import path from "node:path";
import { z } from "zod";
import { DatabricksClient } from "./databricks";

const STATIC_DIR = path.join(__dirname, "..", "public");

const t = initTRPC.create({
  transformer: superjson,
});

const publicProcedure = t.procedure;
const router = t.router;

const appRouter = router({
  healthcheck: publicProcedure.query(() => {
    return { status: "ok", timestamp: new Date().toISOString() };
  }),
  executeQuery: publicProcedure
    .input(
      z.object({
        sql: z.string(),
        // Optional: pass a zod schema for runtime validation
        // schema: z.object({ id: z.number(), name: z.string() })
      })
    )
    .query(async ({ input }) => {
      const client = new DatabricksClient();
      return await client.executeQuery(input.sql);
    }),
});

export type AppRouter = typeof appRouter;

const app = express();

// Serve static files
app.use(express.static(STATIC_DIR));

app.use(
  "/api",
  createExpressMiddleware({
    router: appRouter,
    createContext() {
      return {};
    },
  }),
);

app.get("/{*zzz}", (req, res) => {
  res.sendFile(path.join(STATIC_DIR, "index.html"));
});

const port = process.env["PORT"] || 8000;

app.listen(port, () => {
  console.log(`Server listening at port: ${port}`);
  console.log(`tRPC endpoint: http://localhost:${port}/api`);
  console.log(`Frontend: http://localhost:${port}/`);
});
