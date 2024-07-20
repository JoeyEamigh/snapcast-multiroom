# kichen image
FROM rust:alpine AS kitchen

RUN apk add --no-cache musl-dev
RUN cargo install cargo-chef --locked

WORKDIR /app
# end kichen image

# planner image
FROM kitchen AS planner

COPY . .

RUN cargo chef prepare --recipe-path recipe.json
# end planner image

# builder image
FROM kitchen AS builder

COPY --from=planner /app/recipe.json recipe.json
COPY snapcast-control snapcast-control

RUN cargo chef cook --release --recipe-path recipe.json

COPY . .

RUN cargo build --release
# end builder image

# runtime image
FROM alpine

ARG VERSION

LABEL org.opencontainers.image.version=$VERSION
LABEL org.opencontainers.image.title=snapcast-multiroom
LABEL org.opencontainers.image.description="A tool to manage multiple Snapcast clients in a multiroom setup."
LABEL org.opencontainers.image.authors="Joey Eamigh"
LABEL org.opencontainers.image.source=https://github.com/JoeyEamigh/snapcast-multiroom

WORKDIR /app

COPY --from=builder /app/target/release/snapcast-multiroom .

CMD ["./snapcast-multiroom"]
# end runtime image