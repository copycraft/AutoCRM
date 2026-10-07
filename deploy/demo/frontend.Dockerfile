# The web app for the demo stack. Build context: the repository root.
FROM node:20-bookworm-slim AS build
WORKDIR /app
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
# Both are baked in at build time: /api is proxied to the backend container, and images
# are allowed from the public file-storage address.
ARG AUTOCRM_API_URL=http://backend:8080
ARG AUTOCRM_S3_URL
ENV AUTOCRM_API_URL=$AUTOCRM_API_URL AUTOCRM_S3_URL=$AUTOCRM_S3_URL NEXT_TELEMETRY_DISABLED=1
RUN npm run build

FROM node:20-bookworm-slim
WORKDIR /app
ENV NODE_ENV=production PORT=3000 HOSTNAME=:: NEXT_TELEMETRY_DISABLED=1
COPY --from=build /app/.next/standalone ./
COPY --from=build /app/.next/static ./.next/static
USER node
EXPOSE 3000
CMD ["node", "server.js"]
