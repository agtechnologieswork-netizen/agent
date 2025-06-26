import React from 'react';
import { createInertiaApp } from '@inertiajs/react';
import { resolvePageComponent } from 'laravel-vite-plugin/inertia-helpers';
import { renderToString } from 'react-dom/server';

const appName = import.meta.env.VITE_APP_NAME || 'Laravel';

createInertiaApp({
    page: window.page,
    render: renderToString,
    title: (title) => `${title} - ${appName}`,
    resolve: (name) =>
        resolvePageComponent(
            `./Pages/${name}.tsx`,
            import.meta.glob('./Pages/**/*.tsx'),
        ),
    setup: ({ App, props }) => <App {...props} />,
});
