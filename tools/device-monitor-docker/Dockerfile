FROM ubuntu:24.04

# Install dependencies in one layer to reduce image size
RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      ca-certificates \
      curl \
      wget \
      mosquitto \
      mosquitto-clients \
      nodejs \
      npm \
      iproute2 \
      bash \
      procps \
    && npm install -g azurite@3.31.0 \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Allow pinning of device-monitor version and checksum at build time
ARG DEVICE_MONITOR_VERSION=""
ARG DEVICE_MONITOR_SHA256=""

# Download and install device-monitor from deb package (versioned if provided)
RUN set -euo pipefail; \
    if [ -n "$DEVICE_MONITOR_VERSION" ]; then \
      DEB_VER="$DEVICE_MONITOR_VERSION"; \
    else \
      LATEST_RELEASE=$(curl -fsSL https://api.github.com/repos/JSeimizu/device-monitor/releases/latest | grep -o '"tag_name": "[^\"]*' | grep -o '[^\"]*$'); \
      DEB_VER="${LATEST_RELEASE#v}"; \
    fi; \
    DEB_FILE="device-monitor_${DEB_VER}_amd64.deb"; \
    wget -q "https://github.com/JSeimizu/device-monitor/releases/download/v${DEB_VER}/${DEB_FILE}"; \
    if [ -n "$DEVICE_MONITOR_SHA256" ]; then \
      echo "$DEVICE_MONITOR_SHA256  ${DEB_FILE}" | sha256sum -c -; \
    fi; \
    dpkg -i "$DEB_FILE"; \
    rm -f "$DEB_FILE"

# Copy and setup startup script
COPY start.sh /usr/local/bin/start.sh
RUN chmod +x /usr/local/bin/start.sh

# Copy healthcheck script
COPY healthcheck.sh /usr/local/bin/healthcheck.sh
RUN chmod +x /usr/local/bin/healthcheck.sh

# Create non-root user and work directory
RUN useradd -m -u 10001 -s /usr/sbin/nologin device \
    && mkdir -p /work \
    && chown -R device:device /work

# Expose ports
EXPOSE 1883 10000 10001 10002

# Add a healthcheck that respects internal service flags
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=5 CMD /usr/local/bin/healthcheck.sh

# Set the startup script as entrypoint
ENTRYPOINT ["/usr/local/bin/start.sh"]

