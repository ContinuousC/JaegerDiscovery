# syntax=docker/dockerfile:1.6

FROM gitea.contc/controlplane/rust-builder:0.2.0 as source
ARG GITVERSION=
WORKDIR /root/source/jaeger-discovery
COPY --link . /root/source/jaeger-discovery

FROM source as test
RUN --mount=type=ssh,required=true \
    --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/source/jaeger-discovery/target \
    RUST_BACKTRACE=full /root/.cargo/bin/cargo test

FROM source as audit
RUN --mount=type=ssh,required=true \
    --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/source/jaeger-discovery/target \
    /root/.cargo/bin/cargo audit --color=always

FROM source as build-dev
RUN --mount=type=ssh,required=true \
    --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/source/jaeger-discovery/target \
    /root/.cargo/bin/cargo build --bin jaeger-discovery \
    && cp target/debug/jaeger-discovery .

FROM source as build-release
RUN --mount=type=ssh,required=true \
    --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/source/jaeger-discovery/target \
    /root/.cargo/bin/cargo build --release --bin jaeger-discovery \
    && cp target/release/jaeger-discovery .

FROM ubuntu:24.04 as image-dev
COPY --from=build-dev \
    /root/source/jaeger-discovery/jaeger-discovery \
    /usr/bin/
EXPOSE 9999
CMD /usr/bin/jaeger-discovery

FROM ubuntu:24.04 as image-release
COPY --from=build-release \
    /root/source/jaeger-discovery/jaeger-discovery \
    /usr/bin/
EXPOSE 9999
CMD /usr/bin/jaeger-discovery
