package codecpages;

import java.io.BufferedOutputStream;
import java.io.DataOutputStream;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.PrintStream;
import java.nio.file.Path;
import java.util.List;

/**
 * The child process CodecPages runs: encodes and decodes the blocks of one
 * block length from a given block on, and reports each on stdout as a frame:
 *
 *   'B' block                         about to start the block
 *   'R' block status length body      its result
 *   'E'                               every block done
 *
 * A block whose encoder or decoder never returns is the parent's to kill;
 * a JVM cannot stop a thread that spins without checking for interrupts.
 *
 *   java -cp ... codecpages.Worker <codec> <series file> <L> <first block>
 */
public final class Worker {
	private Worker() {}

	public static void main(String[] args) throws Exception {
		String codecName = args[0];
		Path seriesPath = Path.of(args[1]);
		int L = Integer.parseInt(args[2]);
		int firstBlock = Integer.parseInt(args[3]);

		// The frames own stdout; anything the authors' code prints goes to stderr.
		DataOutputStream frames = new DataOutputStream(new BufferedOutputStream(new FileOutputStream(FileDescriptor.out), 1 << 16));
		System.setOut(new PrintStream(new FileOutputStream(FileDescriptor.err), true));

		Codec codec = Codec.named(codecName);
		List<SeriesFile.Series> series = SeriesFile.read(seriesPath);
		List<SeriesFile.Block> blocks = SeriesFile.blocks(series, L);

		SeriesFile.Series loaded = null;
		long[] raw = null;
		for (int b = firstBlock; b < blocks.size(); b++) {
			SeriesFile.Block block = blocks.get(b);
			if (block.series != loaded) {
				raw = null; // let the previous series go before reading the next
				raw = SeriesFile.values(seriesPath, block.series);
				loaded = block.series;
			}
			frames.writeByte('B');
			frames.writeInt(b);
			frames.flush();

			long[] bits = new long[block.count];
			System.arraycopy(raw, (int) block.first, bits, 0, block.count);
			byte[] body = new byte[0];
			String failure = null;
			try {
				double[] values = new double[bits.length];
				for (int i = 0; i < bits.length; i++) values[i] = Double.longBitsToDouble(bits[i]);
				body = codec.encode(values);
			} catch (Throwable t) {
				failure = "encode threw " + t;
			}
			if (failure == null) {
				try {
					failure = compare(bits, codec.decode(body));
				} catch (Throwable t) {
					failure = "decode threw " + t;
				}
			}
			if (failure != null) {
				System.err.println(describe(codecName, L, b, block) + ": " + failure);
			}

			frames.writeByte('R');
			frames.writeInt(b);
			frames.writeByte(failure == null ? 0 : 1);
			frames.writeInt(body.length);
			frames.write(body);
			frames.flush();
		}
		frames.writeByte('E');
		frames.flush();
	}

	/** Null if the decoded values are exactly the input's, bit for bit; otherwise what differs. */
	static String compare(long[] expected, double[] decoded) {
		int n = Math.min(expected.length, decoded.length);
		for (int i = 0; i < n; i++) {
			long got = Double.doubleToRawLongBits(decoded[i]);
			if (got != expected[i]) {
				return String.format("value %d is %016x (%s), expected %016x (%s)%s", i, got, decoded[i], expected[i],
						Double.longBitsToDouble(expected[i]),
						decoded.length == expected.length ? "" : "; " + decoded.length + " values decoded, " + expected.length + " expected");
			}
		}
		if (decoded.length != expected.length) {
			return decoded.length + " values decoded, " + expected.length + " expected";
		}
		return null;
	}

	static String describe(String codec, int L, int b, SeriesFile.Block block) {
		return String.format("%s L=%d block %d: series %d '%s' values %d..%d", codec, L, b, block.series.index,
				block.series.name, block.first, block.first + block.count - 1);
	}
}
