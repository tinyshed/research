// Command empty is the baseline every other program's weight is measured
// against: the runtime and os, and nothing else.
package main

import "os"

func main() {
	_, _ = os.Stdout.WriteString(os.Args[0] + "\n")
}
