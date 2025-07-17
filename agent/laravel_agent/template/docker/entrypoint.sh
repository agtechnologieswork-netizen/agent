#!/bin/sh

# Exit on any error
set -e

echo "Starting Laravel application..."

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