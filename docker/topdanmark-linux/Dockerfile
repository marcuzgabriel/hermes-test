FROM node:22-bookworm

ENV DEBIAN_FRONTEND=noninteractive
ENV PATH="/root/.cargo/bin:/root/.bun/bin:${PATH}"

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    cmake \
    curl \
    git \
    libicu-dev \
    ninja-build \
    pkg-config \
    python3 \
    xz-utils \
  && rm -rf /var/lib/apt/lists/*

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal
RUN curl -fsSL https://bun.sh/install | bash

WORKDIR /work/hermes-test
