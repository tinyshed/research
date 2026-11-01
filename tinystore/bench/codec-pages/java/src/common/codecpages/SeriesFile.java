package codecpages;

import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.channels.FileChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.List;

/** The series file of ../README.md and its block rule. */
final class SeriesFile {
	/** One series: its name, its value count and where its values start in the file. */
	static final class Series {
		final int index;
		final String name;
		final long count;
		final long offset;

		Series(int index, String name, long count, long offset) {
			this.index = index;
			this.name = name;
			this.count = count;
			this.offset = offset;
		}
	}

	/** One block: a series, the first value's position in it, and how many values. */
	static final class Block {
		final Series series;
		final long first;
		final int count;

		Block(Series series, long first, int count) {
			this.series = series;
			this.first = first;
			this.count = count;
		}
	}

	private SeriesFile() {}

	static List<Series> read(Path path) throws IOException {
		List<Series> out = new ArrayList<>();
		try (FileChannel ch = FileChannel.open(path, StandardOpenOption.READ)) {
			ByteBuffer head = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN);
			readFully(ch, head, 0);
			if (head.get(0) != 'C' || head.get(1) != 'P' || head.get(2) != 'S' || head.get(3) != '1') {
				throw new IOException(path + ": not a series file");
			}
			long count = Integer.toUnsignedLong(head.getInt(4));
			long pos = 8;
			for (int i = 0; i < count; i++) {
				ByteBuffer len = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN);
				readFully(ch, len, pos);
				int nl = len.getInt(0);
				pos += 4;
				ByteBuffer name = ByteBuffer.allocate(nl);
				readFully(ch, name, pos);
				pos += nl;
				ByteBuffer n = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN);
				readFully(ch, n, pos);
				pos += 8;
				long values = n.getLong(0);
				out.add(new Series(i, new String(name.array(), StandardCharsets.UTF_8), values, pos));
				pos += 8 * values;
			}
			if (pos != ch.size()) throw new IOException(path + ": " + (ch.size() - pos) + " bytes after the last series");
		}
		return out;
	}

	/** The block rule: L consecutive values of a series from its first, the remainder last, never across series. */
	static List<Block> blocks(List<Series> series, int L) {
		List<Block> out = new ArrayList<>();
		for (Series s : series) {
			for (long first = 0; first < s.count; first += L) {
				out.add(new Block(s, first, (int) Math.min(L, s.count - first)));
			}
		}
		return out;
	}

	/** A series' values as their raw 64 bits. */
	static long[] values(Path path, Series s) throws IOException {
		if (s.count > Integer.MAX_VALUE - 8) throw new IOException(s.name + ": too many values for one array");
		long[] out = new long[(int) s.count];
		try (FileChannel ch = FileChannel.open(path, StandardOpenOption.READ)) {
			ByteBuffer buf = ByteBuffer.allocate(1 << 20).order(ByteOrder.LITTLE_ENDIAN);
			long pos = s.offset;
			int i = 0;
			while (i < out.length) {
				buf.clear();
				buf.limit((int) Math.min(buf.capacity(), 8L * (out.length - i)));
				readFully(ch, buf, pos);
				pos += buf.limit();
				buf.flip();
				while (buf.hasRemaining()) out[i++] = buf.getLong();
			}
		}
		return out;
	}

	private static void readFully(FileChannel ch, ByteBuffer buf, long pos) throws IOException {
		while (buf.hasRemaining()) {
			int r = ch.read(buf, pos);
			if (r < 0) throw new IOException("unexpected end of series file");
			pos += r;
		}
	}
}
