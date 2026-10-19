FROM debian:bookworm-slim AS whisper
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl cmake g++ make
WORKDIR /src
RUN curl -fL https://github.com/ggml-org/whisper.cpp/archive/refs/tags/v1.9.5.tar.gz | tar xz --strip-components=1 \
    && cmake -B build -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF -DGGML_NATIVE=OFF -DWHISPER_BUILD_TESTS=OFF -DWHISPER_CURL=OFF \
    && cmake --build build --target whisper-cli -j 2 \
    && bash models/download-ggml-model.sh base

FROM rust:1.98-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ffmpeg chromium python3-venv ca-certificates libgomp1 \
    && python3 -m venv /opt/streamlink \
    && /opt/streamlink/bin/pip install --no-cache-dir streamlink==8.6.2 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 clipper \
    && mkdir /data && chown clipper:clipper /data
ENV PATH="/opt/streamlink/bin:$PATH" DATA_DIR=/data API_BIND=0.0.0.0:8000
COPY --from=build /app/target/release/notyetclipped /usr/local/bin/notyetclipped
COPY --from=whisper /src/build/bin/whisper-cli /usr/local/bin/whisper-cli
COPY --from=whisper /src/models/ggml-base.bin /opt/whisper/ggml-base.bin
USER clipper
WORKDIR /app
EXPOSE 8000
ENTRYPOINT ["notyetclipped"]
CMD ["api"]
