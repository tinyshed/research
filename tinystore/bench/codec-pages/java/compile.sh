#!/bin/sh
# Runs inside eclipse-temurin:21-jdk (see codec-pages.sh). Compiles, from the
# trees fetch-sources.sh put in /src, the authors' classes the four codecs
# need, unmodified, with the harness, into /build: two class trees, because
# panagiotisl/chimp and Elf each carry their own gr.aueb.delorean.chimp bit
# streams (Elf's adds getBuffer()). javac compiles only the named sources and
# what they reference on the source path, so of dsiutils and fastutil only
# the classes the bit streams import, transitively, are built.
set -eu

deps=/src/fastutil/src:/src/dsiutils/src
common=$(find /harness/src/common -name '*.java')

rm -rf /build/chimp /build/elf

C=/src/chimp/src/main/java
mkdir -p /build/chimp
javac -nowarn -Xlint:none -encoding UTF-8 -d /build/chimp -sourcepath "$deps:$C" \
	$common $(find /harness/src/chimp -name '*.java') \
	$C/fi/iki/yak/ts/compression/gorilla/Compressor.java \
	$C/fi/iki/yak/ts/compression/gorilla/Decompressor.java \
	$C/fi/iki/yak/ts/compression/gorilla/ByteBufferBitOutput.java \
	$C/fi/iki/yak/ts/compression/gorilla/ByteBufferBitInput.java \
	$C/gr/aueb/delorean/chimp/Chimp.java \
	$C/gr/aueb/delorean/chimp/ChimpDecompressor.java \
	$C/gr/aueb/delorean/chimp/ChimpN.java \
	$C/gr/aueb/delorean/chimp/ChimpNDecompressor.java

E=/src/elf/src/main/java
mkdir -p /build/elf
javac -nowarn -Xlint:none -encoding UTF-8 -d /build/elf -sourcepath "$deps:$E" \
	$common $(find /harness/src/elf -name '*.java') \
	$E/org/urbcomp/startdb/compress/elf/compressor/ElfCompressor.java \
	$E/org/urbcomp/startdb/compress/elf/decompressor/ElfDecompressor.java

# The bit streams' static initialisers load dsiutils' precomputed code
# tables (committed in its source tree) from the class path.
for tree in chimp elf; do
	mkdir -p /build/$tree/it/unimi/dsi/io
	cp /src/dsiutils/src/it/unimi/dsi/io/*.in.16 /src/dsiutils/src/it/unimi/dsi/io/*.out.12 /build/$tree/it/unimi/dsi/io/
	echo "$tree: $(find /build/$tree -name '*.class' | wc -l) classes"
done
cp /src/commits.txt /build/commits.txt
java -version 2>&1 | head -1 >/build/jdk.txt
cat /build/commits.txt /build/jdk.txt
