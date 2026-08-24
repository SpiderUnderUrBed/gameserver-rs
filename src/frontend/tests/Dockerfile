# TODO: consider if a dockerfile for frontend tests is nessesary

FROM node:22-alpine as builder
WORKDIR /app
COPY . .
RUN npm ci

FROM builder as test
WORKDIR /app
COPY --from=builder /app /app
RUN npm run test
