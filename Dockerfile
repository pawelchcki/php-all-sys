FROM debian:jessie AS base
RUN set -xe; \
    apt-get update; 
RUN set -xe; \
    apt-get install -y php5-cli;
RUN apt-get install -y valgrind

    # podman run -v $(pwd):/host -it tmp php -i