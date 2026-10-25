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
