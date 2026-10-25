// the module a plugin imports as 'dashbin': it only records what the plugin declares, and the host reads it back
const declared = { settings: {}, hosts: [] }

export function settings(fields) {
  declared.settings = fields
}

export function hosts(...names) {
  declared.hosts.push(...names)
}

const field = (type) => (options = {}) => ({ type, ...options })
export const secret = field('secret')
export const text = field('text')
export const url = field('url')
export const number = field('number')
export const money = (currency) => ({ unit: 'money', currency })

const kind = (name) => (options, fn) => {
  if (typeof options === 'function') [options, fn] = [{}, options]
  return { __dashbin: name, options, fn }
}
export const read = kind('read')
export const action = kind('action')
export const webhook = kind('webhook')

const secretOf = (value) => Object.freeze({ reveal: () => value, toString: () => '***', toJSON: () => '***' })

globalThis.__dashbinDescribe = () => {
  const functions = {}
  for (const [name, value] of Object.entries(globalThis.__dashbinExports)) {
    if (value && value.__dashbin) functions[name] = { kind: value.__dashbin, ...value.options }
  }
  if (Object.keys(functions).length === 0) throw new Error('dashbin: the module declares no function')
  return JSON.stringify({ settings: declared.settings, hosts: declared.hosts, functions })
}

globalThis.__dashbinCall = (name, paramsJson, settingsJson, id, rows) => {
  const params = JSON.parse(paramsJson)
  if (params.fromUs !== undefined) params.from = new Date(params.fromUs / 1000)
  if (params.toUs !== undefined) params.to = new Date(params.toUs / 1000)
  const config = {}
  for (const [key, value] of Object.entries(JSON.parse(settingsJson))) {
    config[key] = declared.settings[key]?.type === 'secret' ? secretOf(value) : value
  }
  const controller = new AbortController()
  globalThis.__dashbinAbort = controller
  const call = { config, id, signal: controller.signal, limit: { rows } }
  return Promise.resolve(globalThis.__dashbinExports[name].fn(params, call)).then((result) => JSON.stringify(result))
}
