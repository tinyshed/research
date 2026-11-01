package codecpages;

import gr.aueb.delorean.chimp.ChimpN;
import gr.aueb.delorean.chimp.ChimpNDecompressor;
import gr.aueb.delorean.chimp.OutputBitStream;

/**
 * Chimp128: gr.aueb.delorean.chimp.ChimpN(128) and ChimpNDecompressor(body,
 * 128) of panagiotisl/chimp, driven as its TestDoublePrecision.testChimp128
 * does (the class its test reports as "Chimp128"; ChimpNNoIndex is the
 * variant without the index of previous values).
 *
 * The one intervention, as for Chimp: ChimpN's constructor writes into a
 * fixed byte[8000]; the harness replaces the private field with an
 * OutputBitStream over a buffer large enough for the block before the first
 * value. The bytes written are the same.
 *
 * Body: the first value's 64 bits, then ChimpN's flags, 7-bit references to
 * one of the 128 previous values and fields per value; the end-of-stream
 * marker is a NaN (0x7ff8000000000000) encoded like a value, then one zero
 * bit, then the padding to a whole byte.
 */
final class Chimp128Codec implements Codec {
	static final int PREVIOUS_VALUES = 128;

	@Override
	public byte[] encode(double[] values) throws ReflectiveOperationException {
		ChimpN compressor = new ChimpN(PREVIOUS_VALUES);
		byte[] buffer = new byte[Codec.capacity(values.length)];
		OutputBitStream out = new OutputBitStream(buffer);
		Codec.setField(compressor, "out", out);
		for (double v : values) compressor.addValue(v);
		compressor.close();
		if (compressor.getOut() != buffer) throw new IllegalStateException("ChimpN did not write into the harness's buffer");
		return Codec.bodyOf(buffer, out.writtenBits());
	}

	@Override
	public double[] decode(byte[] body) {
		return Codec.unbox(new ChimpNDecompressor(body, PREVIOUS_VALUES).getValues());
	}
}
