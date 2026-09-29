# Multi-stage: TypeScript is compiled in a throwaway stage, and the final image
# carries only the compiled JavaScript and the production dependencies.

FROM node:22-alpine AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --omit=dev --no-audit --no-fund

FROM node:22-alpine AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY tsconfig.json ./
COPY src ./src
RUN npm run build

FROM node:22-alpine AS runtime
ENV NODE_ENV=production
# Inside the container, listen on its own interfaces so the published port reaches it.
# Outside Docker the default is 127.0.0.1. Either way, SIDECAR_TOKEN guards every route.
ENV HOST=0.0.0.0
WORKDIR /app

COPY --from=deps /app/node_modules ./node_modules
COPY --from=build /app/dist ./dist
COPY package.json ./

# Never as root. The invoice data that reaches the PDF renderer is input, and
# the image has no reason to be able to write to itself.
USER node

EXPOSE 8080

# No credentials are used and NAV is not contacted, so this is safe to poll.
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD node -e "fetch('http://127.0.0.1:'+(process.env.PORT||8080)+'/health').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"

CMD ["node", "dist/index.js"]
