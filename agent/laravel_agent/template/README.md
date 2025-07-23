# Welcome to your ✨ Laravel app!

_Built by [app.build](https://www.app.build)_

## Deploy it to [Laravel Cloud](https://cloud.laravel.com/)

[Laravel Cloud](https://cloud.laravel.com/) is the only fully managed platform-as-a-service (PaaS) obsessively optimized for shipping and scaling Laravel applications:

- No application or server configuration necessary.
- Fully managed databases.
- Zero downtime deployments and scaling.
- Automatic TLS and load balancing.
- Cloud Edge Network.
- Monitoring and logs.

### Sign up there and follow the [quickstart](https://cloud.laravel.com/docs/quickstart):

- Connect your GitHub account
- Grant access to your repo (this repo)
- Setup environment variables (like your database URL)

## GitHub Actions CI/CD

To set up continuous integration and deployment with GitHub Actions, create a `.github/workflows/tests.yml` file in your repository with the following configuration:

```yaml
name: tests

on:
  push:
    branches:
      - develop
      - main
  pull_request:
    branches:
      - develop
      - main

jobs:
  ci:
    runs-on: ubuntu-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup PHP
        uses: shivammathur/setup-php@v2
        with:
          php-version: 8.4
          tools: composer:v2
          coverage: xdebug

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '22'
          cache: 'npm'

      - name: Install Node Dependencies
        run: npm ci

      - name: Build Assets
        run: npm run build

      - name: Install Dependencies
        run: composer install --no-interaction --prefer-dist --optimize-autoloader

      - name: Copy Environment File
        run: cp .env.example .env

      - name: Generate Application Key
        run: php artisan key:generate

      - name: Tests
        run: composer test
```

Create a `.github/workflows/lint.yml` file in your repository with the following configuration:

```yaml
name: linter

on:
  push:
    branches:
      - develop
      - main
  pull_request:
    branches:
      - develop
      - main

permissions:
  contents: write

jobs:
  quality:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup PHP
        uses: shivammathur/setup-php@v2
        with:
          php-version: '8.4'

      - name: Install Dependencies
        run: |
          composer install -q --no-ansi --no-interaction --no-scripts --no-progress --prefer-dist
          npm install

      - name: Build assets
        run: npm run build

      - name: Format Laravel
        run: composer format

      - name: Lint Laravel
        run: composer ci

      - name: Format Frontend
        run: npm run format

      - name: Lint Frontend
        run: npm run lint

