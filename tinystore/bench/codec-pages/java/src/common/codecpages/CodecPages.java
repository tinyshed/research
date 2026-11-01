package codecpages;

import java.io.BufferedInputStream;
import java.io.BufferedOutputStream;
import java.io.DataInputStream;
import java.io.EOFException;
import java.io.IOException;
import java.io.OutputStream;
import java.lang.management.ManagementFactory;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;

/**
 * The encoder of ../README.md for the published codecs written in Java:
 *
 *   java -cp ... codecpages.CodecPages <codec> <series file> <out dir> <L1,L2,...>
 *
 * Writes <out dir>/<codec>.L<L>.bodies for each block length and prints one
 * JSON line for each. The blocks are encoded and decoded by a child JVM
 * (Worker), so that a block whose encoder never returns can be killed and
 * recorded as failed, with an empty body, while the rest go on.
 */
public final class CodecPages {
	/** How long one block may take, encode and decode together, before it is killed: generous, since a block takes milliseconds. */
	static long blockBudgetMillis(int L) {
		return 3000 + L;
	}

	/** How long the worker may take to start, or to read the next series, before the run is abandoned. */
	static final long IDLE_BUDGET_MILLIS = 600_000;

	private CodecPages() {}

	static final class Result {
		byte status = 1;
		byte[] body = new byte[0];
	}

	public static void main(String[] args) throws Exception {
		if (args.length != 4 || !Codec.known(args[0])) {
			System.err.println("usage: codecpages.CodecPages <gorilla|chimp|chimp128|elf> <series file> <out dir> <L1,L2,...>");
			System.exit(2);
		}
		String codec = args[0];
		Path seriesPath = Path.of(args[1]);
		Path outDir = Path.of(args[2]);
		List<Integer> lengths = new ArrayList<>();
		for (String s : args[3].split(",")) {
			int L = Integer.parseInt(s.trim());
			if (L <= 0) throw new IllegalArgumentException("block length " + L);
			lengths.add(L);
		}
		Codec.named(codec); // fail here, not in the worker, if the class path lacks the codec

		List<SeriesFile.Series> series = SeriesFile.read(seriesPath);
		Files.createDirectories(outDir);
		for (int L : lengths) {
			List<SeriesFile.Block> blocks = SeriesFile.blocks(series, L);
			Result[] results = run(codec, seriesPath, L, blocks);
			long values = 0, payload = 0, failed = 0;
			for (int b = 0; b < blocks.size(); b++) {
				values += blocks.get(b).count;
				payload += results[b].body.length;
				if (results[b].status != 0) failed++;
			}
			writeBodies(outDir.resolve(codec + ".L" + L + ".bodies"), blocks, results);
			System.out.printf("{\"codec\":\"%s\",\"block\":%d,\"blocks\":%d,\"values\":%d,\"payload_bytes\":%d,\"failed_blocks\":%d}%n",
					codec, L, blocks.size(), values, payload, failed);
			System.out.flush();
		}
	}

	private static final Object EOF = new Object();

	static Result[] run(String codec, Path seriesPath, int L, List<SeriesFile.Block> blocks) throws Exception {
		Result[] results = new Result[blocks.size()];
		for (int b = 0; b < results.length; b++) results[b] = new Result();
		int next = 0;
		while (next < blocks.size()) {
			Process p = startWorker(codec, seriesPath, L, next);
			BlockingQueue<Object> events = new LinkedBlockingQueue<>();
			Thread reader = new Thread(() -> readFrames(p, events), "frames");
			reader.setDaemon(true);
			reader.start();

			int current = -1;
			long budget = IDLE_BUDGET_MILLIS;
			boolean restart = false;
			while (!restart) {
				Object ev = events.poll(budget, TimeUnit.MILLISECONDS);
				if (ev == null) {
					p.destroyForcibly();
					p.waitFor();
					if (current < 0) throw new IOException("worker made no progress for " + budget + " ms outside a block");
					System.err.println(Worker.describe(codec, L, current, blocks.get(current))
							+ ": did not return within " + budget + " ms; worker killed");
					next = current + 1;
					restart = true;
				} else if (ev == EOF) {
					int code = p.waitFor();
					if (current < 0) throw new IOException("worker exited with " + code + " outside a block");
					System.err.println(Worker.describe(codec, L, current, blocks.get(current)) + ": worker exited with " + code);
					next = current + 1;
					restart = true;
				} else if (ev instanceof Integer) {
					current = (Integer) ev;
					budget = blockBudgetMillis(L);
				} else if (ev instanceof Object[]) {
					Object[] r = (Object[]) ev;
					int b = (Integer) r[0];
					results[b].status = (Byte) r[1];
					results[b].body = (byte[]) r[2];
					next = b + 1;
					current = -1;
					budget = IDLE_BUDGET_MILLIS;
				} else { // 'E'
					p.waitFor();
					if (next != blocks.size()) throw new IOException("worker ended at block " + next + " of " + blocks.size());
					restart = true;
				}
			}
		}
		return results;
	}

	static Process startWorker(String codec, Path seriesPath, int L, int first) throws IOException {
		List<String> cmd = new ArrayList<>();
		cmd.add(ProcessHandle.current().info().command().orElse(Path.of(System.getProperty("java.home"), "bin", "java").toString()));
		cmd.addAll(ManagementFactory.getRuntimeMXBean().getInputArguments());
		cmd.add("-cp");
		cmd.add(System.getProperty("java.class.path"));
		cmd.add(Worker.class.getName());
		cmd.add(codec);
		cmd.add(seriesPath.toString());
		cmd.add(Integer.toString(L));
		cmd.add(Integer.toString(first));
		ProcessBuilder pb = new ProcessBuilder(cmd);
		pb.redirectError(ProcessBuilder.Redirect.INHERIT);
		return pb.start();
	}

	static void readFrames(Process p, BlockingQueue<Object> events) {
		try (DataInputStream in = new DataInputStream(new BufferedInputStream(p.getInputStream(), 1 << 16))) {
			while (true) {
				int kind = in.readUnsignedByte();
				if (kind == 'B') {
					events.add(in.readInt());
				} else if (kind == 'R') {
					int b = in.readInt();
					byte status = in.readByte();
					byte[] body = new byte[in.readInt()];
					in.readFully(body);
					events.add(new Object[] {b, status, body});
				} else if (kind == 'E') {
					events.add("E");
					return;
				} else {
					throw new IOException("bad frame " + kind);
				}
			}
		} catch (EOFException e) {
			events.add(EOF);
		} catch (IOException e) {
			System.err.println("reading the worker: " + e);
			events.add(EOF);
		}
	}

	static void writeBodies(Path path, List<SeriesFile.Block> blocks, Result[] results) throws IOException {
		Path tmp = path.resolveSibling(path.getFileName() + ".tmp");
		try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(tmp), 1 << 20)) {
			ByteBuffer head = ByteBuffer.allocate(13).order(ByteOrder.LITTLE_ENDIAN);
			out.write(new byte[] {'C', 'P', 'B', '1'});
			head.clear();
			head.putInt(blocks.size());
			out.write(head.array(), 0, 4);
			for (int b = 0; b < blocks.size(); b++) {
				head.clear();
				head.putInt(blocks.get(b).series.index);
				head.putInt(blocks.get(b).count);
				head.putInt(results[b].body.length);
				head.put(results[b].status);
				out.write(head.array(), 0, 13);
				out.write(results[b].body);
			}
		}
		Files.move(tmp, path, java.nio.file.StandardCopyOption.REPLACE_EXISTING);
	}
}
