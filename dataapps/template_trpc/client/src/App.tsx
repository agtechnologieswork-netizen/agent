import './App.css';
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
    <div>
      <div className="gradient"></div>
      <div className="grid"></div>
      <div className="container">
        <h1 className="title">tRPC Template</h1>
        <p className="description">
          Your tRPC app is running!
        </p>
        {health && (
          <div style={{ marginTop: '2rem', padding: '1rem', background: 'rgba(0,255,0,0.1)', borderRadius: '8px' }}>
            <p>✓ Server Status: {health.status}</p>
            <p>Timestamp: {health.timestamp}</p>
          </div>
        )}
        {error && (
          <div style={{ marginTop: '2rem', padding: '1rem', background: 'rgba(255,0,0,0.1)', borderRadius: '8px' }}>
            <p>✗ Error: {error}</p>
          </div>
        )}
        <footer className="footer">
          Built with ❤️ by{" "}
          <a href="https://app.build" target="_blank" className="footer-link">
            app.build
          </a>
        </footer>
      </div>
    </div>
  );
}

export default App;
