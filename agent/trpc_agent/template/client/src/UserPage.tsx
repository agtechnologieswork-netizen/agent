import { AccountSettings } from "@stackframe/react";
import { useEffect } from 'react';
import { trpc } from './utils/trpc';

export function UserPage() {
  useEffect(() => {
    const data = async () => {
      const result = await trpc.healthcheck.query();
      console.log(result);
    }
    data();
  }, []);
  return (
    <div className="w-screen h-auto">
      <AccountSettings />
    </div>
  );
}
