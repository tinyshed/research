// a read that allocates until its runtime runs out, and one that does nothing, to see the runtime after
import { read } from 'dashbin'

export const bomb = read(async () => {
  const kept = []
  for (;;) kept.push(new Array(1 << 20).fill(1.5))
})

export const nothing = read(async () => [])
