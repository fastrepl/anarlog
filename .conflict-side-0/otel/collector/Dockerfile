FROM otel/opentelemetry-collector-contrib:0.139.0

COPY otel/collector/config.yaml /etc/otelcol-contrib/config.yaml

CMD ["--config", "/etc/otelcol-contrib/config.yaml"]
