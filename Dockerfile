ARG ARCH_TAG="aarch64-musl"
ARG S6_OVERLAY_INSTALLER="https://github.com/just-containers/s6-overlay/releases/download/v2.2.0.3/s6-overlay-aarch64-installer"

FROM messense/rust-musl-cross:${ARCH_TAG} as build

ADD ./server /home/rust/src/

RUN cd /home/rust/src && \
    cargo build --release && \
    find ./target -name "twitter_comic_streamer" -exec musl-strip {} \; 

FROM debian:buster as run
ARG S6_OVERLAY_INSTALLER

RUN mkdir -p /app /app/server /app/classifier

RUN apt-get update && \
    apt-get install -y curl gnupg && \
    echo "deb https://packages.cloud.google.com/apt coral-edgetpu-stable main" >/etc/apt/sources.list.d/coral-edgetpu.list && \
    curl https://packages.cloud.google.com/apt/doc/apt-key.gpg | apt-key add - && \
    apt-get update && \
    apt-get install -y python3-tflite-runtime && \
    apt-get install -y python3-pip && \
    apt-get install -y libz-dev libjpeg-dev && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

RUN pip3 install flask pillow

ADD ./classifier/* /app/classifier/

ADD ${S6_OVERLAY_INSTALLER} /tmp/
RUN chmod u+x /tmp/s6-overlay-*-installer && \
    /tmp/s6-overlay-*-installer / && \
    rm -rf /tmp/s6-overlay-*-installer

ADD s6/ /etc

# Copy over this late, to properly use cache and parallel building
COPY --from=build /home/rust/src/target/*/release/twitter_comic_streamer /app/server/twitter_comic_streamer

## Needs to be set to run the container
# ENV CONSUMER_KEY
# ENV CONSUMER_SECRET
# ENV ACCESS_TOKEN
# ENV ACCESS_TOKEN_SECRET

ENV TWITTER_USERNAMES=daskritzelt,erzaehlmirnix,islieb,isfies666,joschasauer,foxes_in_love,hauckundbauer
ENV TWITTER_REFRESH_INTERVAL=600
ENV HTTP_CLASSIFIER_URL="http://127.0.0.1:5000/classify"
ENV ROCKET_ADDRESS="0.0.0.0"

ENTRYPOINT [ "/init" ]
