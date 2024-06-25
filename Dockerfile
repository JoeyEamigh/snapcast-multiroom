# builder image
FROM rust:alpine as builder

RUN apk add --no-cache musl-dev

WORKDIR /app

COPY . .

RUN cargo build --release
# end builder image

# runtime image
FROM alpine

ARG VERSION

LABEL org.opencontainers.image.version $VERSION
LABEL org.opencontainers.image.title snapcast-multiroom
LABEL org.opencontainers.image.description "A tool to manage multiple Snapcast clients in a multiroom setup."
LABEL org.opencontainers.image.authors "Joey Eamigh"
LABEL org.opencontainers.image.source https://github.com/JoeyEamigh/snapcast-multiroom

WORKDIR /app

COPY --from=builder /app/target/release/snapcast-multiroom .

CMD ["./snapcast-multiroom"]
# end runtime image