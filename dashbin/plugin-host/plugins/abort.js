// requests a call leaves in flight, against a server that never answers
import { settings, url, read } from 'dashbin'

settings({ slow: url() })

export const waits = read(async (_, { config }) => {
  const response = await fetch(config.slow)
  return [{ status: response.status }]
})

export const waitsWithSignal = read(async (_, { config, signal }) => {
  const response = await fetch(config.slow, { signal })
  return [{ status: response.status }]
})

export const leaves = read(async (_, { config }) => {
  fetch(config.slow).catch(() => {})
  return [{ left: 1 }]
})

export const retries = read(async (_, { config }) => {
  for (;;) {
    try {
      await fetch(config.slow)
    } catch {}
  }
})

// what Stripe's fetch client does: its own controller, a timeout that aborts it, the signal given to fetch
export const sdkLike = read(async (_, { config }) => {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(new Error('timed out')), 80000)
  try {
    const response = await fetch(config.slow, { signal: controller.signal })
    return [{ status: response.status }]
  } finally {
    clearTimeout(timeout)
  }
})

export const sdkLikeNoTimer = read(async (_, { config }) => {
  const controller = new AbortController()
  const response = await fetch(config.slow, { signal: controller.signal })
  return [{ status: response.status }]
})
