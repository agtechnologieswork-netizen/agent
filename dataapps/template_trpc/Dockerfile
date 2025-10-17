# Multi-stage Dockerfile for tRPC template
FROM node:20-alpine AS builder

WORKDIR /app

# Copy all package.json files
COPY client/package*.json ./client/
COPY server/package*.json ./server/

# Install all dependencies with BuildKit cache mount
RUN --mount=type=cache,target=/root/.npm \
    cd client && npm ci

RUN --mount=type=cache,target=/root/.npm \
    cd server && npm ci

# Copy server source (needed for client build due to tRPC types)
COPY server/ ./server/

# Copy client source and build
COPY client/ ./client/
RUN cd client && npm run build

# Stage 2: Production image
FROM node:20-alpine

# Install curl for healthcheck
RUN apk add --no-cache curl

WORKDIR /app

# Install production dependencies only with BuildKit cache mount
COPY server/package*.json ./server/
RUN --mount=type=cache,target=/root/.npm \
    cd server && npm ci --omit=dev

# Copy server source
COPY server/ ./server/

# Copy built client from builder stage
RUN mkdir -p server/public
COPY --from=builder /app/client/dist/ ./server/public/

# Set working directory to server
WORKDIR /app/server

# Expose port
EXPOSE 8000

# Start the application
CMD ["npm", "start"]
