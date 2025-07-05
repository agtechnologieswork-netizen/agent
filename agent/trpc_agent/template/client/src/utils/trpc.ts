import { createTRPCClient, httpBatchLink, loggerLink } from '@trpc/client';
import type { AppRouter } from '../../../server/src';
import superjson from 'superjson';

export const trpc = createTRPCClient<AppRouter>({
  links: [
    httpBatchLink({ url: 'http://localhost:2022/', transformer: superjson, fetch: (url, options) => {
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
