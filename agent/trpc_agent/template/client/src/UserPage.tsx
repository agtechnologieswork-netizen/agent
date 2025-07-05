import { AccountSettings } from "@stackframe/react";
import { trpc } from '@/utils/trpc';
import { useEffect } from 'react';

export function UserPage() {
    useEffect(() => {
        const fetchData = async () => {
            try {
                const result = await trpc.healthcheck.query();
                console.log(result);
            } catch (error) {
                console.error(error);
            }
        }
        fetchData();
    }, []);
    
    return (
        <div className="w-screen h-auto">
           <AccountSettings />
        </div>
    )
}