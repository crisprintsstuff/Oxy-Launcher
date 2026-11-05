#!/usr/bin/env bash
# Build the NSIS installer for OxyStudio.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

if [[ ! -f "dist/oxystudio.exe" ]]; then
  echo "dist/oxystudio.exe is missing. Running scripts/build-windows.sh first..."
  ./scripts/build-windows.sh
fi

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
outfile="dist/OxyStudio-${version}-Setup.exe"

if command -v makensis >/dev/null 2>&1; then
  echo "Building installer using host makensis..."
  makensis -DSOURCE_EXE="$root/dist/oxystudio.exe" -DOUTFILE="$root/$outfile" installer/oxystudio.nsi
elif command -v docker >/dev/null 2>&1; then
  echo "Building installer using Docker..."
  docker run --rm \
    -v "$root":/work \
    -w /work \
    ubuntu:24.04 \
    bash -lc "apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y nsis && makensis -DSOURCE_EXE=/work/dist/oxystudio.exe -DOUTFILE=/work/$outfile installer/oxystudio.nsi"
else
  echo "Neither makensis nor docker is available to build the installer." >&2
  exit 1
fi

(cd dist && sha256sum "OxyStudio-${version}-Setup.exe" "oxystudio-${version}-windows-x86_64.exe" "oxystudio.exe" > SHA256SUMS.txt)

echo "Wrote $outfile"
echo "Updated dist/SHA256SUMS.txt"
