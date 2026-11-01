import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

import org.apache.lucene.index.DirectoryReader;
import org.apache.lucene.index.LeafReader;
import org.apache.lucene.index.NumericDocValues;
import org.apache.lucene.index.Term;
import org.apache.lucene.search.BooleanClause;
import org.apache.lucene.search.BooleanQuery;
import org.apache.lucene.search.IndexSearcher;
import org.apache.lucene.search.Query;
import org.apache.lucene.search.ScoreDoc;
import org.apache.lucene.search.TermQuery;
import org.apache.lucene.search.TopDocs;
import org.apache.lucene.search.TopScoreDocCollectorManager;
import org.apache.lucene.store.FSDirectory;

/**
 * Each query's ten from the Lucene index: a disjunction of one TermQuery a
 * term, BM25 with k1 1.2 and b 0.75, no query cache, one thread.
 *
 * <p>{@code default} is IndexSearcher.search(query, 10), which counts hits
 * exactly up to 1,000 and then skips by Lucene's windowed MaxScore; {@code
 * exhaustive} counts every hit, which makes Lucene score every match.
 *
 * <p>Usage: Search index-dir queries.tsv results.jsonl default|exhaustive [answer|warm|cold]
 *
 * <p>{@code answer} writes each query's ten with scores; {@code warm} runs every
 * query once untimed first and {@code cold} does not, and both then write each
 * query's nanoseconds and ids. The caller evicts the index's pages before a
 * cold pass.
 */
public final class Search {
  record Asked(String qid, List<String> terms) {}

  static List<Asked> read(Path path) throws Exception {
    List<Asked> asked = new ArrayList<>();
    try (BufferedReader input = Files.newBufferedReader(path, StandardCharsets.UTF_8)) {
      String line;
      while ((line = input.readLine()) != null) {
        int tab = line.indexOf('\t');
        List<String> terms = new ArrayList<>();
        for (String term : line.substring(tab + 1).split(" ")) {
          if (!term.isEmpty()) {
            terms.add(term);
          }
        }
        asked.add(new Asked(line.substring(0, tab), terms));
      }
    }
    return asked;
  }

  static Query query(List<String> terms) {
    BooleanQuery.Builder builder = new BooleanQuery.Builder();
    for (String term : terms) {
      builder.add(new TermQuery(new Term("b", term)), BooleanClause.Occur.SHOULD);
    }
    return builder.build();
  }

  public static void main(String[] args) throws Exception {
    Path dir = Path.of(args[0]);
    List<Asked> asked = read(Path.of(args[1]));
    Path results = Path.of(args[2]);
    boolean exhaustive = args[3].equals("exhaustive");
    String mode = args.length > 4 ? args[4] : "answer";
    long opening = System.nanoTime();
    try (FSDirectory directory = FSDirectory.open(dir);
        DirectoryReader reader = DirectoryReader.open(directory)) {
      IndexSearcher searcher = new IndexSearcher(reader);
      searcher.setSimilarity(Index.BM25);
      searcher.setQueryCache(null);
      int[] pids = new int[reader.maxDoc()];
      for (var leaf : reader.leaves()) {
        LeafReader leafReader = leaf.reader();
        NumericDocValues values = leafReader.getNumericDocValues("pid");
        for (int doc = 0; doc < leafReader.maxDoc(); doc++) {
          values.advanceExact(doc);
          pids[leaf.docBase + doc] = (int) values.longValue();
        }
      }
      double openMs = (System.nanoTime() - opening) / 1e6;
      int threshold = exhaustive ? Integer.MAX_VALUE : 1000;
      if (mode.equals("warm")) {
        for (Asked one : asked) {
          searcher.search(query(one.terms()), new TopScoreDocCollectorManager(10, null, threshold));
        }
      }
      try (BufferedWriter out = Files.newBufferedWriter(results, StandardCharsets.UTF_8)) {
        for (Asked one : asked) {
          TopDocs top;
          long took = 0;
          if (one.terms().isEmpty()) {
            top = new TopDocs(null, new ScoreDoc[0]);
          } else {
            Query query = query(one.terms());
            long started = System.nanoTime();
            top = searcher.search(query, new TopScoreDocCollectorManager(10, null, threshold));
            took = System.nanoTime() - started;
          }
          StringBuilder line = new StringBuilder("{\"qid\":\"").append(one.qid()).append('"');
          if (mode.equals("answer")) {
            line.append(",\"ten\":[");
            for (int at = 0; at < top.scoreDocs.length; at++) {
              ScoreDoc hit = top.scoreDocs[at];
              line.append(at == 0 ? "" : ",").append('[').append(pids[hit.doc]).append(',')
                  .append(hit.score).append(']');
            }
            line.append("],\"us\":").append(took / 1000.0);
          } else {
            line.append(",\"ns\":").append(took).append(",\"ids\":[");
            for (int at = 0; at < top.scoreDocs.length; at++) {
              line.append(at == 0 ? "" : ",").append(pids[top.scoreDocs[at].doc]);
            }
            line.append(']');
          }
          out.write(line.append("}\n").toString());
        }
      }
      System.out.printf(
          "{\"step\":\"%s\",\"engine\":\"lucene-%s\",\"mode\":\"%s\",\"queries\":%d,\"open_ms\":%.3f,"
              + "\"heap_used_mib\":%d}%n",
          mode.equals("answer") ? "answer" : "time", exhaustive ? "exhaustive" : "maxscore", mode,
          asked.size(), openMs,
          (Runtime.getRuntime().totalMemory() - Runtime.getRuntime().freeMemory()) >> 20);
    }
  }
}
