# downloaders/pikpak/Dockerfile
FROM rust:1.82-slim AS builder

RUN apt-get update && apt-get install -y pkg-config && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace

# Copy workspace config
COPY Cargo.toml Cargo.lock ./

# Copy all crates (needed for workspace resolver)
COPY shared ./shared
COPY downloaders/pikpak ./downloaders/pikpak
COPY core-service/Cargo.toml ./core-service/Cargo.toml
COPY fetchers/mikanani/Cargo.toml ./fetchers/mikanani/Cargo.toml
COPY downloaders/qbittorrent/Cargo.toml ./downloaders/qbittorrent/Cargo.toml
COPY viewers/jellyfin/Cargo.toml ./viewers/jellyfin/Cargo.toml
COPY metadata/Cargo.toml ./metadata/Cargo.toml
COPY cli/Cargo.toml ./cli/Cargo.toml

# Create stub src for other workspace members so cargo can resolve
RUN for dir in core-service fetchers/mikanani downloaders/qbittorrent viewers/jellyfin metadata cli; do \
    mkdir -p $dir/src && \
    echo "fn main() {}" > $dir/src/main.rs && \
    touch $dir/src/lib.rs; \
    done

RUN cargo build --release -p downloader-pikpak

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /data
WORKDIR /app
COPY --from=builder /workspace/target/release/downloader-pikpak .

EXPOSE 8006
VOLUME ["/data"]

CMD ["./downloader-pikpak"]
