#!/bin/bash

stable_user_id=""
stable_version=""

if [ -d "$HOME/Library/Application Support/hyprnote" ]; then
    if [ -f "$HOME/Library/Application Support/hyprnote/store.json" ]; then
        stable_user_id=$(jq -r '."auth-user-id" // empty' "$HOME/Library/Application Support/hyprnote/store.json")
    fi
fi

if [ -d "/Applications/Char.app" ]; then
    stable_version=$(defaults read /Applications/Char.app/Contents/Info.plist CFBundleShortVersionString 2>/dev/null || echo "")
elif [ -d "/Applications/Hyprnote.app" ]; then
    stable_version=$(defaults read /Applications/Hyprnote.app/Contents/Info.plist CFBundleShortVersionString 2>/dev/null || echo "")
fi

cat << EOF
{
    "stable": {
        "userId": "${stable_user_id}",
        "version": "${stable_version}"
    }
}
EOF
