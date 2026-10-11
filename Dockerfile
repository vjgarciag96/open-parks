FROM rust:1.99.0-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
RUN cargo build --release --locked

FROM scratch
COPY --from=build /src/target/release/open-parks /open-parks
ENV OPEN_PARKS_ADDRESS=0.0.0.0:3000
EXPOSE 3000
USER 65534:65534
ENTRYPOINT ["/open-parks"]
