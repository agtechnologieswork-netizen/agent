import { createTRPCClient, httpBatchLink, loggerLink } from '@trpc/client';
import type { AppRouter } from '../../../server/src';
import superjson from 'superjson';

// In local dev, we serve the tRPC server on port 2022 without any path. In
// production, we serve the tRPC server on the same path as the frontend but
// with a /api path.
let BASE_URL = 'http://localhost:2022/';
if (process.env.NODE_ENV === 'production') {
  BASE_URL = '/api';
}

export const trpc = createTRPCClient<AppRouter>({
  links: [
    httpBatchLink({ url: BASE_URL, transformer: superjson, fetch: (url, options) => {
      return fetch(url, {
        ...options,
        credentials: 'include',
      });
    },
}),
    loggerLink({
          enabled: (opts) =>
            (typeof window !== 'undefined') ||
            (opts.direction === 'down' && opts.result instanceof Error),
        }),
  ],
});
