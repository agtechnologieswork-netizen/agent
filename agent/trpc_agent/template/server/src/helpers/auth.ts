import { StackServerApp } from "@stackframe/react";

export function getStackServerApp(requestContext: { headers: Headers }) {
  return new StackServerApp({
    projectId: process.env["STACK_PROJECT_ID"],
    secretServerKey: process.env["STACK_SECRET_SERVER_KEY"],
    tokenStore: {
      headers: new Headers(requestContext.headers),
    },
  });
}

export async function getCurrentUser(requestContext: { headers: Headers }) {
  const stackServerApp = getStackServerApp(requestContext);
  return await stackServerApp.getUser();
}
