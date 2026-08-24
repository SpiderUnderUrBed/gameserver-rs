import { expect, test } from 'vitest'
// import { sum } from './sum.js'

export function sum(a: number, b: number) {
  return a + b
}

test('adds 1 + 2 to equal 3', () => {
  expect(sum(1, 2)).toBe(3)
})

test('get request to /api/users and see if it returns a successful response', async () => {
  const response = await fetch('/api/users')
 

  console.log('Result of /api/users:', {
    status: response.status,
    ok: response.ok,
  })
  const body = await response.json()
  console.log('body: ', body);

  expect(response.ok).toBe(true)
})