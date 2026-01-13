# syntax=docker/dockerfile:1.6

FROM node:22-slim AS web-deps
WORKDIR /app/web
ENV CI=true
RUN corepack enable
COPY web/package.json web/pnpm-lock.yaml web/pnpm-workspace.yaml web/.npmrc ./
RUN pnpm install --frozen-lockfile

FROM web-deps AS web-build
WORKDIR /app/web
COPY web/ ./
RUN pnpm build

FROM rust:1.92-slim AS chef
WORKDIR /app
RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    pkg-config \
    libsqlite3-dev \
  && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked

FROM chef AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build -p bctn-backend --release

FROM gcr.io/distroless/cc-debian13:nonroot AS runtime
WORKDIR /app

COPY --from=builder /app/target/release/bctn /app/bctn
COPY --from=builder /usr/lib/x86_64-linux-gnu/libsqlite3.so.0* /usr/lib/x86_64-linux-gnu/
COPY --from=web-build /app/web/build /app/web/build
COPY data/bctn.sqlite /app/data/bctn.sqlite

ENV BCTN_STATIC_DIR=/app/web/build
ENV BCTN_NAMES_DB=/app/data/bctn.sqlite
ENV BCTN_APP_DB=/data/bctn_app.sqlite
ENV BCTN_COOKIE_SECURE=false
ENV RUST_LOG=info,tower_http=info,sqlx=warn

EXPOSE 2286
ENTRYPOINT ["/app/bctn"]
