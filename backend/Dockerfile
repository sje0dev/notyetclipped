FROM rust:1.98-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ffmpeg chromium python3-venv ca-certificates \
    && python3 -m venv /opt/streamlink \
    && /opt/streamlink/bin/pip install --no-cache-dir streamlink==8.6.2 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 clipper \
    && mkdir /data && chown clipper:clipper /data
ENV PATH="/opt/streamlink/bin:$PATH" DATA_DIR=/data API_BIND=0.0.0.0:8000
COPY --from=build /app/target/release/notyetclipped /usr/local/bin/notyetclipped
USER clipper
WORKDIR /app
EXPOSE 8000
ENTRYPOINT ["notyetclipped"]
CMD ["api"]
