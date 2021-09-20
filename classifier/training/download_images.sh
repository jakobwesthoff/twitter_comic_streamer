#!/bin/bash
set -eu
set -o pipefail


mkdir -p images/{comic,no_comic} &>/dev/null || true

pushd "images/comic"
  while read -r url; do
    wget -nv -c "$url"
  done < "../../urls_comic.log"
popd

pushd "images/no_comic"
  while read -r url; do
    wget -nv -c "$url"
  done < "../../urls_no_comic.log"
popd
