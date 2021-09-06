ARG ARCH_TAG="armv7-musleabihf"
ARG S6_OVERLAY_INSTALLER="https://github.com/just-containers/s6-overlay/releases/download/v2.2.0.3/s6-overlay-armhf-installer"

FROM messense/rust-musl-cross:${ARCH_TAG} as build

ADD ./server /home/rust/src/

RUN cd /home/rust/src && \
    cargo build --release && \
    find ./target -name "twitter_comic_streamer" -exec musl-strip {} \; 

FROM alpine as run
ARG S6_OVERLAY_INSTALLER

RUN mkdir /app
ADD ./server/twitter.env.sh /app/twitter.env.sh
COPY --from=build /home/rust/src/target/*/release/twitter_comic_streamer /app/twitter_comic_streamer

ADD ${S6_OVERLAY_INSTALLER} /tmp/
RUN chmod u+x /tmp/s6-overlay-*-installer && \
    /tmp/s6-overlay-*-installer / && \
    rm -rf /tmp/s6-overlay-*-installer

ADD s6/ /etc

## Needs to be set to run the container
# ENV CONSUMER_KEY
# ENV CONSUMER_SECRET
# ENV ACCESS_TOKEN
# ENV ACCESS_TOKEN_SECRET

ENV TWITTER_USERNAMES=daskritzelt,erzaehlmirnix,islieb,isfies666,joschasauer,foxes_in_love,hauckundbauer
ENV TWITTER_REFRESH_INTERVAL=600
ENV ROCKET_ADDRESS="0.0.0.0"

ENTRYPOINT [ "/init" ]
