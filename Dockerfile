FROM rust:1.99.0-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY assets assets
RUN cargo build --release --locked
# `scratch` has no shell to create it.
RUN mkdir /data

FROM scratch
COPY --from=build /src/target/release/open-parks /open-parks
COPY --from=build --chown=65534:65534 /data /data
ENV OPEN_PARKS_ADDRESS=0.0.0.0:3000
ENV OPEN_PARKS_DATABASE=/data/open-parks.sqlite3
EXPOSE 3000
USER 65534:65534
ENTRYPOINT ["/open-parks"]
