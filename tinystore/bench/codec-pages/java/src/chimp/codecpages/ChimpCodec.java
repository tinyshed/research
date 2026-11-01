package codecpages;

import gr.aueb.delorean.chimp.Chimp;
import gr.aueb.delorean.chimp.ChimpDecompressor;
import gr.aueb.delorean.chimp.OutputBitStream;

/**
 * Chimp: gr.aueb.delorean.chimp.Chimp and ChimpDecompressor of
 * panagiotisl/chimp, driven as its TestDoublePrecision.testChimp does.
 *
 * The one intervention: Chimp's constructor writes into a fixed byte[8000]
 * (sized for its test's blocks of 1,000 values), and the OutputBitStream
 * swallows the exception on overflow and silently drops the rest. Before the
 * first value the harness replaces the private field with an OutputBitStream
 * over a buffer large enough for the block. The bytes written are the same.
 *
 * Body: the first value's 64 bits, then Chimp's flags and fields per value;
 * the end-of-stream marker is a NaN (0x7ff8000000000000) encoded like a
 * value, then one zero bit, then the padding to a whole byte.
 */
final class ChimpCodec implements Codec {
	@Override
	public byte[] encode(double[] values) throws ReflectiveOperationException {
		Chimp compressor = new Chimp();
		byte[] buffer = new byte[Codec.capacity(values.length)];
		OutputBitStream out = new OutputBitStream(buffer);
		Codec.setField(compressor, "out", out);
		for (double v : values) compressor.addValue(v);
		compressor.close();
		if (compressor.getOut() != buffer) throw new IllegalStateException("Chimp did not write into the harness's buffer");
		return Codec.bodyOf(buffer, out.writtenBits());
	}

	@Override
	public double[] decode(byte[] body) {
		return Codec.unbox(new ChimpDecompressor(body).getValues());
	}
}
