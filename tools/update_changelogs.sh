#!/bin/bash

set -euo pipefail

# Script to update CHANGELOG.md and debian/changelog before creating a git tag
# This replaces the GitHub Actions workflow that ran after tag creation

usage() {
    echo "Usage: $0 <version>"
    echo "Example: $0 v1.0.0"
    echo ""
    echo "This script will:"
    echo "  1. Update CHANGELOG.md with commits since the last tag"
    echo "  2. Update debian/changelog with the new version"
    echo "  3. Create a commit with the changelog updates"
    echo ""
    echo "Run this script before creating and pushing your git tag."
    exit 1
}

if [ $# -ne 1 ]; then
    usage
fi

LATEST_TAG="$1"
VERSION="${LATEST_TAG#v}"  # Remove 'v' prefix

# Validate tag format
if [[ ! "$LATEST_TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: Tag must be in format v1.2.3"
    exit 1
fi

# Check if we're in a git repository
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    echo "Error: Not in a git repository"
    exit 1
fi

# Check if tag already exists
if git rev-parse "$LATEST_TAG" >/dev/null 2>&1; then
    echo "Error: Tag $LATEST_TAG already exists"
    exit 1
fi

# Get package name from Cargo.toml
PKG_NAME=$(grep -A 10 '\[package\]' Cargo.toml | grep '^name *=' | sed 's/name *= *"\(.*\)"/\1/')
if [ -z "$PKG_NAME" ]; then
    echo "Error: Could not extract package name from Cargo.toml"
    exit 1
fi

echo "Updating changelogs for $PKG_NAME version $VERSION (tag: $LATEST_TAG)"

# Determine previous tag (exclude current if it exists)
PREV_TAG=$(git tag --sort=-creatordate | head -n1 || true)

if [ -z "$PREV_TAG" ]; then
    echo "No previous tags found, including all commits"
    COMMIT_LOG=$(git log --pretty=format:"- %s (%h)" HEAD || true)
else
    echo "Previous tag: $PREV_TAG"
    COMMIT_LOG=$(git log --pretty=format:"- %s (%h)" "${PREV_TAG}..HEAD" || true)
fi

if [ -z "$COMMIT_LOG" ]; then
    echo "Warning: No commits found for changelog"
    COMMIT_LOG="- Initial release"
fi

DATE=$(date -u +"%Y-%m-%d")

# Update CHANGELOG.md
echo "Updating CHANGELOG.md..."
CHANGELOG_ENTRY="## ${LATEST_TAG} - ${DATE}\n\n### Commits\n\n${COMMIT_LOG}\n"

TMPFILE=$(mktemp)
echo -e "# Changelog\n" > "$TMPFILE"
echo -e "$CHANGELOG_ENTRY" >> "$TMPFILE"
if [ -f CHANGELOG.md ]; then
    echo "" >> "$TMPFILE"
    # Skip the first line if it's already "# Changelog"
    if head -n1 CHANGELOG.md | grep -q "^# Changelog"; then
        tail -n +2 CHANGELOG.md >> "$TMPFILE"
    else
        cat CHANGELOG.md >> "$TMPFILE"
    fi
fi
mv "$TMPFILE" CHANGELOG.md

# Update debian/changelog
echo "Updating debian/changelog..."
if [ -z "$PREV_TAG" ]; then
    DEBIAN_COMMIT_LOG=$(git log --pretty=format:"  * %s" HEAD || true)
else
    DEBIAN_COMMIT_LOG=$(git log --pretty=format:"  * %s" "${PREV_TAG}..HEAD" || true)
fi

if [ -z "$DEBIAN_COMMIT_LOG" ]; then
    DEBIAN_COMMIT_LOG="  * Initial release"
fi

DEBIAN_CHANGELOG_ENTRY="${PKG_NAME} (${VERSION}-1) unstable; urgency=medium

$DEBIAN_COMMIT_LOG

 -- $(git config user.name) <$(git config user.email)>  $(date -R)
"

# Prepend the new entry to the changelog
if [ -f debian/changelog ]; then
    echo "$DEBIAN_CHANGELOG_ENTRY" | cat - debian/changelog > temp_changelog && mv temp_changelog debian/changelog
else
    echo "$DEBIAN_CHANGELOG_ENTRY" > debian/changelog
fi

# Show what changed
echo ""
echo "=== Changes made ==="
echo "CHANGELOG.md updated:"
head -n 10 CHANGELOG.md | sed 's/^/  /'
echo ""
echo "debian/changelog updated:"
head -n 8 debian/changelog | sed 's/^/  /'
echo ""

# Stage the files
git add CHANGELOG.md debian/changelog

# Show git status
echo "=== Git status ==="
git status --porcelain

echo ""
echo "=== Next steps ==="
echo "1. Review the changes above"
echo "2. Commit the changelog updates:"
echo "   git commit -m \"Update changelogs for release $LATEST_TAG\""
echo "3. Create and push the tag:"
echo "   git tag $LATEST_TAG"
echo "   git push origin main $LATEST_TAG"
echo ""
echo "Changelog updates are ready. The files have been staged for commit."