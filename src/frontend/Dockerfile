FROM node:20-alpine
LABEL com.azure.containerizationassist.createdby="containerization-assist"
WORKDIR /app
COPY package*.json ./
RUN npm ci --omit=dev
COPY . .
EXPOSE 3000
USER node
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD wget --spider -q "http://127.0.0.1:${PORT:-3000}/auth/providers" || exit 1
CMD ["node", "server.js"]
