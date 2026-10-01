package main

import (
	"github.com/tinyshed/tinystore/bench/idle/internal/consumer"
	"github.com/tinyshed/tinystore/bench/idle/internal/probe"
)

func main() {
	probe.Main(consumer.Open())
}
