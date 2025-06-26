<?php

use Illuminate\Support\Str;

return [
    'default' => env('CACHE_STORE', 'database'),
    'stores' => [
        'array' => [
            'driver' => 'array',
            'serialize' => false,
        ],
        'database' => [
            'driver' => 'database',
            'table' => env('DB_CACHE_TABLE', 'cache'),
            'connection' => env('DB_CACHE_CONNECTION'),
            'lock_connection' => env('DB_CACHE_LOCK_CONNECTION'),
        ],
        'file' => [
            'driver' => 'file',
            'path' => env('CACHE_FILE_PATH', storage_path('framework/cache/data')),
            'lock_path' => env('CACHE_LOCK_PATH', storage_path('framework/cache/data')),
        ],
        'redis' => [
            'driver' => 'redis',
            'connection' => env('CACHE_REDIS_CONNECTION', 'cache'),
            'lock_connection' => env('CACHE_REDIS_LOCK_CONNECTION', 'default'),
        ],
    ],
    'prefix' => env('CACHE_PREFIX', Str::slug(env('APP_NAME', 'laravel'), '_').'_cache_'),
];
