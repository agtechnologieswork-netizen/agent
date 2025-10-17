import { useState, useEffect } from 'react';
import { trpc } from './utils/trpc';

function App() {
  const [health, setHealth] = useState<{ status: string; timestamp: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    trpc.healthcheck.query()
      .then(setHealth)
      .catch((err) => setError(err.message));
  }, []);

  return (
    <div className="container mx-auto p-4">
      <h1 className="text-2xl font-bold mb-4">tRPC Template</h1>
      <p className="text-gray-600 mb-8">
        Your tRPC app is running!
      </p>
      {health && (
        <div className="p-4 mb-4 bg-green-50 border border-green-200 rounded-md">
          <p>✓ Server Status: {health.status}</p>
          <p>Timestamp: {health.timestamp}</p>
        </div>
      )}
      {error && (
        <div className="p-4 mb-4 bg-red-50 border border-red-200 rounded-md">
          <p>✗ Error: {error}</p>
        </div>
      )}
    </div>
  );
}

export default App;
