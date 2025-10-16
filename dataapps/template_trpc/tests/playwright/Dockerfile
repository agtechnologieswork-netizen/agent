FROM mcr.microsoft.com/playwright:v1.40.0-jammy

WORKDIR /tests

# copy package files
COPY package.json package-lock.json* ./

# install dependencies and browsers (cached layer)
RUN npm install && npx playwright install chromium

# copy test files (this layer changes frequently, so put it last)
COPY . .

CMD ["npx", "playwright", "test"]
