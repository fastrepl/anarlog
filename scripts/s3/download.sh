CREDENTIALS_FILE="$HOME/hyprnote-r2.toml"
ENDPOINT_URL="https://45207bf426d0acf3baf9f70fd2c610c8.r2.cloudflarestorage.com"
BUCKET="anarlog-cache"

FROM_PATH="s3://$BUCKET/v0/*"
TO_PATH="/Users/yujonglee/dev/anarlog/.cache/"

AWS_REGION=auto s5cmd \
    --log trace \
    --credentials-file "$CREDENTIALS_FILE" \
    --endpoint-url "$ENDPOINT_URL" \
    cp "$FROM_PATH" "$TO_PATH"
