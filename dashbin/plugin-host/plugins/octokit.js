// one repository, in chapter 6's shape; `api` points the SDK at the mock
import { Octokit } from '@octokit/rest'
import { settings, hosts, secret, url, read } from 'dashbin'

settings({ token: secret({ optional: true }), api: url({ optional: true }) })
hosts('api.github.com')

export const repo = read(async (_, { config }) => {
  const octokit = new Octokit({ auth: config.token?.reveal(), baseUrl: config.api })
  const { data } = await octokit.rest.repos.get({ owner: 'tinyshed', repo: 'tinystore' })
  return [{ repo: data.full_name, stars: data.stargazers_count }]
})
