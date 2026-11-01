import java.io.BufferedReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import org.apache.lucene.analysis.Analyzer;
import org.apache.lucene.analysis.TokenStream;
import org.apache.lucene.analysis.core.WhitespaceTokenizer;
import org.apache.lucene.document.Document;
import org.apache.lucene.document.Field;
import org.apache.lucene.document.FieldType;
import org.apache.lucene.document.NumericDocValuesField;
import org.apache.lucene.index.DirectoryReader;
import org.apache.lucene.index.IndexOptions;
import org.apache.lucene.index.IndexWriter;
import org.apache.lucene.index.IndexWriterConfig;
import org.apache.lucene.index.LeafReader;
import org.apache.lucene.index.LogDocMergePolicy;
import org.apache.lucene.index.NumericDocValues;
import org.apache.lucene.index.Terms;
import org.apache.lucene.index.TermsEnum;
import org.apache.lucene.search.similarities.BM25Similarity;
import org.apache.lucene.store.FSDirectory;

/**
 * Lucene over the round's one field: tokens.tsv's tokens split on spaces as
 * they were joined, frequencies and positions kept, a length a document in
 * Lucene's norm byte, pid as a doc value. One indexing thread and a merge
 * policy of adjacent segments, merged into one, so that a document's number
 * is its pid as in every other engine; the build checks it.
 *
 * <p>Usage: Index tokens.tsv index-dir
 */
public final class Index {
  static final BM25Similarity BM25 = new BM25Similarity(1.2f, 0.75f);

  static Analyzer whitespace() {
    return new Analyzer() {
      @Override
      protected TokenStreamComponents createComponents(String field) {
        return new TokenStreamComponents(
            new WhitespaceTokenizer(TokenStream.DEFAULT_TOKEN_ATTRIBUTE_FACTORY, 1024 * 1024));
      }
    };
  }

  public static void main(String[] args) throws Exception {
    long started = System.nanoTime();
    Path tokens = Path.of(args[0]);
    Path dir = Path.of(args[1]);
    FieldType text = new FieldType();
    text.setTokenized(true);
    text.setIndexOptions(IndexOptions.DOCS_AND_FREQS_AND_POSITIONS);
    text.setOmitNorms(false);
    text.setStored(false);
    text.freeze();
    LogDocMergePolicy merges = new LogDocMergePolicy();
    merges.setNoCFSRatio(0.0);
    IndexWriterConfig config = new IndexWriterConfig(whitespace())
        .setSimilarity(BM25)
        .setRAMBufferSizeMB(1024)
        .setUseCompoundFile(false)
        .setMergePolicy(merges)
        .setOpenMode(IndexWriterConfig.OpenMode.CREATE);
    long rows = 0;
    try (FSDirectory directory = FSDirectory.open(dir);
        IndexWriter writer = new IndexWriter(directory, config);
        BufferedReader input = Files.newBufferedReader(tokens, StandardCharsets.UTF_8)) {
      String line;
      while ((line = input.readLine()) != null) {
        int tab = line.indexOf('\t');
        long pid = Long.parseLong(line.substring(0, tab));
        Document doc = new Document();
        doc.add(new Field("b", line.substring(tab + 1), text));
        doc.add(new NumericDocValuesField("pid", pid));
        writer.addDocument(doc);
        rows++;
        if (rows % 1_000_000 == 0) {
          System.err.printf("lucene %d documents, %.0f s%n", rows, (System.nanoTime() - started) / 1e9);
        }
      }
      double indexed = (System.nanoTime() - started) / 1e9;
      writer.forceMerge(1);
      writer.commit();
      double seconds = (System.nanoTime() - started) / 1e9;
      try (DirectoryReader reader = DirectoryReader.open(directory)) {
        boolean inOrder = true;
        long postings = 0;
        for (var leaf : reader.leaves()) {
          LeafReader leafReader = leaf.reader();
          NumericDocValues pids = leafReader.getNumericDocValues("pid");
          for (int doc = 0; doc < leafReader.maxDoc(); doc++) {
            inOrder &= pids.advanceExact(doc) && pids.longValue() == leaf.docBase + doc;
          }
          Terms terms = leafReader.terms("b");
          TermsEnum each = terms.iterator();
          while (each.next() != null) {
            postings += each.docFreq();
          }
        }
        System.out.printf(
            "{\"step\":\"build\",\"engine\":\"lucene\",\"lucene\":\"%s\",\"documents\":%d,\"index_seconds\":%.3f,"
                + "\"seconds\":%.3f,\"segments\":%d,\"ids_follow_pids\":%b,\"postings\":%d,\"heap_max_mib\":%d}%n",
            org.apache.lucene.util.Version.LATEST, rows, indexed, seconds, reader.leaves().size(), inOrder,
            postings, Runtime.getRuntime().maxMemory() >> 20);
      }
    }
  }
}
