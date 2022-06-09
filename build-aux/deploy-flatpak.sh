#!/usr/bin/env bash

# This scripts edits the flatpak manifest to fetch the source archive from a url
# and sets the correct hash for that archive.
set -e

manifest=$1;
archive=$2;
url=$3;
git_out=$4;

sha256=$(sha256sum $archive | awk '{print $1}');
echo $sha256;
mv "$manifest" "$manifest.old"
< "$manifest.old" jq --arg url $url --arg sha "$sha256" '(.["modules"] | last | .["sources"] | last) |= {type: "archive", url: $url, sha256: $sha}' > "$manifest";

# Clone the flathub repo, commit update, push
git clone "$git_out" git_repo;
cd $_;
cp ../"$manifest" ./;
git commit -a -m "Update";
