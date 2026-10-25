import { read } from 'dashbin'
const deeper = (n) => deeper(n + 1) + 1
deeper(0)
export const never = read(async () => [])
