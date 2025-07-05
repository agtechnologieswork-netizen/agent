import { AccountSettings } from "@stackframe/react";
import { trpc } from '@/utils/trpc';
import { useEffect, useState } from 'react';

export function UserPage() {
    const [user, setUser] = useState<any>(null);
    
    useEffect(() => {
        const fetchData = async () => {
            try {
                const result = await trpc.healthcheck.query();
                setUser(result.currentUser);
            } catch (error) {
                console.error(error);
            }
        }
        fetchData();
    }, []);
    
    console.log(user);
    
    return (
        <div className="w-screen h-auto">
           <AccountSettings />
        </div>
    )
}