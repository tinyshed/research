package codecpages;

import java.nio.ByteBuffer;

import fi.iki.yak.ts.compression.gorilla.ByteBufferBitInput;
import fi.iki.yak.ts.compression.gorilla.ByteBufferBitOutput;
import fi.iki.yak.ts.compression.gorilla.Compressor;
import fi.iki.yak.ts.compression.gorilla.Decompressor;

/**
 * Gorilla's value encoding as the Chimp paper measured it: the value-only
 * fi.iki.yak.ts.compression.gorilla.Compressor of panagiotisl/chimp over a
 * ByteBufferBitOutput, decoded by its Decompressor over a ByteBufferBitInput,
 * exactly as that repository's TestDoublePrecision.testCorilla does.
 *
 * Body: the first value's 64 bits, then per value '0' (same as previous),
 * '10' + meaningful bits (inside the previous window) or '11' + 5 bits of
 * leading zeros + 6 bits of length + meaningful bits; the end-of-stream
 * marker is a NaN (0x7ff8000000000000) encoded the same way, then one zero
 * bit, then flush() writes the partial byte (a whole zero byte if the stream
 * was already aligned).
 */
final class GorillaCodec implements Codec {
	@Override
	public byte[] encode(double[] values) {
		ByteBufferBitOutput output = new ByteBufferBitOutput();
		Compressor compressor = new Compressor(output);
		for (double v : values) compressor.addValue(v);
		compressor.close();
		ByteBuffer bb = output.getByteBuffer();
		bb.flip();
		byte[] body = new byte[bb.remaining()];
		bb.get(body);
		return body;
	}

	@Override
	public double[] decode(byte[] body) {
		Decompressor d = new Decompressor(new ByteBufferBitInput(ByteBuffer.wrap(body)));
		return Codec.unbox(d.getValues());
	}
}
