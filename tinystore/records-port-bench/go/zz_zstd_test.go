package records

import (
	"fmt"
	"strings"
	"testing"

	"github.com/klauspost/compress/zstd"
)

// TestCompareZstd compresses the corpus's text lines, a thousand and
// twenty-four a blob, each line ended by a break, at three of this engine's
// zstd levels: the Rust port compresses the same blobs with the C library. A
// scratch measurement, never committed.
func TestCompareZstd(t *testing.T) {
	bodies, _ := corpusTexts(t)
	levels := []zstd.EncoderLevel{zstd.SpeedDefault, zstd.SpeedBetterCompression, zstd.SpeedBestCompression}
	encoders := make([]*zstd.Encoder, len(levels))
	for i, level := range levels {
		encoder, err := newBlobEncoder(level)
		if err != nil {
			t.Fatal(err)
		}
		t.Cleanup(func() { _ = encoder.Close() })
		encoders[i] = encoder
	}
	sizes := make([]int, len(levels))
	var records, raw, blobs int
	for _, texts := range bodies {
		for start := 0; start < len(texts); start += maxBlockRecords {
			blob := []byte(strings.Join(texts[start:min(start+maxBlockRecords, len(texts))], "\n") + "\n")
			raw += len(blob)
			for i, encoder := range encoders {
				sizes[i] += len(encoder.EncodeAll(blob, nil))
			}
			blobs++
		}
		records += len(texts)
	}
	per := func(n int) float64 { return float64(n) / float64(records) }
	fmt.Printf("go   records=%d blobs=%d raw=%.3f default=%.3f better=%.3f best=%.3f\n",
		records, blobs, per(raw), per(sizes[0]), per(sizes[1]), per(sizes[2]))
}
