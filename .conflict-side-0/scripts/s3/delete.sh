CREDENTIALS_FILE="$HOME/hyprnote-r2.toml"
ENDPOINT_URL="https://3db5267cdeb5f79263ede3ec58090fe0.r2.cloudflarestorage.com"
BUCKET="hyprnote-cache"

TARGET="quantized-whisper-large-v3-turbo/*"

AWS_REGION=auto s5cmd \
    --credentials-file "$CREDENTIALS_FILE" \
    --endpoint-url "$ENDPOINT_URL" \
    rm "s3://$BUCKET/$TARGET"
