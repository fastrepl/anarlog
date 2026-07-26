FROM caddy:2.11.1-alpine

# Render does not permit executing the Caddy binary with bundled file capabilities.
# We do not need those optional capabilities for this OTEL ingest edge.
RUN setcap -r /usr/bin/caddy

COPY otel/caddy/Caddyfile /etc/caddy/Caddyfile
