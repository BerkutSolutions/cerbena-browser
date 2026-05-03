FROM golang:1.24-bookworm AS builder

ARG AMNEZIAWG_GO_REF=master
ARG AMNEZIAWG_TOOLS_REF=v1.0.20260223
ARG SING_BOX_REF=v1.12.0

RUN apt-get update \
    && apt-get install -y --no-install-recommends git make gcc libc6-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

RUN git clone --depth 1 --branch ${AMNEZIAWG_GO_REF} https://github.com/amnezia-vpn/amneziawg-go.git /src/amneziawg-go \
    && cd /src/amneziawg-go \
    && CGO_ENABLED=0 go build -trimpath -ldflags='-s -w' -o /out/amneziawg-go .

RUN git clone --depth 1 --branch ${AMNEZIAWG_TOOLS_REF} https://github.com/amnezia-vpn/amneziawg-tools.git /src/amneziawg-tools \
    && make -C /src/amneziawg-tools/src WITH_BASHCOMPLETION=no WITH_SYSTEMDUNITS=no WITH_WGQUICK=yes PREFIX=/usr/local install

RUN git clone --depth 1 --branch ${SING_BOX_REF} https://github.com/SagerNet/sing-box.git /src/sing-box \
    && cd /src/sing-box/cmd/sing-box \
    && CGO_ENABLED=0 go build -tags with_utls -trimpath -ldflags='-s -w' -o /out/sing-box .

COPY container_socks_proxy.go /src/container_socks_proxy.go
RUN CGO_ENABLED=0 go build -trimpath -ldflags='-s -w' -o /out/container-socks-proxy /src/container_socks_proxy.go

COPY entrypoint.sh /src/entrypoint.sh
COPY sysctl_wrapper.sh /src/sysctl_wrapper.sh
COPY openvpn_dns_sync.sh /src/openvpn_dns_sync.sh
RUN chmod +x /src/entrypoint.sh /src/openvpn_dns_sync.sh

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends bash iproute2 iptables ca-certificates procps openvpn tor obfs4proxy snowflake-client \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /out/amneziawg-go /usr/local/bin/amneziawg-go
COPY --from=builder /out/sing-box /usr/local/bin/sing-box
COPY --from=builder /usr/local/bin/awg /usr/local/bin/awg
COPY --from=builder /usr/local/bin/awg-quick /usr/local/bin/awg-quick
COPY --from=builder /out/container-socks-proxy /usr/local/bin/container-socks-proxy
COPY --from=builder /src/entrypoint.sh /usr/local/bin/cerbena-container-entrypoint
COPY --from=builder /src/openvpn_dns_sync.sh /usr/local/bin/openvpn-dns-sync
COPY --from=builder /src/sysctl_wrapper.sh /usr/local/bin/sysctl

RUN chmod +x /usr/local/bin/amneziawg-go \
    /usr/local/bin/sing-box \
    /usr/local/bin/awg \
    /usr/local/bin/awg-quick \
    /usr/local/bin/sysctl \
    /usr/local/bin/container-socks-proxy \
    /usr/local/bin/cerbena-container-entrypoint \
    /usr/local/bin/openvpn-dns-sync

ENV WG_QUICK_USERSPACE_IMPLEMENTATION=/usr/local/bin/amneziawg-go

ENTRYPOINT ["/usr/local/bin/cerbena-container-entrypoint"]
