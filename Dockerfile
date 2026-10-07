# The whole game in one container, built from source: the arena server,
# serving the page too. (CI ships a prebuilt image instead: scripts/ship.sh.)
FROM rust:1-bookworm AS build
RUN rustup target add wasm32-unknown-unknown x86_64-unknown-linux-musl \
 && cargo install wasm-bindgen-cli --version 0.2.129 --locked
WORKDIR /src
COPY . .
RUN bash scripts/build-web.sh \
 && cargo build -p secretspace-server --release --target x86_64-unknown-linux-musl

FROM scratch
COPY --from=build /src/target/x86_64-unknown-linux-musl/release/server /server
COPY --from=build /src/dist /dist
ENV PORT=8787
ENV DATA_DIR=/data
EXPOSE 8787
CMD ["/server", "--static", "/dist"]
