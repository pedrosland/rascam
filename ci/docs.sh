#!/bin/bash
set -e
set -x

LIB_NAME="mmal_sys"
SOURCE_BRANCH="master"
TARGET_BRANCH="gh-pages"

# Pull requests and commits to other branches shouldn't deploy
#if [ "$TRAVIS_PULL_REQUEST" != "false" -o "$TRAVIS_BRANCH" != "$SOURCE_BRANCH" ]; then
#    echo "Pull request or not master branch. Skipping docs deployment."
#    exit 0
#fi

# Save some useful information
REPO=`git config remote.origin.url`
SSH_REPO=${REPO/https:\/\/github.com\//git@github.com:}
SHA=`git rev-parse --verify HEAD`

git remote add deploy "$SSH_REPO"

# Clone the existing gh-pages for this repo into out/
# Create a new empty branch if gh-pages doesn't exist yet (should only happen on first deploy)
if git fetch origin $TARGET_BRANCH:$TARGET_BRANCH; then
  git worktree add gh-pages $TARGET_BRANCH
else
  git worktree add gh-pages HEAD
  cd gh-pages
  git checkout --orphan $TARGET_BRANCH
  cd ..
fi

cd gh-pages

# Clean out existing contents
git rm -rf --quiet .

# Adding built docs dir.
cp -r ../target/armv7-unknown-linux-gnueabihf/doc/* .

touch .nojekyll
echo "<html><head><meta http-equiv='refresh' content='0;url=$LIB_NAME/index.html'></head></html>" > index.html

git add .

# If there are no changes to the docs then just bail.
if [[ -z $(git status -s) ]]; then
    echo "No changes to the docs. Exiting."
    exit 0
fi

git config user.name "Travis CI"
git config user.email "$COMMIT_AUTHOR_EMAIL"

# Commit the "changes", i.e. the new version.
git commit --quiet -m "Deploy to GitHub Pages: ${SHA}"

cd ..

# Get the deploy key by using Travis's stored variables to decrypt travis_deploy_key.enc
openssl aes-256-cbc -K $encrypted_13d65ea92dcd_key -iv $encrypted_13d65ea92dcd_iv -in ci/travis_deploy_key.enc -out ci/travis_deploy_key -d
chmod 600 ci/travis_deploy_key

# Push (deploy)
GIT_SSH_COMMAND='ssh -i ci/travis_deploy_key' git push $SSH_REPO $TARGET_BRANCH
