ARG RUST_IMAGE=rust
ARG ALPINE_VER=3.22
ARG RUST_VER=1.88-alpine${ALPINE_VER}

FROM ${RUST_IMAGE}:${RUST_VER} as builder

ARG SERVICE_NAME
ARG BUILD_PROFILE
ARG TARGET="x86_64-unknown-linux-musl"

ENV RUSTFLAGS="-C target-feature=+crt-static"
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

WORKDIR /src
COPY . .

RUN apk add --no-cache musl-dev perl g++ alpine-sdk

RUN rustup target add ${TARGET}
RUN cargo build --profile=${BUILD_PROFILE} --target=${TARGET} --bin ${SERVICE_NAME}

FROM alpine:${ALPINE_VER} as runtime

ARG SERVICE_NAME
ARG BUILD_PROFILE

RUN addgroup -g 101 app && \
    adduser -H -u 101 -G app -s /bin/sh -D app && \
    apk update --no-cache -X alpine/v${ALPINE_VER}/main && \
    apk upgrade --no-cache -X alpine/v${ALPINE_VER}/main

WORKDIR /app/

COPY --from=builder --chown=app:app /src/target/x86_64-unknown-linux-musl/${BUILD_PROFILE}/${SERVICE_NAME} .
COPY --chown=app:app config.toml config.toml

USER app

# https://stackoverflow.com/questions/35560894/is-docker-arg-allowed-within-cmd-instruction/35562189#35562189
ENV APP_BIN="/app/${SERVICE_NAME}"

CMD sh -c ${APP_BIN}