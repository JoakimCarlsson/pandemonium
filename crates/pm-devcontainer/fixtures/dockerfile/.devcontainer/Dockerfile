FROM python:3.12-slim-trixie

ARG FIXTURE_USER=dev

RUN apt-get update \
    && apt-get install -y --no-install-recommends git ca-certificates curl procps \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --shell /bin/bash "$FIXTURE_USER"

USER $FIXTURE_USER
ENV PATH="/home/$FIXTURE_USER/.local/bin:$PATH"
