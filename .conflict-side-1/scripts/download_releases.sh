#!/bin/bash

base_dir="downloads"

mkdir -p "$base_dir"

download_version() {
    local version=$1
    local dir="$base_dir/$version"
    
    release_data=$(cn release show fastrepl/hyprnote "$version" --channel=stable 2>/dev/null)
    
    if [ $? -ne 0 ]; then
        return 1
    fi
    
    dmg_x86_64=$(echo "$release_data" | jq -r '.assets[] | select(.publicPlatform == "dmg-x86_64") | .filename')
    dmg_aarch64=$(echo "$release_data" | jq -r '.assets[] | select(.publicPlatform == "dmg-aarch64") | .filename')
    
    mkdir -p "$dir"

    if [ ! -z "$dmg_x86_64" ] && [ "$dmg_x86_64" != "null" ]; then
        curl -sL "https://cdn.crabnebula.app/download/fastrepl/hyprnote/$version/$dmg_x86_64" -o "$dir/$dmg_x86_64"
    fi
    if [ ! -z "$dmg_aarch64" ] && [ "$dmg_aarch64" != "null" ]; then
        curl -sL "https://cdn.crabnebula.app/download/fastrepl/hyprnote/$version/$dmg_aarch64" -o "$dir/$dmg_aarch64"
    fi
}

for i in {55..56}; do
    version="0.0.$i"
    download_version "$version"
done
