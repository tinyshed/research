package codecpages;

import gr.aueb.delorean.chimp.OutputBitStream;
import org.urbcomp.startdb.compress.elf.compressor.ElfCompressor;
import org.urbcomp.startdb.compress.elf.decompressor.ElfDecompressor;
import org.urbcomp.startdb.compress.elf.xorcompressor.ElfXORCompressor;

/**
 * Elf: org.urbcomp.startdb.compress.elf.compressor.ElfCompressor and
 * decompressor.ElfDecompressor of Spatio-Temporal-Lab/elf, branch
 * vldb2023-release, driven as its TestCompressor.testELFCompressor does.
 * It is compiled against that repository's own copy of the bit streams in
 * gr.aueb.delorean.chimp, never against panagiotisl/chimp's.
 *
 * The one intervention: ElfXORCompressor's constructor writes into a fixed
 * byte[12000] (sized for blocks of 1,000 values); the harness replaces that
 * private final field with an OutputBitStream over a buffer large enough
 * for the block before the first value. The bytes written are the same.
 *
 * Body: per value Elf's erasure flag ('0', or '1' + 4 bits of beta*), then
 * the ElfXORCompressor's encoding of the (erased) value, the first one as 7
 * bits of trailing zeros + the rest of its 64 bits; the end of the stream is
 * a '0' flag, a NaN (0x7ff8000000000000) through the XOR encoder, one zero
 * bit and the padding to a whole byte.
 */
final class ElfCodec implements Codec {
	@Override
	public byte[] encode(double[] values) throws ReflectiveOperationException {
		ElfCompressor compressor = new ElfCompressor();
		ElfXORCompressor xor = (ElfXORCompressor) Codec.getField(compressor, "xorCompressor");
		byte[] buffer = new byte[Codec.capacity(values.length)];
		OutputBitStream out = new OutputBitStream(buffer);
		Codec.setField(xor, "out", out);
		if (xor.getOutputStream() != out) throw new IllegalStateException("could not give Elf the harness's buffer");
		for (double v : values) compressor.addValue(v);
		compressor.close();
		if (compressor.getBytes() != buffer) throw new IllegalStateException("Elf did not write into the harness's buffer");
		return Codec.bodyOf(buffer, out.writtenBits());
	}

	@Override
	public double[] decode(byte[] body) {
		return Codec.unbox(new ElfDecompressor(body).decompress());
	}
}
