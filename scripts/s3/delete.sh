CREDENTIALS_FILE="$HOME/hyprnote-r2.toml"
ENDPOINT_URL="https://45207bf426d0acf3baf9f70fd2c610c8.r2.cloudflarestorage.com"
BUCKET="hyprnote-cache"

TARGET="quantized-whisper-large-v3-turbo/*"

AWS_REGION=auto s5cmd \
    --credentials-file "$CREDENTIALS_FILE" \
    --endpoint-url "$ENDPOINT_URL" \
    rm "s3://$BUCKET/$TARGET"
