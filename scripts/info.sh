#!/bin/bash

stable_user_id=""
nightly_user_id=""
stable_version=""
nightly_version=""

if [ -d "$HOME/Library/Application Support/char" ]; then
    if [ -f "$HOME/Library/Application Support/char/store.json" ]; then
        stable_user_id=$(jq -r '."auth-user-id" // empty' "$HOME/Library/Application Support/char/store.json")
    fi
fi

if [ -d "$HOME/Library/Application Support/char-nightly" ]; then
    if [ -f "$HOME/Library/Application Support/char-nightly/store.json" ]; then
        nightly_user_id=$(jq -r '."auth-user-id" // empty' "$HOME/Library/Application Support/char-nightly/store.json")
    fi
fi

if [ -d "/Applications/Char.app" ]; then
    stable_version=$(defaults read /Applications/Char.app/Contents/Info.plist CFBundleShortVersionString 2>/dev/null || echo "")
elif [ -d "/Applications/Char.app" ]; then
    stable_version=$(defaults read /Applications/Char.app/Contents/Info.plist CFBundleShortVersionString 2>/dev/null || echo "")
fi

if [ -d "/Applications/Char Nightly.app" ]; then
    nightly_version=$(defaults read "/Applications/Char Nightly.app/Contents/Info.plist" CFBundleShortVersionString 2>/dev/null || echo "")
elif [ -d "/Applications/Char Nightly.app" ]; then
    nightly_version=$(defaults read "/Applications/Char Nightly.app/Contents/Info.plist" CFBundleShortVersionString 2>/dev/null || echo "")
fi

cat << EOF
{
    "stable": {
        "userId": "${stable_user_id}",
        "version": "${stable_version}"
    },
    "nightly": {
        "userId": "${nightly_user_id}",
        "version": "${nightly_version}"
    }
}
EOF
