#!/usr/bin/env bash
# Signs release artefacts with the CodeAlchemy-Labs GPG key.
#
# Usage: sign-release-artifacts.sh <artifacts-dir>
#
# The directory is expected to contain the artefact tree produced by
# actions/download-artifact. The script signs .deb, .rpm, .tar.gz, and .zip
# files. It creates a SHA256SUMS file covering every unsigned artefact, then
# signs that file as well.
#
# Required environment variables:
#   GPG_PRIVATE_KEY  ASCII-armored GPG private key
#   GPG_PASSPHRASE   Passphrase for the key
#   GPG_KEY_ID       Long key ID, e.g. C63EA044D207C82C

set -euo pipefail

ARTIFACTS_DIR="${1:-dist}"

if [[ -z "${GPG_PRIVATE_KEY:-}" ]]; then
  echo "GPG_PRIVATE_KEY is not set" >&2
  exit 1
fi
if [[ -z "${GPG_PASSPHRASE:-}" ]]; then
  echo "GPG_PASSPHRASE is not set" >&2
  exit 1
fi
if [[ -z "${GPG_KEY_ID:-}" ]]; then
  echo "GPG_KEY_ID is not set" >&2
  exit 1
fi

if [[ ! -d "$ARTIFACTS_DIR" ]]; then
  echo "Artifacts directory not found: $ARTIFACTS_DIR" >&2
  exit 1
fi

# --- Isolated GNUPGHOME ---
export GNUPGHOME="$(mktemp -d)"
chmod 700 "$GNUPGHOME"

cleanup() {
  rm -rf "$GNUPGHOME"
}
trap cleanup EXIT

# --- Import the key ---

echo "Importing GPG key"

printf '%s\n' "$GPG_PRIVATE_KEY" | gpg --batch --import

echo "allow-loopback-pinentry" >> "$GNUPGHOME/gpg-agent.conf"
echo "pinentry-mode loopback" >> "$GNUPGHOME/gpg.conf"
gpg-connect-agent reloadagent /bye >/dev/null 2>&1 || true

# Verify the key is present
if ! gpg --list-secret-keys "$GPG_KEY_ID" >/dev/null 2>&1; then
  echo "GPG key $GPG_KEY_ID not found after import" >&2
  exit 1
fi

# Helper: sign a single file with a detached armored signature
sign_detached() {
  local file="$1"
  if [[ ! -f "$file" ]]; then
    echo "Skipping missing file: $file" >&2
    return 0
  fi
  echo "Detached signing: $file"
  printf '%s\n' "$GPG_PASSPHRASE" | gpg --batch --yes \
      --pinentry-mode loopback \
      --passphrase-fd 0 \
      --local-user "$GPG_KEY_ID" \
      --armor --detach-sign \
      --output "${file}.asc" \
      "$file"
}

# --- Sign .deb in place with debsigs ---
if command -v debsigs >/dev/null 2>&1; then
  # debsigs reads the key from the standard GPG keyring.
  # Copy the key to the default location because debsigs does not honour
  # GNUPGHOME in all versions.
  mkdir -p "$HOME/.gnupg"
  chmod 700 "$HOME/.gnupg"
  printf '%s\n' "$GPG_PRIVATE_KEY" | gpg --batch --homedir "$HOME/.gnupg" --import

  while IFS= read -r -d '' deb; do
    echo "Signing .deb: $deb"
    debsigs --sign=origin --default-key="$GPG_KEY_ID" "$deb"
  done < <(find "$ARTIFACTS_DIR" -type f -name '*.deb' -print0)
else
  echo "debsigs not installed; skipping .deb in-place signing" >&2
  exit 1
fi

# --- Sign .rpm in place with rpm --addsign ---
if command -v rpm >/dev/null 2>&1; then
  cat > "$HOME/.rpmmacros" <<EOF
%_gpg_name $GPG_KEY_ID
%_gpg_path $GNUPGHOME
EOF

  while IFS= read -r -d '' rpmfile; do
    echo "Signing .rpm: $rpmfile"
    exec 3<<<"$GPG_PASSPHRASE"
    rpm --addsign \
        --define "_gpg_name $GPG_KEY_ID" \
        --define "_gpg_path $GNUPGHOME" \
        --define "_gpg_sign_cmd /usr/bin/gpg --batch --pinentry-mode loopback --passphrase-fd 3 --no-tty" \
        "$rpmfile"
    exec 3<&-
  done < <(find "$ARTIFACTS_DIR" -type f -name '*.rpm' -print0)
else
  echo "rpm not installed; skipping .rpm in-place signing" >&2
  exit 1
fi

# --- Detached signatures for .tar.gz and .zip ---
while IFS= read -r -d '' tarball; do
  sign_detached "$tarball"
done < <(find "$ARTIFACTS_DIR" -type f -name '*.tar.gz' -print0)

while IFS= read -r -d '' zipfile; do
  sign_detached "$zipfile"
done < <(find "$ARTIFACTS_DIR" -type f -name '*.zip' -print0)

# --- SHA256SUMS over everything that is not a .asc ---
echo "Generating SHA256SUMS"
(
  cd "$ARTIFACTS_DIR"
  find . -type f \
    ! -name '*.asc' \
    ! -name 'SHA256SUMS' \
    -print0 \
    | sort -z \
    | xargs -0 sha256sum > SHA256SUMS
)

sign_detached "$ARTIFACTS_DIR/SHA256SUMS"

echo "Signing complete."
