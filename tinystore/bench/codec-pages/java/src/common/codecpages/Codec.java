package codecpages;

/**
 * One codec as the harness drives it: a block of values in, the codec's own
 * output bytes for that block out, and back. Each call to {@link #encode}
 * uses a fresh compressor, each call to {@link #decode} a fresh decompressor.
 */
interface Codec {
	/** The block's body: exactly the bytes the codec wrote for it, end-of-stream marker and final padding included. */
	byte[] encode(double[] values) throws Exception;

	/** Every value the codec's own decompressor reads from the body, until its end-of-stream marker. */
	double[] decode(byte[] body) throws Exception;

	static Codec named(String name) throws ReflectiveOperationException {
		String cls;
		switch (name) {
		case "gorilla": cls = "codecpages.GorillaCodec"; break;
		case "chimp": cls = "codecpages.ChimpCodec"; break;
		case "chimp128": cls = "codecpages.Chimp128Codec"; break;
		case "elf": cls = "codecpages.ElfCodec"; break;
		default: throw new IllegalArgumentException("unknown codec " + name + " (gorilla, chimp, chimp128, elf)");
		}
		try {
			return (Codec) Class.forName(cls).getDeclaredConstructor().newInstance();
		} catch (ClassNotFoundException e) {
			throw new IllegalStateException(name + " is not in this class path: run.sh picks the build tree by codec", e);
		}
	}

	static boolean known(String name) {
		return name.equals("gorilla") || name.equals("chimp") || name.equals("chimp128") || name.equals("elf");
	}

	/** Replaces a private field of an authors' object; used only to give a compressor an output buffer large enough for the block. */
	static void setField(Object target, String name, Object value) throws ReflectiveOperationException {
		java.lang.reflect.Field f = target.getClass().getDeclaredField(name);
		f.setAccessible(true);
		f.set(target, value);
	}

	static Object getField(Object target, String name) throws ReflectiveOperationException {
		java.lang.reflect.Field f = target.getClass().getDeclaredField(name);
		f.setAccessible(true);
		return f.get(target);
	}

	/** Output capacity given to the fixed-buffer compressors: 128 bits a value, well above any of the four codecs' worst case (under 80). */
	static int capacity(int values) {
		return 16 * values + 64;
	}

	/**
	 * The body of a compressor writing into a byte array through the authors'
	 * OutputBitStream: after close() (which flushes and so byte-aligns) the
	 * stream has written exactly writtenBits / 8 bytes. Everything past them
	 * must still be zero, or the count is not what was written.
	 */
	static byte[] bodyOf(byte[] buffer, long writtenBits) {
		if (writtenBits % 8 != 0) throw new IllegalStateException("stream not byte-aligned after close: " + writtenBits + " bits");
		if (writtenBits / 8 >= buffer.length) throw new IllegalStateException("output filled its buffer of " + buffer.length + " bytes");
		int len = (int) (writtenBits / 8);
		for (int i = len; i < buffer.length; i++) {
			if (buffer[i] != 0) throw new IllegalStateException("byte " + i + " written past the stream's " + len + " bytes");
		}
		return java.util.Arrays.copyOf(buffer, len);
	}

	static double[] unbox(java.util.List<Double> list) {
		double[] out = new double[list.size()];
		int i = 0;
		for (Double d : list) out[i++] = d;
		return out;
	}
}
