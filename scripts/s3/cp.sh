CREDENTIALS_FILE="$HOME/hyprnote-r2.toml"
ENDPOINT_URL="https://45207bf426d0acf3baf9f70fd2c610c8.r2.cloudflarestorage.com"
BUCKET_FROM="hyprnote-cache"
BUCKET_TO="anarlog-cache"

AWS_REGION=auto s5cmd \
    --log trace \
    --credentials-file "$CREDENTIALS_FILE" \
    --endpoint-url "$ENDPOINT_URL" \
    cp "s3://$BUCKET_FROM/*" "s3://$BUCKET_TO/"
