#!/bin/sh

# Exit on any error
set -e

echo "Starting Laravel application..."

# -----------------------------------------------------------------------------
# Ensure an application encryption key exists. A missing APP_KEY results in a
# 500 "Application key set error" response which also breaks the container
# health-check. If the key isn’t provided via environment or pre-baked .env
# file we generate one on the fly and export it so every subsequent artisan
# command – and the processes started by supervisord – can access it.
# -----------------------------------------------------------------------------

if [ -z "$APP_KEY" ]; then
    echo "APP_KEY is not set – generating a new one…"
    # php artisan key:generate --show prints a fresh key without touching .env
    # We export that value so PHP-FPM picks it up via the parent environment.
    export APP_KEY=$(php /var/www/html/artisan key:generate --show)
    echo "Generated APP_KEY=$APP_KEY"
fi

# Wait for database to be ready
echo "Waiting for database connection..."
until php /var/www/html/artisan tinker --execute="DB::connection()->getPdo(); echo 'Database connected';" > /dev/null 2>&1; do
    echo "Database not ready, waiting..."
    sleep 2
done

# Run database migrations
echo "Running database migrations..."
php /var/www/html/artisan migrate --force

# Clear and cache configuration for production
echo "Optimizing Laravel for production..."
php /var/www/html/artisan config:cache
php /var/www/html/artisan route:cache
php /var/www/html/artisan view:cache

echo "Starting services via supervisor..."

# Start supervisor (which manages nginx and php-fpm)
exec /usr/bin/supervisord -c /etc/supervisor/conf.d/supervisord.conf 