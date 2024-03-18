#!/bin/bash

if [ -z ${1} ]; then 
    current_version=$(cat Cargo.toml | grep '^version =' | awk '{print $3}' | tr -d '"')
    IFS='.' read -r major minor patch <<< "$current_version"
     ((patch++))
    version="$major.$minor.$patch"
else
version=$1
fi

cargo changelog --write flute || exit -1
cargo set-version $version  || exit -1
git commit Cargo.toml CHANGELOG.md -m "v$version"
git tag v$version
git push
git push --tags
