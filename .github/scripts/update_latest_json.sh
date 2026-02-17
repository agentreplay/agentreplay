#!/usr/bin/env bash
set -euo pipefail

TAG="${1:-}"
REPO="${GITHUB_REPOSITORY:-agentreplay/agentreplay}"
TOKEN="${GITHUB_TOKEN:-}"

if [[ -z "$TOKEN" ]]; then
  echo "ERROR: GITHUB_TOKEN is required"
  exit 1
fi

if [[ -z "$TAG" ]]; then
  echo "ERROR: tag argument is required (e.g. v0.2.2 or main)"
  exit 1
fi

api_base="https://api.github.com/repos/${REPO}"
release_json=$(curl -fsSL \
  -H "Accept: application/vnd.github+json" \
  -H "Authorization: Bearer ${TOKEN}" \
  "${api_base}/releases/tags/${TAG}")

upload_url=$(echo "$release_json" | jq -r '.upload_url' | sed 's/{?name,label}//')
pub_date=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
base_url="https://github.com/${REPO}/releases/download/${TAG}"

asset_name() {
  local pattern="$1"
  echo "$release_json" | jq -r --arg p "$pattern" '[.assets[].name | select(test($p))][0] // ""'
}

asset_sig() {
  local asset="$1"
  if [[ -z "$asset" ]]; then
    echo ""
    return
  fi
  echo "$release_json" | jq -r --arg a "$asset" '[.assets[].name | select(. == ($a + ".sig"))][0] // ""'
}

# macOS updater artifacts (prefer arch-specific names)
mac_aarch64_asset=$(asset_name 'aarch64.*\.app\.tar\.gz$')
mac_x64_asset=$(asset_name '(x64|x86_64).*\.app\.tar\.gz$')

# fallback for legacy generic naming
if [[ -z "$mac_aarch64_asset" ]]; then
  mac_aarch64_asset=$(asset_name '\.app\.tar\.gz$')
fi
if [[ -z "$mac_x64_asset" ]]; then
  mac_x64_asset=$(asset_name '\.app\.tar\.gz$')
fi

# Linux updater artifact
linux_asset=$(asset_name '\.AppImage\.tar\.gz$')
if [[ -z "$linux_asset" ]]; then
  linux_asset=$(asset_name '\.AppImage$')
fi

# Windows updater artifact
windows_asset=$(asset_name '\.nsis\.zip$')

mac_aarch64_sig=$(asset_sig "$mac_aarch64_asset")
mac_x64_sig=$(asset_sig "$mac_x64_asset")
linux_sig=$(asset_sig "$linux_asset")
windows_sig=$(asset_sig "$windows_asset")

platforms_json=$(jq -n \
  --arg base "$base_url" \
  --arg maa "$mac_aarch64_asset" --arg mas "$mac_aarch64_sig" \
  --arg mxa "$mac_x64_asset" --arg mxs "$mac_x64_sig" \
  --arg lia "$linux_asset" --arg lis "$linux_sig" \
  --arg wia "$windows_asset" --arg wis "$windows_sig" '
  {}
  + (if ($maa != "" and $mas != "") then {"darwin-aarch64": {signature: $mas, url: ($base + "/" + $maa)}} else {} end)
  + (if ($mxa != "" and $mxs != "") then {"darwin-x86_64": {signature: $mxs, url: ($base + "/" + $mxa)}} else {} end)
  + (if ($lia != "" and $lis != "") then {"linux-x86_64": {signature: $lis, url: ($base + "/" + $lia)}} else {} end)
  + (if ($wia != "" and $wis != "") then {"windows-x86_64": {signature: $wis, url: ($base + "/" + $wia)}} else {} end)
')

platform_count=$(echo "$platforms_json" | jq 'keys | length')
if [[ "$platform_count" -eq 0 ]]; then
  echo "ERROR: No valid updater platform artifacts with signatures found in release ${TAG}"
  echo "Detected assets in release ${TAG}:"
  echo "$release_json" | jq -r '.assets[].name' | sed 's/^/  - /'
  echo "Expected updater artifacts:"
  echo "  - macOS: *.app.tar.gz + *.app.tar.gz.sig"
  echo "  - Linux: *.AppImage.tar.gz + *.AppImage.tar.gz.sig (or .AppImage + .AppImage.sig)"
  echo "  - Windows: *.nsis.zip + *.nsis.zip.sig"
  echo "Tip: run per-platform release workflows for the same version tag (e.g., v0.2.2), then run update-latest-json workflow." 
  exit 1
fi

jq -n \
  --arg version "$TAG" \
  --arg notes "See release notes at https://github.com/${REPO}/releases/tag/${TAG}" \
  --arg pub_date "$pub_date" \
  --argjson platforms "$platforms_json" \
  '{version: $version, notes: $notes, pub_date: $pub_date, platforms: $platforms}' > latest.json

# delete old latest.json asset if present
existing_latest_id=$(echo "$release_json" | jq -r '.assets[] | select(.name == "latest.json") | .id' || true)
if [[ -n "$existing_latest_id" ]]; then
  curl -fsSL -X DELETE \
    -H "Accept: application/vnd.github+json" \
    -H "Authorization: Bearer ${TOKEN}" \
    "${api_base}/releases/assets/${existing_latest_id}" >/dev/null
fi

curl -fsSL -X POST \
  -H "Accept: application/vnd.github+json" \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  --data-binary @latest.json \
  "${upload_url}?name=latest.json" >/dev/null

echo "Updated latest.json for release tag: ${TAG}"
cat latest.json
