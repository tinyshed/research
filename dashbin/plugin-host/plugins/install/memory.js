import { read } from 'dashbin'
const kept = []
while (true) kept.push(new Array(1 << 20).fill(1.5))
export const never = read(async () => [])
