package main

import (
	"bufio"
	"encoding/binary"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math"
	"os"
	"path/filepath"
)

type series struct {
	Metric     map[string]string `json:"metric"`
	Values     []float64         `json:"values"`
	Timestamps []int64           `json:"timestamps"`
}

type entry struct {
	Labels map[string]string
	Offset int64
	Count  int
}

type result struct {
	Samples    int64
	Series     int
	DataBytes  int64
	IndexBytes int64
	TotalBytes int64
}

func main() {
	if len(os.Args) != 3 {
		fmt.Fprintln(os.Stderr, "usage: raw <TSBS-JSONL> <new-directory>")
		os.Exit(1)
	}
	measured, err := writeRaw(os.Args[1], os.Args[2])
	if err == nil {
		err = json.NewEncoder(os.Stdout).Encode(measured)
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func writeRaw(corpus, dir string) (_ result, err error) {
	if err = os.Mkdir(dir, 0o700); err != nil {
		return result{}, err
	}
	file, err := os.Create(filepath.Join(dir, "samples.bin"))
	if err != nil {
		return result{}, err
	}
	defer func() { err = errors.Join(err, file.Close()) }()
	writer := bufio.NewWriter(file)
	var index []entry
	measured := result{}
	err = walk(corpus, func(s series) error {
		index = append(index, entry{Labels: s.Metric, Offset: measured.Samples * 16, Count: len(s.Values)})
		var encoded [16]byte
		for i, value := range s.Values {
			binary.LittleEndian.PutUint64(encoded[:8], uint64(s.Timestamps[i]))
			binary.LittleEndian.PutUint64(encoded[8:], math.Float64bits(value))
			if _, writeErr := writer.Write(encoded[:]); writeErr != nil {
				return writeErr
			}
		}
		measured.Samples += int64(len(s.Values))
		return nil
	})
	if err != nil {
		return result{}, err
	}
	if err = writer.Flush(); err != nil {
		return result{}, err
	}
	if err = file.Sync(); err != nil {
		return result{}, err
	}
	indexBytes, err := json.Marshal(index)
	if err != nil {
		return result{}, err
	}
	if err = os.WriteFile(filepath.Join(dir, "series.json"), indexBytes, 0o600); err != nil {
		return result{}, err
	}
	if err = verify(corpus, filepath.Join(dir, "samples.bin")); err != nil {
		return result{}, err
	}
	measured.Series, measured.IndexBytes = len(index), int64(len(indexBytes))
	measured.DataBytes = measured.Samples * 16
	measured.TotalBytes = measured.DataBytes + measured.IndexBytes
	return measured, nil
}

func walk(path string, visit func(series) error) (err error) {
	file, err := os.Open(path)
	if err != nil {
		return err
	}
	defer func() { err = errors.Join(err, file.Close()) }()
	lines := bufio.NewScanner(file)
	lines.Buffer(make([]byte, 1<<20), 64<<20)
	for lines.Scan() {
		var s series
		if err = json.Unmarshal(lines.Bytes(), &s); err != nil {
			return err
		}
		if len(s.Values) == 0 || len(s.Values) != len(s.Timestamps) {
			return errors.New("corpus sample lengths disagree")
		}
		if err = visit(s); err != nil {
			return err
		}
	}
	return lines.Err()
}

func verify(corpus, path string) (err error) {
	file, err := os.Open(path)
	if err != nil {
		return err
	}
	defer func() { err = errors.Join(err, file.Close()) }()
	reader := bufio.NewReader(file)
	err = walk(corpus, func(s series) error {
		var encoded [16]byte
		for i, value := range s.Values {
			if _, readErr := io.ReadFull(reader, encoded[:]); readErr != nil {
				return readErr
			}
			if int64(binary.LittleEndian.Uint64(encoded[:8])) != s.Timestamps[i] ||
				binary.LittleEndian.Uint64(encoded[8:]) != math.Float64bits(value) {
				return errors.New("raw file changed a sample")
			}
		}
		return nil
	})
	if err != nil {
		return err
	}
	if _, err = reader.ReadByte(); !errors.Is(err, io.EOF) {
		return errors.New("raw file has trailing bytes")
	}
	return nil
}
