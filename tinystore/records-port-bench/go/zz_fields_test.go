package records

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"testing"
	"time"
)

// TestCompareFields writes every record a writer of Lines makes of the corpus,
// text, JSON and logfmt alike, a file a container, and prints what this
// engine's slots take of them and what each kind of value column takes against
// the same values as plain text: a scratch measurement for the Rust port,
// never committed.
func TestCompareFields(t *testing.T) {
	root := os.Getenv("TINYSTORE_CORPUS")
	if os.Getenv("TINYSTORE_SPIKE") != "1" || root == "" {
		t.Skip("a corpus measurement: set TINYSTORE_SPIKE=1 and TINYSTORE_CORPUS")
	}
	out := os.Getenv("TINYSTORE_DUMP")
	e, _ := testCoders(t)
	hosts, _ := os.ReadDir(root)
	var containers []string
	for _, host := range hosts {
		dirs, _ := filepath.Glob(filepath.Join(root, host.Name(), "raw", "*"))
		slices.Sort(dirs)
		containers = append(containers, dirs...)
	}
	var records, blockBytes, rowBytes, blockCount int
	byName := map[string]int{}
	bySlot := map[byte]int{}
	kinds := map[string][3]int{} // columns, bytes, bytes as plain text
	started := time.Now()
	for i, dir := range containers {
		all := containerRecords(t, dir)
		for _, r := range all {
			byName[r.Name]++
		}
		if out != "" {
			dumpRecords(t, filepath.Join(out, fmt.Sprintf("%03d.jsonl", i)), all)
		}
		for start := 0; start < len(all); start += maxSegmentRecords {
			segment := all[start:min(start+maxSegmentRecords, len(all))]
			sortByTime(segment)
			s, ids := newSchema("corpus", segment)
			rowBytes += len(e.appendSchema(nil, s))
			for from := 0; from < len(segment); {
				to := blockEnd(segment, from)
				gathered := gatherColumns(s, segment[from:to], ids.slice(from, to))
				blockBytes += len(e.appendBlock(s, &gathered))
				for _, slot := range s.slots {
					column := e.appendSlot(nil, slot, &gathered)
					bySlot[slot.kind] += len(column)
					if slot.kind == slotAttr && len(column) > 0 {
						name := valueKindName(column[0])
						seen := kinds[name]
						seen[0]++
						seen[1] += len(column)
						seen[2] += 1 + len(e.appendTexts(nil, gathered.attrs[slot.column]))
						kinds[name] = seen
					}
				}
				blockCount++
				from = to
			}
			records += len(segment)
		}
	}
	per := func(n int) float64 { return float64(n) / float64(records) }
	fmt.Printf("go   records=%d by_name=%v blocks=%d took=%v\n", records, byName, blockCount,
		time.Since(started).Round(time.Millisecond))
	fmt.Printf("go   blocks=%.3f segment_rows=%.3f whole=%.3f\n", per(blockBytes), per(rowBytes), per(blockBytes+rowBytes))
	slotNames := []string{"time", "name", "shape", "context", "level", "body", "trace", "span", "raw", "attr"}
	for kind, name := range slotNames {
		fmt.Printf("go   slot %-8s %.3f\n", name, per(bySlot[byte(kind)]))
	}
	names := make([]string, 0, len(kinds))
	for name := range kinds {
		names = append(names, name)
	}
	slices.Sort(names)
	for _, name := range names {
		seen := kinds[name]
		fmt.Printf("go   attr %-32s columns=%-6d bytes=%.4f as_text=%.4f\n", name, seen[0], per(seen[1]), per(seen[2]))
	}
}

func valueKindName(flags byte) string {
	name := []string{"text", "integer", "hex", "uuid", "stamped"}[min(int(flags&0x0f), 4)]
	if flags&valueQuoted != 0 {
		name = "quoted " + name
	}
	if flags&valueExceptions != 0 {
		name += " exceptions"
	}
	if flags&valueTimed != 0 {
		name += " timed"
	}
	return name
}

// containerRecords are the records a writer of Lines makes of a container's
// entries, at the times docker received them
func containerRecords(t *testing.T, dir string) []Record {
	var all []Record
	var received time.Time
	w := newLineWriter("corpus", func() time.Time { return received }, func(r Record) { all = append(all, r) })
	for _, entry := range dockerEntries(t, dir) {
		received = entry.at
		if _, err := w.Write([]byte(entry.text)); err != nil {
			t.Fatal(err)
		}
	}
	if err := w.Close(); err != nil {
		t.Fatal(err)
	}
	return all
}

func dumpRecords(t *testing.T, path string, all []Record) {
	file, err := os.Create(path)
	if err != nil {
		t.Fatal(err)
	}
	w := bufio.NewWriterSize(file, 1<<20)
	enc := json.NewEncoder(w)
	enc.SetEscapeHTML(false)
	type dumped struct {
		At     int64       `json:"at"`
		Name   string      `json:"name"`
		Level  *int        `json:"level,omitempty"`
		Body   *string     `json:"body,omitempty"`
		Fields [][2]string `json:"fields,omitempty"`
	}
	for _, r := range all {
		d := dumped{At: r.At.UnixNano(), Name: r.Name, Body: r.Body}
		if r.Level != nil {
			level := int(*r.Level)
			d.Level = &level
		}
		for _, field := range r.Attrs {
			d.Fields = append(d.Fields, [2]string{field.Key, field.Value})
		}
		if err := enc.Encode(d); err != nil {
			t.Fatal(err)
		}
	}
	if err := w.Flush(); err != nil {
		t.Fatal(err)
	}
	_ = file.Close()
}
