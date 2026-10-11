package records

import (
	"bufio"
	"encoding/binary"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"testing"
	"time"
)

// TestCompareDump writes what a writer of Lines makes of the corpus, a file a
// container, and prints what this engine's columns take of it: a scratch
// measurement for the Rust port, never committed.
func TestCompareDump(t *testing.T) {
	bodies, times := corpusTexts(t)
	out := os.Getenv("TINYSTORE_DUMP")
	e, _ := testCoders(t)
	var records, timeBytes, bodyBytes, blockBytes, rowBytes, plain, blockCount int
	bySlot := map[byte]int{}
	tally := map[string][2]int{}
	note := func(kind string, values []int64) {
		if len(values) == 0 {
			return
		}
		out := e.appendInts(nil, values)
		name := kind + " "
		switch out[0] {
		case layoutDictionary:
			name += "dictionary"
		case layoutDominant:
			name += "dominant"
		default:
			plan := e.planInts(values)
			name += fmt.Sprintf("direct transform=%d packer=%d", plan.transform, plan.packer)
		}
		seen := tally[name]
		seen[0]++
		seen[1] += len(out)
		tally[name] = seen
	}
	started := time.Now()
	for i := range bodies {
		if out != "" {
			dump(t, filepath.Join(out, fmt.Sprintf("%03d.jsonl", i)), bodies[i], times[i])
		}
		for start := 0; start < len(bodies[i]); start += maxSegmentRecords {
			end := min(start+maxSegmentRecords, len(bodies[i]))
			segment := make([]Record, end-start)
			for j := start; j < end; j++ {
				body := bodies[i][j]
				plain += len(body)
				segment[j-start] = Record{At: time.Unix(0, times[i][j]).UTC(), Stream: "corpus", Name: textLine, Body: &body}
			}
			encoded := e.encodeSegment("corpus", segment)
			rowBytes += len(encoded.row)
			s, _ := newSchema("corpus", segment)
			for _, block := range encoded.blocks {
				blockBytes += len(block.body)
				at := 1
				_, n := binary.Uvarint(block.body[at:])
				at += n
				for _, slot := range s.slots {
					length, n := binary.Uvarint(block.body[at:])
					at += n
					bySlot[slot.kind] += int(length)
				}
				blockCount++
			}
			for from := 0; from < len(segment); from += maxBlockRecords {
				to := min(from+maxBlockRecords, len(segment))
				at, text := make([]int64, to-from), make([]string, to-from)
				for k := from; k < to; k++ {
					at[k-from], text[k-from] = segment[k].At.UnixNano(), *segment[k].Body
				}
				timeBytes += len(e.appendInts(nil, at))
				note("times", at)
				if e.findColumnStamps(text, at) {
					column := e.cutStamps(text, at)
					note("stamp counts", column.counts)
					note("stamp layouts", column.layoutIDs)
					note("stamp gaps", column.gaps)
					note("stamp behind", column.behind)
				}
				bodyBytes += len(e.appendValues(nil, text, at))
			}
			records += len(segment)
		}
	}
	per := func(n int) float64 { return float64(n) / float64(records) }
	fmt.Printf("go   records=%d plain=%.2f times=%.3f bodies=%.3f blocks=%.3f segment_rows=%.3f whole=%.3f took=%v\n",
		records, per(plain), per(timeBytes), per(bodyBytes), per(blockBytes), per(rowBytes), per(blockBytes+rowBytes),
		time.Since(started).Round(time.Millisecond))
	names := make([]string, 0, len(tally))
	for name := range tally {
		names = append(names, name)
	}
	slices.Sort(names)
	for _, name := range names {
		fmt.Printf("go   %-44s columns=%-5d bytes_a_record=%.4f\n", name, tally[name][0], per(tally[name][1]))
	}
	fmt.Printf("go   blocks=%d in_block: time=%.3f body=%.3f others=%v\n", blockCount, per(bySlot[slotTime]), per(bySlot[slotBody]), bySlot)
}

func dump(t *testing.T, path string, bodies []string, times []int64) {
	file, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	w := bufio.NewWriterSize(file, 1<<20)
	enc := json.NewEncoder(w)
	enc.SetEscapeHTML(false)
	for j := range bodies {
		if err := enc.Encode(struct {
			At   int64  `json:"at"`
			Body string `json:"body"`
		}{times[j], bodies[j]}); err != nil {
			t.Fatal(err)
		}
	}
	if err := w.Flush(); err != nil {
		t.Fatal(err)
	}
	_ = file.Close()
}
