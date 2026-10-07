# The API for the demo stack. Build context: the repository root.
FROM rust:1-bookworm AS build
WORKDIR /src
COPY backend/ ./
# Queries are checked against the committed .sqlx metadata; no database at build time.
ENV SQLX_OFFLINE=true
RUN cargo build --release --bins

FROM debian:bookworm-slim
# heif-dec turns iPhone HEIC photos into JPEG previews (HEIC_CONVERTER).
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libheif-examples tzdata \
 && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/autocrm /usr/local/bin/autocrm
USER nobody
EXPOSE 8080
CMD ["autocrm", "serve"]
