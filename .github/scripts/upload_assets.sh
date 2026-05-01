#! /bin/sh
TAG=$1
for i in dist/*.tar.gz
do
echo "Uploading $i to $UPLOAD_URL"
gh release upload v$TAG $i
done