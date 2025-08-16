FROM ubuntu:latest

# Install dependencies in one layer to reduce image size
RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y \
    curl \
    wget \
    mosquitto \
    mosquitto-clients \
    nodejs \
    npm \
    iproute2 \
    && npm install -g azurite \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Download and install latest device-monitor from deb package
RUN LATEST_RELEASE=$(curl -s https://api.github.com/repos/JSeimizu/device-monitor/releases/latest | grep -o '"tag_name": "[^"]*' | grep -o '[^"]*$') && \
    wget "https://github.com/JSeimizu/device-monitor/releases/download/${LATEST_RELEASE}/device-monitor_${LATEST_RELEASE#v}_amd64.deb" && \
    dpkg -i "device-monitor_${LATEST_RELEASE#v}_amd64.deb" && \
    rm "device-monitor_${LATEST_RELEASE#v}_amd64.deb"

# Copy and setup startup script
COPY start.sh /usr/local/bin/start.sh
RUN chmod +x /usr/local/bin/start.sh

# Expose ports
EXPOSE 1883 10000 10001 10002

# Set the startup script as entrypoint
ENTRYPOINT ["/usr/local/bin/start.sh"]