#!/bin/sh
# Fetches Lucene's two jars this round compiles against from Maven Central
# into the directory given, checks them against jars.sha256, and compiles
# Index.java and Search.java beside them. Run in eclipse-temurin:21-jdk; a
# proxy goes in https_proxy, as curl reads it.
set -eu
out=$1
here=$(cd "$(dirname "$0")" && pwd)
version=10.5.2
mkdir -p "$out/classes"
cd "$out"
for artifact in lucene-core lucene-analysis-common; do
  [ -s "$artifact-$version.jar" ] || curl -fsSL -o "$artifact-$version.jar" \
    "https://repo1.maven.org/maven2/org/apache/lucene/$artifact/$version/$artifact-$version.jar"
done
sha256sum -c "$here/jars.sha256"
javac -cp "lucene-core-$version.jar:lucene-analysis-common-$version.jar" -d classes \
  "$here/Index.java" "$here/Search.java"
